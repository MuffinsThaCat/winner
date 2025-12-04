#!/bin/bash
# Run SupraBTM benchmark on same 500 blocks

set -e

echo "============================================"
echo "Running SupraBTM Benchmark (500 blocks)"
echo "============================================"
echo ""

# Setup
DATA_DIR="$PWD/data_100k"
OUTPUT_DIR="$PWD/suprabtm_results"

mkdir -p "$OUTPUT_DIR"

# Check Docker is available
DOCKER="/Applications/Docker.app/Contents/Resources/bin/docker"
if [ ! -f "$DOCKER" ]; then
    echo "Error: Docker Desktop not found at $DOCKER"
    echo "Please start Docker Desktop first."
    exit 1
fi

# Check if Docker daemon is running
if ! $DOCKER ps &> /dev/null; then
    echo "Error: Docker daemon not running. Please start Docker Desktop."
    exit 1
fi

# Check data exists
if [ ! -d "$DATA_DIR" ]; then
    echo "Error: Data directory not found at $DATA_DIR"
    exit 1
fi

echo "Data directory: $DATA_DIR"
echo "Output directory: $OUTPUT_DIR"
echo ""

# Pull latest image
echo "Pulling SupraBTM Docker image..."
$DOCKER pull rohitkapoor9312/ibtm-image:latest

echo ""
echo "Starting SupraBTM execution..."
echo ""

# Run SupraBTM (use 0-13 for 14-core system)
time $DOCKER run --rm \
  --cpuset-cpus="0-13" \
  -v "$DATA_DIR:/data" \
  -v "$OUTPUT_DIR:/out" \
  rohitkapoor9312/ibtm-image:latest \
  --data-dir /data \
  --output-dir /out \
  --inmemory

echo ""
echo "============================================"
echo "SupraBTM Benchmark Complete"
echo "============================================"
echo ""
echo "Results saved to: $OUTPUT_DIR/"
echo ""
echo "To compare with Williams:"
echo "  cat $OUTPUT_DIR/execution_time.txt"
echo "  cat williams_revm_complete/williams_complete_results.txt"
echo ""
