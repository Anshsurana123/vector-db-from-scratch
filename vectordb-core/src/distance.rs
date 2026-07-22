use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    L2,
    Cosine,
    DotProduct,
}

pub trait DistanceMetric: Send + Sync {
    /// Computes distance between two vectors.
    /// Lower distance means higher similarity / closer vectors.
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;
}

#[derive(Debug, Clone, Copy)]
pub struct L2Distance;

impl DistanceMetric for L2Distance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| {
                let diff = x - y;
                diff * diff
            })
            .sum()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CosineDistance;

impl DistanceMetric for CosineDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (&x, &y) in a.iter().zip(b.iter()) {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        let norm = (norm_a * norm_b).sqrt();
        if norm < 1e-10 {
            1.0
        } else {
            1.0 - (dot / norm)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DotProductDistance;

impl DistanceMetric for DotProductDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());
        let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
        -dot
    }
}

pub fn get_distance_metric(metric: MetricType) -> Box<dyn DistanceMetric> {
    match metric {
        MetricType::L2 => Box::new(L2Distance),
        MetricType::Cosine => Box::new(CosineDistance),
        MetricType::DotProduct => Box::new(DotProductDistance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_distance() {
        let metric = L2Distance;
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];
        // (1-4)^2 + (2-5)^2 + (3-6)^2 = 9 + 9 + 9 = 27
        assert_eq!(metric.distance(&v1, &v2), 27.0);
        assert_eq!(metric.distance(&v1, &v1), 0.0);
    }

    #[test]
    fn test_cosine_distance() {
        let metric = CosineDistance;
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let v3 = vec![2.0, 0.0, 0.0];
        assert!((metric.distance(&v1, &v2) - 1.0).abs() < 1e-6);
        assert!((metric.distance(&v1, &v3) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product_distance() {
        let metric = DotProductDistance;
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];
        // dot = 4 + 10 + 18 = 32 -> distance = -32
        assert_eq!(metric.distance(&v1, &v2), -32.0);
    }
}
