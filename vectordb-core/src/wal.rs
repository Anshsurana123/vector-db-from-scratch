use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};

use crate::distance::MetricType;
use crate::error::{Result, VectorDbError};
use crate::hnsw::HnswConfig;

const WAL_MAGIC: &[u8; 4] = b"VWAL";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOp {
    CreateCollection {
        name: String,
        dim: usize,
        metric: MetricType,
        config: HnswConfig,
    },
    Insert {
        collection: String,
        id: u64,
        vector: Vec<f32>,
        metadata: Option<serde_json::Value>,
    },
    Delete {
        collection: String,
        id: u64,
    },
}

#[derive(Debug, Clone)]
pub struct WalFrame {
    pub seq: u64,
    pub op: WalOp,
}

pub struct WalWriter {
    file_path: PathBuf,
    writer: BufWriter<File>,
}

impl WalWriter {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path_buf)?;
        
        Ok(Self {
            file_path: path_buf,
            writer: BufWriter::new(file),
        })
    }

    pub fn append(&mut self, seq: u64, op: &WalOp) -> Result<()> {
        let payload = bincode::serialize(op)
            .map_err(|e| VectorDbError::StorageError(format!("WAL serialization error: {}", e)))?;
        
        let payload_len = payload.len() as u32;
        let op_type: u8 = match op {
            WalOp::CreateCollection { .. } => 1,
            WalOp::Insert { .. } => 2,
            WalOp::Delete { .. } => 3,
        };

        // Compute CRC32 over header + payload
        let mut hasher = Hasher::new();
        hasher.update(WAL_MAGIC);
        hasher.update(&[op_type]);
        hasher.update(&seq.to_le_bytes());
        hasher.update(&payload_len.to_le_bytes());
        hasher.update(&payload);
        let crc32 = hasher.finalize();

        // Write Frame
        self.writer.write_all(WAL_MAGIC)?;
        self.writer.write_all(&[op_type])?;
        self.writer.write_all(&seq.to_le_bytes())?;
        self.writer.write_all(&payload_len.to_le_bytes())?;
        self.writer.write_all(&payload)?;
        self.writer.write_all(&crc32.to_le_bytes())?;

        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(())
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

pub struct WalReader;

impl WalReader {
    pub fn read_all(path: impl AsRef<Path>) -> Result<(Vec<WalFrame>, u64)> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Ok((Vec::new(), 0));
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path_ref)?;
            
        let file_len = file.metadata()?.len();
        let mut reader = BufReader::new(&file);

        let mut frames = Vec::new();
        let mut last_valid_offset = 0u64;

        loop {
            let current_pos = match reader.stream_position() {
                Ok(pos) => pos,
                Err(_) => break,
            };

            let mut magic = [0u8; 4];
            if reader.read_exact(&mut magic).is_err() {
                break;
            }
            if &magic != WAL_MAGIC {
                println!("WAL recovery: Invalid magic bytes at offset {}, stopping.", current_pos);
                break;
            }

            let mut op_type_buf = [0u8; 1];
            if reader.read_exact(&mut op_type_buf).is_err() {
                break;
            }

            let mut seq_buf = [0u8; 8];
            if reader.read_exact(&mut seq_buf).is_err() {
                break;
            }
            let seq = u64::from_le_bytes(seq_buf);

            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).is_err() {
                break;
            }
            let payload_len = u32::from_le_bytes(len_buf) as usize;

            let mut payload = vec![0u8; payload_len];
            if reader.read_exact(&mut payload).is_err() {
                println!("WAL recovery: Partial frame payload at offset {}, stopping.", current_pos);
                break;
            }

            let mut crc_buf = [0u8; 4];
            if reader.read_exact(&mut crc_buf).is_err() {
                println!("WAL recovery: Partial CRC32 trailer at offset {}, stopping.", current_pos);
                break;
            }
            let expected_crc = u32::from_le_bytes(crc_buf);

            // Compute actual CRC32
            let mut hasher = Hasher::new();
            hasher.update(WAL_MAGIC);
            hasher.update(&op_type_buf);
            hasher.update(&seq_buf);
            hasher.update(&len_buf);
            hasher.update(&payload);
            let actual_crc = hasher.finalize();

            if actual_crc != expected_crc {
                println!("WAL recovery: CRC32 mismatch at offset {} (expected {}, got {}), truncating.", current_pos, expected_crc, actual_crc);
                break;
            }

            let op: WalOp = bincode::deserialize(&payload)
                .map_err(|e| VectorDbError::StorageError(format!("WAL frame deserialization failed: {}", e)))?;

            frames.push(WalFrame { seq, op });
            last_valid_offset = match reader.stream_position() {
                Ok(pos) => pos,
                Err(_) => break,
            };
        }

        drop(reader);

        // If file was truncated at partial frame, trim corrupted EOF bytes
        if last_valid_offset < file_len {
            println!("Truncating WAL file {:?} from {} bytes to {} valid bytes.", path_ref, file_len, last_valid_offset);
            file.set_len(last_valid_offset)?;
            file.sync_all()?;
        }

        Ok((frames, last_valid_offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wal_write_read_and_corruption_truncation() -> Result<()> {
        let dir = tempdir()?;
        let wal_path = dir.path().join("test.wal");

        let mut writer = WalWriter::open(&wal_path)?;
        writer.append(
            1,
            &WalOp::CreateCollection {
                name: "col1".into(),
                dim: 4,
                metric: MetricType::L2,
                config: HnswConfig::default(),
            },
        )?;
        writer.append(
            2,
            &WalOp::Insert {
                collection: "col1".into(),
                id: 100,
                vector: vec![1.0, 2.0, 3.0, 4.0],
                metadata: None,
            },
        )?;
        writer.flush()?;
        drop(writer);

        let (frames, _) = WalReader::read_all(&wal_path)?;
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].seq, 1);
        assert_eq!(frames[1].seq, 2);

        // Append partial corrupted frame to test truncation
        let mut file = OpenOptions::new().append(true).open(&wal_path)?;
        file.write_all(b"VWAL\x02\x03\x00\x00\x00\x00\x00\x00\x00\x05\x00\x00\x00corrupt")?;
        file.sync_all()?;
        drop(file);

        // Re-read and ensure corrupt partial frame is safely truncated
        let (recovered, _) = WalReader::read_all(&wal_path)?;
        assert_eq!(recovered.len(), 2);

        Ok(())
    }
}
