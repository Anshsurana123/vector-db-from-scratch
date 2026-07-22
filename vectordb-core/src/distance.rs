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
        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        let chunks_a = a.chunks_exact(4);
        let chunks_b = b.chunks_exact(4);
        let rem_a = chunks_a.remainder();
        let rem_b = chunks_b.remainder();

        for (ca, cb) in chunks_a.zip(chunks_b) {
            let d0 = ca[0] - cb[0];
            let d1 = ca[1] - cb[1];
            let d2 = ca[2] - cb[2];
            let d3 = ca[3] - cb[3];
            sum0 += d0 * d0;
            sum1 += d1 * d1;
            sum2 += d2 * d2;
            sum3 += d3 * d3;
        }

        let mut sum = sum0 + sum1 + sum2 + sum3;
        for (&x, &y) in rem_a.iter().zip(rem_b) {
            let diff = x - y;
            sum += diff * diff;
        }
        sum
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CosineDistance;

impl DistanceMetric for CosineDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());
        let mut dot0 = 0.0f32;
        let mut dot1 = 0.0f32;
        let mut na0 = 0.0f32;
        let mut na1 = 0.0f32;
        let mut nb0 = 0.0f32;
        let mut nb1 = 0.0f32;

        let chunks_a = a.chunks_exact(2);
        let chunks_b = b.chunks_exact(2);
        let rem_a = chunks_a.remainder();
        let rem_b = chunks_b.remainder();

        for (ca, cb) in chunks_a.zip(chunks_b) {
            let x0 = ca[0];
            let x1 = ca[1];
            let y0 = cb[0];
            let y1 = cb[1];

            dot0 += x0 * y0;
            dot1 += x1 * y1;
            na0 += x0 * x0;
            na1 += x1 * x1;
            nb0 += y0 * y0;
            nb1 += y1 * y1;
        }

        let mut dot = dot0 + dot1;
        let mut norm_a = na0 + na1;
        let mut norm_b = nb0 + nb1;

        for (&x, &y) in rem_a.iter().zip(rem_b) {
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
        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        let chunks_a = a.chunks_exact(4);
        let chunks_b = b.chunks_exact(4);
        let rem_a = chunks_a.remainder();
        let rem_b = chunks_b.remainder();

        for (ca, cb) in chunks_a.zip(chunks_b) {
            sum0 += ca[0] * cb[0];
            sum1 += ca[1] * cb[1];
            sum2 += ca[2] * cb[2];
            sum3 += ca[3] * cb[3];
        }

        let mut dot = sum0 + sum1 + sum2 + sum3;
        for (&x, &y) in rem_a.iter().zip(rem_b) {
            dot += x * y;
        }
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
        let v1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let v2 = vec![4.0, 5.0, 6.0, 7.0, 8.0];
        // 5 * 9 = 45
        assert_eq!(metric.distance(&v1, &v2), 45.0);
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
        let v1 = vec![1.0, 2.0, 3.0, 4.0];
        let v2 = vec![4.0, 5.0, 6.0, 7.0];
        // dot = 4 + 10 + 18 + 28 = 60 -> distance = -60
        assert_eq!(metric.distance(&v1, &v2), -60.0);
    }
}
