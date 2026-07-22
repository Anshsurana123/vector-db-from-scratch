import json
import numpy as np

def main():
    np.random.seed(42)
    num_vectors = 10000
    dim = 128
    vectors = np.random.randn(num_vectors, dim).astype(np.float32)

    np.random.seed(12345)
    num_queries = 100
    queries = np.random.randn(num_queries, dim).astype(np.float32)

    # Save dataset and queries to JSON for Rust test to load identical dataset
    dataset_data = {
        "vectors": vectors.tolist(),
        "queries": queries.tolist()
    }
    with open("milestone1_dataset.json", "w") as f:
        json.dump(dataset_data, f)
    print("Saved milestone1_dataset.json")

    # Compute ground truth for L2 distance (squared Euclidean distance)
    # distance = sum((q - v)^2)
    spot_checks = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90]
    k = 10

    ground_truth = {}

    for q_idx in range(num_queries):
        query = queries[q_idx]
        # L2 squared
        diffs = vectors - query
        dists_l2 = np.sum(diffs ** 2, axis=1)
        top_k_indices_l2 = np.argsort(dists_l2)[:k]
        
        # Cosine distance
        norm_q = np.linalg.norm(query)
        norm_v = np.linalg.norm(vectors, axis=1)
        dots = np.dot(vectors, query)
        dists_cosine = 1.0 - (dots / (norm_q * norm_v))
        top_k_indices_cosine = np.argsort(dists_cosine)[:k]

        # Dot product distance (-dot)
        dists_dot = -dots
        top_k_indices_dot = np.argsort(dists_dot)[:k]

        if q_idx in spot_checks:
            ground_truth[str(q_idx)] = {
                "l2": [
                    {"id": int(idx), "distance": float(dists_l2[idx])}
                    for idx in top_k_indices_l2
                ],
                "cosine": [
                    {"id": int(idx), "distance": float(dists_cosine[idx])}
                    for idx in top_k_indices_cosine
                ],
                "dot": [
                    {"id": int(idx), "distance": float(dists_dot[idx])}
                    for idx in top_k_indices_dot
                ]
            }

    with open("milestone1_ground_truth.json", "w") as f:
        json.dump(ground_truth, f, indent=2)

    print("Saved milestone1_ground_truth.json with 10 spot-checked query ground truths.")

if __name__ == "__main__":
    main()
