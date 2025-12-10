# Williams Executor - Docker Usage

## Quick Start

### 1. Build the Image
```bash
chmod +x docker-build.sh
./docker-build.sh
```

### 2. Run Benchmark
```bash
# Download Supra's dataset first
mkdir -p data_bdf
cd data_bdf
# ... download data ...

# Run Williams in Docker
docker run --rm \
  --cpuset-cpus="0-7" \
  -v "$PWD/data_bdf:/data" \
  williams/executor:latest \
  /data 16
```

## Side-by-Side Comparison

```bash
# Run both Williams and Supra with docker-compose
docker-compose run --rm williams-sequential
docker-compose run --rm supra-btm

# Compare results
cat stats_williams/results.txt
cat stats_suprabtm/execution_time.txt
```

## Testing the Image

```bash
# Test help
docker run --rm williams/executor:latest --help

# Test with small dataset
docker run --rm \
  -v "$PWD/test_data:/data" \
  williams/executor:latest /data 4
```

## Reproducibility

This Docker image ensures:
- ✅ Same build environment
- ✅ Same dependencies
- ✅ Same optimizations
- ✅ Reproducible results

Anyone can verify Williams' performance by running the same container.
