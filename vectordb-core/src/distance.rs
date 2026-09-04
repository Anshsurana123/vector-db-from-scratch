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

#[inline(always)]
pub fn compute_distance(metric: MetricType, a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    match metric {
        MetricType::L2 => {
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
        MetricType::Cosine => {
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
        MetricType::DotProduct => {
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
}

#[derive(Debug, Clone, Copy)]
pub struct L2Distance;

impl DistanceMetric for L2Distance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        compute_distance(MetricType::L2, a, b)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CosineDistance;

impl DistanceMetric for CosineDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        compute_distance(MetricType::Cosine, a, b)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DotProductDistance;

impl DistanceMetric for DotProductDistance {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        compute_distance(MetricType::DotProduct, a, b)
    }
}

pub fn get_distance_metric(metric: MetricType) -> Box<dyn DistanceMetric> {
    match metric {
        MetricType::L2 => Box::new(L2Distance),
        MetricType::Cosine => Box::new(CosineDistance),
        MetricType::DotProduct => Box::new(DotProductDistance),
    }
}

// -------------------------------------------------------------------------
// Scalar Reference Implementations (for correctness validation)
// -------------------------------------------------------------------------

pub fn reference_l2(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

pub fn reference_cosine(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
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

pub fn reference_dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    -dot
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

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

    fn normalize(v: &mut [f32]) {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    #[test]
    fn test_unrolled_vs_reference_all_metrics() {
        let mut rng = StdRng::seed_from_u64(987654321);

        // Dimensions: odd, not divisible by 4, very small, large
        let test_dimensions = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 15, 16, 17, 31, 64, 127, 128, 129, 256, 512, 1024, 1536
        ];

        for &dim in &test_dimensions {
            // 1. Positive values only
            let pos_a: Vec<f32> = (0..dim).map(|_| rng.gen_range(0.01..100.0)).collect();
            let pos_b: Vec<f32> = (0..dim).map(|_| rng.gen_range(0.01..100.0)).collect();
            assert_distance_parity(&pos_a, &pos_b, dim, "positive values");

            // 2. Negative values only
            let neg_a: Vec<f32> = (0..dim).map(|_| rng.gen_range(-100.0..-0.01)).collect();
            let neg_b: Vec<f32> = (0..dim).map(|_| rng.gen_range(-100.0..-0.01)).collect();
            assert_distance_parity(&neg_a, &neg_b, dim, "negative values");

            // 3. Mixed positive and negative values
            let mixed_a: Vec<f32> = (0..dim).map(|_| rng.gen_range(-50.0..50.0)).collect();
            let mixed_b: Vec<f32> = (0..dim).map(|_| rng.gen_range(-50.0..50.0)).collect();
            assert_distance_parity(&mixed_a, &mixed_b, dim, "mixed values");

            // 4. Zero vectors
            let zero = vec![0.0f32; dim];
            assert_distance_parity(&zero, &mixed_a, dim, "zero vs mixed");
            assert_distance_parity(&zero, &zero, dim, "zero vs zero");

            // 5. Normalized vectors (unit sphere)
            let mut norm_a = mixed_a.clone();
            let mut norm_b = mixed_b.clone();
            normalize(&mut norm_a);
            normalize(&mut norm_b);
            assert_distance_parity(&norm_a, &norm_b, dim, "normalized vectors");
        }
    }

    fn assert_distance_parity(a: &[f32], b: &[f32], dim: usize, label: &str) {
        // L2
        let opt_l2 = compute_distance(MetricType::L2, a, b);
        let ref_l2 = reference_l2(a, b);
        let max_l2 = opt_l2.abs().max(ref_l2.abs()).max(1.0);
        let rel_err_l2 = (opt_l2 - ref_l2).abs() / max_l2;
        assert!(
            rel_err_l2 < 1e-4,
            "L2 mismatch for dim={} [{}]: opt={}, ref={}, rel_err={}",
            dim, label, opt_l2, ref_l2, rel_err_l2
        );

        // Cosine
        let opt_cos = compute_distance(MetricType::Cosine, a, b);
        let ref_cos = reference_cosine(a, b);
        let diff_cos = (opt_cos - ref_cos).abs();
        assert!(
            diff_cos < 1e-4,
            "Cosine mismatch for dim={} [{}]: opt={}, ref={}, diff={}",
            dim, label, opt_cos, ref_cos, diff_cos
        );

        // Dot Product
        let opt_dot = compute_distance(MetricType::DotProduct, a, b);
        let ref_dot = reference_dot_product(a, b);
        let max_dot = opt_dot.abs().max(ref_dot.abs()).max(1.0);
        let rel_err_dot = (opt_dot - ref_dot).abs() / max_dot;
        assert!(
            rel_err_dot < 1e-4,
            "Dot product mismatch for dim={} [{}]: opt={}, ref={}, rel_err={}",
            dim, label, opt_dot, ref_dot, rel_err_dot
        );
    }
}
