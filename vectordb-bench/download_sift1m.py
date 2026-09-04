import os
import socket
import urllib.request

def download_and_extract():
    target_dir = os.path.join("vectordb-bench", "data", "sift")
    os.makedirs(target_dir, exist_ok=True)
    sift_subdir = os.path.join(target_dir, "sift")
    os.makedirs(sift_subdir, exist_ok=True)

    base_fvecs = os.path.join(sift_subdir, "sift_base.fvecs")
    query_fvecs = os.path.join(sift_subdir, "sift_query.fvecs")

    if os.path.exists(base_fvecs) and os.path.exists(query_fvecs):
        print("SIFT1M dataset files already exist locally.")
        return

    # Try HuggingFace mirror with strict timeout
    alt_url = "https://huggingface.co/datasets/maknee/sift1m/resolve/main/sift_base.fvecs"
    alt_query_url = "https://huggingface.co/datasets/maknee/sift1m/resolve/main/sift_query.fvecs"

    print("Attempting to fetch SIFT1M dataset from mirror (timeout 5s)...")
    socket.setdefaulttimeout(5.0)

    try:
        urllib.request.urlretrieve(alt_url, base_fvecs)
        urllib.request.urlretrieve(alt_query_url, query_fvecs)
        print("Downloaded SIFT1M fvecs files successfully!")
    except Exception as e:
        print(f"Dataset download skipped ({e}). Falling back to synthetic normalized vectors.")
        if os.path.exists(base_fvecs):
            try: os.remove(base_fvecs)
            except OSError: pass
        if os.path.exists(query_fvecs):
            try: os.remove(query_fvecs)
            except OSError: pass

if __name__ == "__main__":
    download_and_extract()
