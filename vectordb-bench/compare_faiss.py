import time
import json
import os
import struct
import numpy as np

try:
    import faiss
    HAS_FAISS = True
except ImportError:
    HAS_FAISS = False

def read_fvecs(path, max_count=None):
    vectors = []
    with open(path, 'rb') as f:
        while True:
            if max_count and len(vectors) >= max_count:
                break
            dim_buf = f.read(4)
            if not dim_buf:
                break
            dim = struct.unpack('<I', dim_buf)[0]
            vec_buf = f.read(dim * 4)
            vec = struct.unpack(f'<{dim}f', vec_buf)
            vectors.append(vec)
    return np.array(vectors, dtype=np.float32)

def generate_synthetic(num, dim, seed=42):
    np.random.seed(seed)
    v = np.random.uniform(-1.0, 1.0, (num, dim)).astype(np.float32)
    norms = np.linalg.norm(v, axis=1, keepdims=True)
    norms[norms == 0] = 1e-10
    return v / norms

def main():
    if not HAS_FAISS:
        print("FAISS not installed, skipping FAISS benchmark.")
        return

    data_dir = "vectordb-bench/data/sift"
    base_file = os.path.join(data_dir, "sift_base.fvecs")
    query_file = os.path.join(data_dir, "sift_query.fvecs")
    
    if os.path.exists(base_file) and os.path.exists(query_file):
        vectors = read_fvecs(base_file, 10000)
        queries = read_fvecs(query_file, 1000)
    else:
        vectors = generate_synthetic(10000, 128, 42)
        queries = generate_synthetic(1000, 128, 12345)

    vectors = np.ascontiguousarray(vectors, dtype=np.float32)
    queries = np.ascontiguousarray(queries, dtype=np.float32)

    num_vectors, dim = vectors.shape
    num_queries = queries.shape[0]
    k = 10
    
    gt_index = faiss.IndexFlatL2(dim)
    gt_index.add(vectors)
    sample_queries = 100
    _, gt_I = gt_index.search(queries[:sample_queries], k)
    
    results = {}
    
    M = 16
    efConstruction = 100
    
    index = faiss.IndexHNSWFlat(dim, M)
    index.hnsw.efConstruction = efConstruction
    
    start_time = time.time()
    index.add(vectors)
    index_time = time.time() - start_time
    throughput = num_vectors / index_time
    
    results["throughput"] = throughput
    
    ef_values = [10, 50, 100, 200, 300]
    results["search"] = {}
    
    for ef in ef_values:
        index.hnsw.efSearch = ef
        
        latencies = []
        for q in queries:
            q_reshaped = q.reshape(1, dim)
            start_q = time.perf_counter()
            index.search(q_reshaped, k)
            latencies.append((time.perf_counter() - start_q) * 1000.0)
            
        latencies = np.array(latencies)
        p50 = np.percentile(latencies, 50)
        p95 = np.percentile(latencies, 95)
        p99 = np.percentile(latencies, 99)
        avg = np.mean(latencies)
        
        _, I = index.search(queries[:sample_queries], k)
        hits = 0
        for i in range(sample_queries):
            hits += len(set(I[i]).intersection(set(gt_I[i])))
        recall = hits / (sample_queries * k)
        
        results["search"][str(ef)] = {
            "recall": recall,
            "p50": p50,
            "p95": p95,
            "p99": p99,
            "avg": avg
        }
        
    with open("vectordb-bench/faiss_results.json", "w") as f:
        json.dump(results, f, indent=4)

if __name__ == "__main__":
    main()
