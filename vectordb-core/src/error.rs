use thiserror::Error;

#[derive(Error, Debug)]
pub enum VectorDbError {
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Vector with ID {0} already exists")]
    DuplicateId(u64),

    #[error("Vector with ID {0} not found")]
    VectorNotFound(u64),

    #[error("Collection {0} not found")]
    CollectionNotFound(String),

    #[error("Collection {0} already exists")]
    CollectionAlreadyExists(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid WAL CRC checksum at record {offset}: expected {expected:#x}, got {actual:#x}")]
    WalCrcMismatch { offset: u64, expected: u32, actual: u32 },
}

pub type Result<T> = std::result::Result<T, VectorDbError>;
