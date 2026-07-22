use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, VectorDbError};
use crate::storage::VectorStorage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterExpression {
    Eq(String, Value),
    Gt(String, f64),
    Gte(String, f64),
    Lt(String, f64),
    Lte(String, f64),
    In(String, Vec<Value>),
    And(Vec<FilterExpression>),
    Or(Vec<FilterExpression>),
}

impl FilterExpression {
    /// Evaluates filter expression against a single JSON metadata object
    pub fn matches(&self, metadata: &Value) -> bool {
        match self {
            FilterExpression::Eq(field, expected) => {
                metadata.get(field).map(|v| v == expected).unwrap_or(false)
            }
            FilterExpression::Gt(field, threshold) => {
                metadata.get(field).and_then(|v| v.as_f64()).map(|num| num > *threshold).unwrap_or(false)
            }
            FilterExpression::Gte(field, threshold) => {
                metadata.get(field).and_then(|v| v.as_f64()).map(|num| num >= *threshold).unwrap_or(false)
            }
            FilterExpression::Lt(field, threshold) => {
                metadata.get(field).and_then(|v| v.as_f64()).map(|num| num < *threshold).unwrap_or(false)
            }
            FilterExpression::Lte(field, threshold) => {
                metadata.get(field).and_then(|v| v.as_f64()).map(|num| num <= *threshold).unwrap_or(false)
            }
            FilterExpression::In(field, allowed) => {
                metadata.get(field).map(|v| allowed.contains(v)).unwrap_or(false)
            }
            FilterExpression::And(exprs) => {
                exprs.iter().all(|e| e.matches(metadata))
            }
            FilterExpression::Or(exprs) => {
                exprs.iter().any(|e| e.matches(metadata))
            }
        }
    }

    /// Builds a RoaringBitmap containing all matching vector IDs from storage
    pub fn build_bitmap(&self, storage: &VectorStorage) -> Result<RoaringBitmap> {
        let mut bitmap = RoaringBitmap::new();

        // Iterate over all active vectors in storage
        for idx in 0..storage.raw_data().len() / storage.dim() {
            if let Some(vec) = storage.get_vector_by_idx(idx) {
                // Find vector ID
                let id = idx as u64; // Fallback mapping, checked via metadata store
                if let Some(meta) = storage.get_metadata(id) {
                    if self.matches(meta) {
                        if id <= u32::MAX as u64 {
                            bitmap.insert(id as u32);
                        }
                    }
                }
            }
        }

        Ok(bitmap)
    }

    /// Evaluates bitmap filter against a specific vector ID (u64)
    pub fn matches_id(&self, storage: &VectorStorage, id: u64) -> bool {
        match storage.get_metadata(id) {
            Some(meta) => self.matches(meta),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_expressions() {
        let meta = serde_json::json!({
            "category": "electronics",
            "price": 99.99,
            "rating": 4.5,
            "tags": ["sale", "tech"]
        });

        let eq_filter = FilterExpression::Eq("category".into(), serde_json::json!("electronics"));
        assert!(eq_filter.matches(&meta));

        let price_gt = FilterExpression::Gt("price".into(), 50.0);
        assert!(price_gt.matches(&meta));

        let price_lt = FilterExpression::Lt("price".into(), 50.0);
        assert!(!price_lt.matches(&meta));

        let and_filter = FilterExpression::And(vec![
            FilterExpression::Eq("category".into(), serde_json::json!("electronics")),
            FilterExpression::Gte("price".into(), 90.0),
        ]);
        assert!(and_filter.matches(&meta));
    }
}
