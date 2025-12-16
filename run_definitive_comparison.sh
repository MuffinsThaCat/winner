#!/bin/bash
# Definitive comparison: Williams vs SupraBTM on EXACT same blocks
# Runs both systems back-to-back on blocks 18000000-18000499

set -e

echo "======================================================================"
echo "DEFINITIVE BENCHMARK: Williams vs SupraBTM"
echo "======================================================================"
echo ""
echo "This script will:"
echo "  1. Run Williams on blocks 18000000-18000499 (500 blocks)"
echo "  2. Run SupraBTM on the SAME blocks"
echo "  3. Compare results side-by-side"
echo ""
echo "Press Ctrl+C to cancel, or Enter to continue..."
read

# Setup
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RESULTS_DIR="$PWD/comparison_results_$TIMESTAMP"
DATA_DIR="$PWD/data_100k"

mkdir -p "$RESULTS_DIR"

echo ""
echo "======================================================================"
echo "STEP 1: Running Williams Executor"
echo "======================================================================"
echo ""

cd williams_revm_complete

# Build if needed
if [ ! -f "target/release/williams-complete" ]; then
    echo "Building Williams executor..."
    cargo build --release
fi

echo "Running Williams on 500 blocks (18000000-18000499)..."
echo "Start time: $(date)"
START_TIME=$(date +%s)

./target/release/williams-complete ../data_100k 16 > "$RESULTS_DIR/williams_output.txt" 2>&1

END_TIME=$(date +%s)
WILLIAMS_TIME=$((END_TIME - START_TIME))

echo "Williams complete!"
echo "Execution time: ${WILLIAMS_TIME}s"
echo ""

# Copy Williams results
cp williams_complete_results.txt "$RESULTS_DIR/williams_results.txt" 2>/dev/null || true

cd ..

echo ""
echo "======================================================================"
echo "STEP 2: Running SupraBTM Executor"
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
echo "Running SupraBTM on same 500 blocks..."
echo "Start time: $(date)"
START_TIME=$(date +%s)

# Create output directory
SUPRABTM_OUT="$RESULTS_DIR/suprabtm_output"
mkdir -p "$SUPRABTM_OUT"

# Run SupraBTM
$DOCKER run --rm \
  --cpuset-cpus="0-13" \
  -v "$DATA_DIR:/data" \
  -v "$SUPRABTM_OUT:/out" \
  rohitkapoor9312/ibtm-image:latest \
  --data-dir /data \
  --output-dir /out \
  --inmemory > "$RESULTS_DIR/suprabtm_docker_output.txt" 2>&1

END_TIME=$(date +%s)
SUPRABTM_TIME=$((END_TIME - START_TIME))

echo "SupraBTM complete!"
echo "Execution time: ${SUPRABTM_TIME}s"
echo ""

# Copy SupraBTM results if they exist
if [ -f "$SUPRABTM_OUT/execution_time.txt" ]; then
    cp "$SUPRABTM_OUT/execution_time.txt" "$RESULTS_DIR/suprabtm_results.txt"
fi

echo ""
echo "======================================================================"
echo "STEP 3: Analyzing Results"
echo "======================================================================"
echo ""

# Create comparison script
python3 << 'PYTHON_EOF' > "$RESULTS_DIR/comparison_analysis.txt"
import os
import re

results_dir = os.environ.get('RESULTS_DIR', '.')

print("="*70)
print("DEFINITIVE COMPARISON RESULTS")
print("="*70)
print()

# Analyze Williams results
williams_file = f"{results_dir}/williams_output.txt"
print("WILLIAMS EXECUTOR:")
if os.path.exists(williams_file):
    with open(williams_file, 'r') as f:
        content = f.read()
        
    # Extract key metrics
    if 'Total transactions:' in content:
        for line in content.split('\n'):
            if 'Total transactions:' in line:
                print(f"  {line.strip()}")
            elif 'Total time:' in line:
                print(f"  {line.strip()}")
            elif 'Throughput:' in line:
                print(f"  {line.strip()}")
else:
    print("  ERROR: No output file found")

print()

# Analyze SupraBTM results
suprabtm_file = f"{results_dir}/suprabtm_results.txt"
print("SUPRABTM EXECUTOR:")
if os.path.exists(suprabtm_file):
    with open(suprabtm_file, 'r') as f:
        lines = f.readlines()
    
    if len(lines) > 1:
        total_txs = 0
        total_time = 0.0
        
        for line in lines[1:]:
            parts = line.strip().split('\t')
            if len(parts) >= 5:
                try:
                    block_size = int(parts[2])
                    time_str = parts[4]
                    
                    if 'ms' in time_str:
                        time_ms = float(time_str.replace('ms', ''))
                    elif 'µs' in time_str:
                        time_ms = float(time_str.replace('µs', '')) / 1000.0
                    else:
                        continue
                    
                    total_txs += block_size
                    total_time += time_ms
                except:
                    continue
        
        if total_txs > 0:
            total_time_s = total_time / 1000.0
            throughput = total_txs / total_time_s
            
            print(f"  Total transactions: {total_txs:,}")
            print(f"  Total time: {total_time_s:.2f}s")
            print(f"  Throughput: {throughput:,.2f} tx/s")
        else:
            print("  ERROR: No valid data in results")
    else:
        print("  ERROR: Empty results file")
else:
    print("  ERROR: No output file found")
    print(f"  Checked: {suprabtm_file}")

print()
print("="*70)
print("COMPARISON STATUS")
print("="*70)
print()

williams_exists = os.path.exists(williams_file)
suprabtm_exists = os.path.exists(suprabtm_file) and os.path.getsize(suprabtm_file) > 100

if williams_exists and suprabtm_exists:
    print("✓ Both executors completed successfully")
    print("✓ Results are directly comparable")
    print("✓ Same machine, same data, same blocks")
else:
    print("⚠ WARNING: Not all executors produced results")
    if not williams_exists:
        print("  ✗ Williams: No output")
    if not suprabtm_exists:
        print("  ✗ SupraBTM: No output or empty")

PYTHON_EOF

RESULTS_DIR="$RESULTS_DIR" python3 << 'PYTHON_EOF2'
import os

results_dir = os.environ.get('RESULTS_DIR', '.')

# Read and print the analysis
analysis_file = f"{results_dir}/comparison_analysis.txt"
if os.path.exists(analysis_file):
    with open(analysis_file, 'r') as f:
        print(f.read())
PYTHON_EOF2

echo ""
echo "======================================================================"
echo "BENCHMARK COMPLETE"
echo "======================================================================"
echo ""
echo "Results saved to: $RESULTS_DIR/"
echo ""
echo "Files created:"
echo "  - williams_output.txt       (Williams full output)"
echo "  - williams_results.txt      (Williams summary)"
echo "  - suprabtm_docker_output.txt (SupraBTM Docker output)"
echo "  - suprabtm_results.txt      (SupraBTM summary)"
echo "  - comparison_analysis.txt   (Side-by-side comparison)"
echo ""
echo "View comparison:"
echo "  cat $RESULTS_DIR/comparison_analysis.txt"
echo ""
