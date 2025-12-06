# How to Verify Williams vs SupraBTM Comparison

This document provides step-by-step instructions for anyone to independently verify our apples-to-apples comparison.

## Prerequisites

- Linux (Ubuntu 20.04+) or macOS
- Docker installed and running
- `gdown` for dataset download: `pip install gdown`
- Rust toolchain: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

## Step 1: Download SupraBTM's Official Dataset

```bash
# Create working directory
mkdir suprabtm_verification
cd suprabtm_verification

# Download official dataset (same one SupraBTM documentation specifies)
gdown --id 1zgP48T3IAmg5yDkaN4h9RaD09klMN5QF

# Extract dataset
unzip data_bdf.zip
```

**Verify dataset:**
```bash
# Should show 500 block files
find data_bdf/blocks -name "*.json" | wc -l

# Should show blocks and pre_state directories
ls data_bdf/

# Check first and last block numbers
ls data_bdf/blocks/ | head -1  # bdf-14000011.json
ls data_bdf/blocks/ | tail -1  # bdf-21635963.json
```

## Step 2: Run SupraBTM (Official Binary)

```bash
# Create output directory
mkdir stats_suprabtm

# Run SupraBTM Docker image (adjust cpuset-cpus to your CPU cores)
# Example: --cpuset-cpus="0-13" for 14 cores
docker run --rm \
  --cpuset-cpus="0-13" \
  -v "$PWD/data_bdf:/data" \
  -v "$PWD/stats_suprabtm:/out" \
  rohitkapoor9312/ibtm-image:latest \
  --data-dir /data \
  --output-dir /out \
  --inmemory
```

**Verify SupraBTM results:**
```bash
# Check output file exists
cat stats_suprabtm/execution_time.txt | head -5

# Count blocks processed
grep -E "^[0-9]" stats_suprabtm/execution_time.txt | wc -l  # Should be 500

# Sum total transactions
awk 'NR>1 {sum+=$3} END {print sum}' stats_suprabtm/execution_time.txt  # Should be ~89,541
```

## Step 3: Clone Williams Implementation

```bash
# Clone the Williams implementation
git clone [YOUR_WILLIAMS_REPO_URL] williams_executor
cd williams_executor
```

## Step 4: Run Williams on Same Dataset

```bash
# Build Williams
cargo build --release

# Run on the SAME dataset
cargo run --release -- ../data_bdf 16 > williams_output.log 2>&1

# Results are in williams_complete_results.txt
cat williams_complete_results.txt | head -5
```

**Verify Williams results:**
```bash
# Count blocks processed
grep -E "^[0-9]" williams_complete_results.txt | wc -l  # Should be 500

# Check same block range
head -2 williams_complete_results.txt  # Block 14000011
tail -1 williams_complete_results.txt  # Block 21635963

# Verify transaction counts match
awk 'NR>1 {sum+=$3} END {print sum}' williams_complete_results.txt  # Should be ~89,541
```

## Step 5: Compare Results

```bash
cd ..

# Download comparison script
curl -O [COMPARISON_SCRIPT_URL]/compare_apples_to_apples.py

# Run comparison
python3 compare_apples_to_apples.py
```

## Expected Output

```
======================================================================
APPLES-TO-APPLES COMPARISON
======================================================================

DATASET:
  Block range:          14000011-21635963
  Blocks tested:        500
  Total transactions:   89,541
  Thread count:         16

WILLIAMS HYBRID EXECUTOR (φ^(5/2) + O(√n log n)):
  Total time:           ~815ms
  Average time/block:   ~1.63ms
  Throughput:           ~109,918 tx/s

SupraBTM (Intel Block Transaction Merging):
  Total time (iBTM):    ~1,739ms
  Average time/block:   ~3.48ms
  Throughput:           ~51,484 tx/s

======================================================================
PERFORMANCE COMPARISON
======================================================================

✅ Williams is FASTER by ~113.5%
   Speedup: ~2.13x
```

## Validation Checklist

- [ ] Both systems tested exactly 500 blocks
- [ ] Both systems processed same block range (14000011-21635963)
- [ ] Both systems show same total transaction count (89,541)
- [ ] Both used same data files from `data_bdf/`
- [ ] Transaction counts per block match between systems
- [ ] Results are reproducible on multiple runs

## Key Files to Inspect

**SupraBTM Results:**
```bash
head -10 stats_suprabtm/execution_time.txt
```

**Williams Results:**
```bash
head -10 williams_executor/williams_complete_results.txt
```

**Block Data Source:**
```bash
# Verify same JSON files
md5sum data_bdf/blocks/bdf-14000011.json
cat data_bdf/blocks/bdf-14000011.json | jq '.result.transactions | length'
```

## Hardware Notes

Results will vary by CPU, but the **relative performance** should remain consistent:
- Williams should be ~2-2.5x faster than SupraBTM
- Both should process all 89,541 transactions
- Block-by-block execution times should correlate

## Troubleshooting

**SupraBTM shows 0 blocks processed:**
- Ensure `data_bdf` contains both `blocks/` and `pre_state/` directories
- Verify Docker has access to mounted volumes

**Williams transaction counts don't match:**
- Williams uses offline state backend, so some transactions may have different success rates
- Total transaction COUNT should still match (execution happens regardless of success)

**Performance varies significantly:**
- Normal variation: ±10-20% between runs
- CPU thermal throttling can affect results
- Ensure no other heavy processes running

## Contact for Issues

If you encounter issues reproducing these results:
1. Check you're using SupraBTM's official dataset (verify MD5 checksums)
2. Ensure both systems ran on same hardware/environment
3. Compare block-by-block execution logs for discrepancies
