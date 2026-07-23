use serde::{Deserialize, Serialize};
use serde_json::Value;


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
