#!/bin/bash
# Definitive Williams Benchmark - 100 blocks for statistical significance

set -e

echo "======================================================================"
echo "DEFINITIVE BENCHMARK: Williams vs SupraBTM"
echo "======================================================================"
echo ""
echo "Running Williams on 100 blocks for statistical significance..."
echo ""

cd williams_revm_complete

# Modify to test 100 blocks
time ./target/release/williams-complete ../data_100k 16 2>&1 | tee ../definitive_benchmark_output.txt

cd ..

echo ""
echo "======================================================================"
echo "EXTRACTING RESULTS"
echo "======================================================================"
echo ""

# Extract summary
grep -A 10 "EXECUTION SUMMARY" definitive_benchmark_output.txt

echo ""
echo "Results saved to:"
echo "  - definitive_benchmark_output.txt (full output)"
echo "  - williams_revm_complete/williams_complete_results.txt (data)"
echo ""
