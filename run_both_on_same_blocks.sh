#!/bin/bash
# Run BOTH Williams and SupraBTM on the EXACT SAME downloaded blocks
# Uses first 500 blocks from data_100k (18000000-18000499)

set -e

echo "======================================================================"
echo "APPLES-TO-APPLES: Williams vs SupraBTM on SAME Downloaded Blocks"
echo "======================================================================"
echo ""

DOCKER="/Applications/Docker.app/Contents/Resources/bin/docker"

# Check if blocks are downloaded
if [ ! -d "data_100k/blocks" ]; then
    echo "❌ Block data not found in data_100k/"
    echo "   Please extract data_100k.tar.gz first"
    exit 1
fi

BLOCK_COUNT=$(find data_100k/blocks -name "*.json" | wc -l | xargs)
echo "📦 Found $BLOCK_COUNT downloaded blocks"
echo "   Testing first 500 blocks (both systems auto-limit to 500)"
echo ""

# ==============================================================================
# STEP 1: Run Williams on downloaded blocks
# ==============================================================================

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 1: Running Williams Hybrid Executor"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

cd williams_revm_complete

echo "Building Williams (Rust)..."
cargo build --release --quiet 2>&1 | head -5

WILLIAMS_OUT="../apples_results/williams_execution.txt"
mkdir -p ../apples_results

echo "Executing Williams on blocks 18000000-18000499..."
echo "  Output: $WILLIAMS_OUT"
echo ""

START_TIME=$(date +%s)

# Williams automatically takes first 500 blocks
cargo run --release -- ../data_100k 16 > "$WILLIAMS_OUT" 2>&1

END_TIME=$(date +%s)
WILLIAMS_TIME=$((END_TIME - START_TIME))

echo ""
echo "✅ Williams complete in ${WILLIAMS_TIME}s"
echo ""

cd ..

# ==============================================================================
# STEP 2: Run SupraBTM on the SAME downloaded blocks
# ==============================================================================

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 2: Running SupraBTM on SAME blocks"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check Docker
if [ ! -f "$DOCKER" ]; then
    echo "❌ Docker Desktop not found"
    exit 1
fi

if ! $DOCKER ps &> /dev/null; then
    echo "❌ Docker daemon not running. Please start Docker Desktop."
    exit 1
fi

echo "Pulling SupraBTM Docker image..."
$DOCKER pull rohitkapoor9312/ibtm-image:latest

SUPRABTM_OUT="$PWD/apples_results/suprabtm_same_blocks"
mkdir -p "$SUPRABTM_OUT"

echo ""
echo "Executing SupraBTM on downloaded blocks..."
echo "  Output: $SUPRABTM_OUT/"
echo ""

START_TIME=$(date +%s)

$DOCKER run --rm \
  --cpuset-cpus="0-13" \
  -v "$PWD/data_100k:/data" \
  -v "$SUPRABTM_OUT:/out" \
  rohitkapoor9312/ibtm-image:latest \
  --data-dir /data \
  --output-dir /out \
  --inmemory

END_TIME=$(date +%s)
SUPRABTM_TIME=$((END_TIME - START_TIME))

echo ""
echo "✅ SupraBTM complete in ${SUPRABTM_TIME}s"
echo ""

# ==============================================================================
# STEP 3: Compare Results
# ==============================================================================

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 3: Analyzing Results"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

python3 compare_apples_to_apples.py

echo ""
echo "======================================================================"
echo "✅ COMPLETE: True Apples-to-Apples Comparison"
echo "   Same blocks, same machine, same data files"
echo "======================================================================"
echo ""
