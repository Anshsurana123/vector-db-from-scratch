use serde::{Deserialize, Serialize};
use crate::filter::FilterExpression;
use crate::storage::VectorStorage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryStrategy {
    BruteForceScan,
    HnswFiltered,
    FilteredScan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    pub strategy: QueryStrategy,
    pub selectivity: f32,
    pub matching_count: usize,
    pub total_count: usize,
    pub rationale: String,
}

pub struct QueryPlanner;

impl QueryPlanner {
    pub fn plan(
        storage: &VectorStorage,
        filter: Option<&FilterExpression>,
        k: usize,
    ) -> QueryPlan {
        let total = storage.len();
        if total == 0 {
            return QueryPlan {
                strategy: QueryStrategy::BruteForceScan,
                selectivity: 1.0,
                matching_count: 0,
                total_count: 0,
                rationale: "Empty collection: using default brute force scan".into(),
            };
        }

        match filter {
            None => QueryPlan {
                strategy: QueryStrategy::HnswFiltered,
                selectivity: 1.0,
                matching_count: total,
                total_count: total,
                rationale: "Unfiltered query: using standard HNSW graph traversal".into(),
            },
            Some(f) => {
                let matching = filter_matching_count(storage, f);
                let selectivity = matching as f32 / total as f32;

                if selectivity < 0.10 || matching <= k * 2 {
                    QueryPlan {
                        strategy: QueryStrategy::FilteredScan,
                        selectivity,
                        matching_count: matching,
                        total_count: total,
                        rationale: format!(
                            "High filter selectivity ({:.2}%, {}/{} vectors match): using FilteredScan on candidate subset",
                            selectivity * 100.0, matching, total
                        ),
                    }
                } else {
                    QueryPlan {
                        strategy: QueryStrategy::HnswFiltered,
                        selectivity,
                        matching_count: matching,
                        total_count: total,
                        rationale: format!(
                            "Broad filter selectivity ({:.2}%, {}/{} vectors match): using in-graph HNSW pre-filtering",
                            selectivity * 100.0, matching, total
                        ),
                    }
                }
            }
        }
    }
}

const MAX_SAMPLE_SIZE: usize = 1000;

fn filter_matching_count(storage: &VectorStorage, filter: &FilterExpression) -> usize {
    let raw_ids = storage.raw_idx_to_id();
    let total_raw = raw_ids.len();
    if total_raw == 0 {
        return 0;
    }

    if total_raw <= MAX_SAMPLE_SIZE {
        let mut count = 0;
        for &id in raw_ids {
            if storage.is_deleted(id) {
                continue;
            }
            if filter.matches_id(storage, id) {
                count += 1;
            }
        }
        count
    } else {
        let step = (total_raw / MAX_SAMPLE_SIZE).max(1);
        let mut sampled_total = 0;
        let mut sampled_matches = 0;

        for i in (0..total_raw).step_by(step) {
            let id = raw_ids[i];
            if storage.is_deleted(id) {
                continue;
            }
            sampled_total += 1;
            if filter.matches_id(storage, id) {
                sampled_matches += 1;
            }
        }

        if sampled_total == 0 {
            0
        } else {
            let total_live = storage.len();
            let estimated = (sampled_matches as f64 / sampled_total as f64 * total_live as f64).round() as usize;
            estimated.min(total_live)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_planner_selectivity() {
        let mut storage = VectorStorage::new(2);
        for i in 0..100 {
            storage.insert(
                i as u64,
                &[i as f32, i as f32],
                Some(serde_json::json!({"val": i})),
            ).unwrap();
        }

        // Selective filter (5 matching out of 100 = 5%)
        let selective_filter = FilterExpression::Lt("val".into(), 5.0);
        let plan = QueryPlanner::plan(&storage, Some(&selective_filter), 10);
        assert_eq!(plan.strategy, QueryStrategy::FilteredScan);
        assert_eq!(plan.matching_count, 5);

        // Broad filter (80 matching out of 100 = 80%)
        let broad_filter = FilterExpression::Lt("val".into(), 80.0);
        let plan_broad = QueryPlanner::plan(&storage, Some(&broad_filter), 10);
        assert_eq!(plan_broad.strategy, QueryStrategy::HnswFiltered);
        assert_eq!(plan_broad.matching_count, 80);
    }

    #[test]
    fn test_query_planner_large_dataset_sampling() {
        let mut storage = VectorStorage::new(2);
        // Insert 10,000 vectors with 5% matching val < 500
        for i in 0..10000 {
            storage.insert(
                i as u64,
                &[i as f32, i as f32],
                Some(serde_json::json!({"val": i})),
            ).unwrap();
        }

        let selective_filter = FilterExpression::Lt("val".into(), 500.0);
        let plan = QueryPlanner::plan(&storage, Some(&selective_filter), 10);
        // Should select FilteredScan since selectivity ~ 5% (< 10%)
        assert_eq!(plan.strategy, QueryStrategy::FilteredScan);
        assert!(plan.matching_count > 400 && plan.matching_count < 600);
    }
}
