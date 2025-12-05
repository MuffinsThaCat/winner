#!/bin/bash
# Run SupraBTM on the same blocks Williams tested (18000000-18000499)

set -e

echo "======================================================================"
echo "Running SupraBTM on Williams' Block Range (18000000-18000499)"
echo "======================================================================"
echo ""

# Check Docker
DOCKER="/Applications/Docker.app/Contents/Resources/bin/docker"
if [ ! -f "$DOCKER" ]; then
    echo "Error: Docker Desktop not found"
    exit 1
fi

if ! $DOCKER ps &> /dev/null; then
    echo "Error: Docker daemon not running. Please start Docker Desktop."
    exit 1
fi

echo "Pulling SupraBTM Docker image..."
$DOCKER pull rohitkapoor9312/ibtm-image:latest

echo ""
echo "Running SupraBTM on 500 blocks (18000000-18000499)..."
echo "Start time: $(date)"
START_TIME=$(date +%s)

# Create output directory
SUPRABTM_OUT="$PWD/suprabtm_williams_range"
mkdir -p "$SUPRABTM_OUT"

# Run SupraBTM
$DOCKER run --rm \
  --cpuset-cpus="0-13" \
  -v "$PWD/data_100k:/data" \
  -v "$SUPRABTM_OUT:/out" \
  rohitkapoor9312/ibtm-image:latest \
  --data-dir /data \
  --output-dir /out \
  --inmemory

END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo ""
echo "SupraBTM complete!"
echo "Execution time: ${ELAPSED}s"
echo ""
echo "Results saved to: $SUPRABTM_OUT/"
echo ""
echo "Now run the comparison:"
echo "  python3 compare_same_blocks.py"
echo ""
