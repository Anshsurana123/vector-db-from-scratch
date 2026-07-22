import os
import urllib.request
import tarfile

def download_and_extract():
    target_dir = os.path.join("vectordb-bench", "data", "sift")
    os.makedirs(target_dir, exist_ok=True)
    
    tar_path = os.path.join(target_dir, "sift.tar.gz")
    url = "ftp://ftp.irisa.fr/local/texmex/corpus/sift.tar.gz"
    
    print(f"Downloading SIFT1M dataset from {url}...")
    try:
        urllib.request.urlretrieve(url, tar_path)
        print(f"Downloaded archive to {tar_path}. Extracting...")
        with tarfile.open(tar_path, "r:gz") as tar:
            tar.extractall(target_dir)
        print("Extraction complete! Files present:")
        for root, dirs, files in os.walk(target_dir):
            for f in files:
                print(" -", os.path.join(root, f))
    except Exception as e:
        print(f"FTP Download failed: {e}. Trying alternative mirror...")
        # HuggingFace raw mirror or ANN benchmarks mirror fallback
        alt_url = "https://huggingface.co/datasets/maknee/sift1m/resolve/main/sift_base.fvecs"
        alt_query_url = "https://huggingface.co/datasets/maknee/sift1m/resolve/main/sift_query.fvecs"
        sift_subdir = os.path.join(target_dir, "sift")
        os.makedirs(sift_subdir, exist_ok=True)
        
        base_fvecs = os.path.join(sift_subdir, "sift_base.fvecs")
        query_fvecs = os.path.join(sift_subdir, "sift_query.fvecs")
        
        urllib.request.urlretrieve(alt_url, base_fvecs)
        urllib.request.urlretrieve(alt_query_url, query_fvecs)
        print("Downloaded SIFT1M fvecs files from HuggingFace mirror successfully!")

if __name__ == "__main__":
    download_and_extract()
