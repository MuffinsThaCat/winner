#!/bin/bash
# Williams vs SupraBTM - Side-by-Side Comparison
# Runs both executors on the same blocks for fair comparison

set -e

DOCKER=/usr/local/bin/docker
DATA_DIR="$PWD/data_100k"
STATS_DIR="$PWD/comparison_results"

echo "=========================================="
echo "Williams vs SupraBTM Comparison"
echo "=========================================="
echo ""

# Create output directory
mkdir -p "$STATS_DIR"

# Check if data exists
if [ ! -d "$DATA_DIR/blocks" ]; then
    echo "Error: Block data not found at $DATA_DIR/blocks"
    exit 1
fi

# Count available blocks
BLOCK_COUNT=$(ls "$DATA_DIR/blocks" | wc -l | tr -d ' ')
echo "Found $BLOCK_COUNT blocks in dataset"
echo ""

# Ask how many blocks to test
echo "How many blocks to test? (10 recommended for quick comparison)"
read -p "Number of blocks [10]: " NUM_BLOCKS
NUM_BLOCKS=${NUM_BLOCKS:-10}

echo ""
echo "=========================================="
echo "TEST 1: Williams Hybrid Executor"
echo "=========================================="
echo ""

cd williams_revm_complete

# Run Williams
./target/release/williams-complete "$DATA_DIR" 16 | tee "$STATS_DIR/williams_output.txt"

# Copy results
cp williams_complete_results.txt "$STATS_DIR/"

cd ..

echo ""
echo "=========================================="
echo "TEST 2: SupraBTM"
echo "=========================================="
echo ""

# Prepare SupraBTM data directory (it expects specific format)
mkdir -p "$STATS_DIR/supra_data"

# Run SupraBTM Docker
echo "Starting SupraBTM Docker container..."
echo "NOTE: SupraBTM expects data in specific format. Using available blocks..."

$DOCKER run --rm \
  --cpuset-cpus="0-15" \
  -v "$DATA_DIR:/data" \
  -v "$STATS_DIR:/out" \
  rohitkapoor9312/ibtm-image:latest \
  --data-dir /data/blocks \
  --output-dir /out \
  --inmemory 2>&1 | tee "$STATS_DIR/supra_output.txt"

echo ""
echo "=========================================="
echo "COMPARISON COMPLETE"
echo "=========================================="
echo ""
echo "Results saved to: $STATS_DIR/"
echo ""
echo "Williams results: $STATS_DIR/williams_complete_results.txt"
echo "SupraBTM results: $STATS_DIR/execution_time.txt"
echo ""
echo "To analyze results:"
echo "  cat $STATS_DIR/williams_complete_results.txt"
echo "  cat $STATS_DIR/execution_time.txt"
echo ""
