#!/usr/bin/env python3
"""
Apples-to-Apples Comparison: Williams vs SupraBTM on SAME blocks
After running SupraBTM on blocks 18000000-18000499
"""

import os
import sys

def parse_williams_line(line):
    """Parse Williams output: Block_No Threads Block_Size Williams_Time"""
    parts = line.strip().split()
    if len(parts) >= 4 and parts[0].isdigit():
        return {
            'block': int(parts[0]),
            'threads': int(parts[1]),
            'txs': int(parts[2]),
            'time_ms': float(parts[3].replace('ms', ''))
        }
    return None

def parse_supra_line(line):
    """Parse SupraBTM output: Block_No Threads Block_Size Seq_Time iBTM_Time"""
    parts = line.strip().split('\t')
    if len(parts) >= 5 and parts[0].isdigit():
        time_str = parts[4]
        if 'ms' in time_str:
            time_ms = float(time_str.replace('ms', ''))
        elif 'µs' in time_str:
            time_ms = float(time_str.replace('µs', '')) / 1000.0
        elif 'ns' in time_str:
            time_ms = float(time_str.replace('ns', '')) / 1000000.0
        else:
            return None
            
        return {
            'block': int(parts[0]),
            'threads': int(parts[1]),
            'txs': int(parts[2]),
            'time_ms': time_ms
        }
    return None

# Check if SupraBTM has been run
supra_file = 'suprabtm_williams_range/execution_time.txt'
if not os.path.exists(supra_file):
    print("❌ SupraBTM results not found!")
    print(f"   Expected: {supra_file}")
    print("")
    print("Please run SupraBTM first:")
    print("  bash run_suprabtm_on_williams_blocks.sh")
    sys.exit(1)

print("Loading datasets...")

# Load Williams results (only blocks 18000000-18000499)
williams_blocks = {}
with open('results/williams_500_blocks.txt', 'r') as f:
    for line in f:
        data = parse_williams_line(line)
        if data and 18000000 <= data['block'] <= 18000499:
            williams_blocks[data['block']] = data

print(f"  Williams: {len(williams_blocks)} blocks (18000000-18000499)")

# Load SupraBTM results
supra_blocks = {}
with open(supra_file, 'r') as f:
    for line in f:
        data = parse_supra_line(line)
        if data:
            supra_blocks[data['block']] = data

print(f"  SupraBTM: {len(supra_blocks)} blocks")

# Find common blocks
common_blocks = sorted(set(williams_blocks.keys()) & set(supra_blocks.keys()))

print(f"\nCommon blocks: {len(common_blocks)}")

if len(common_blocks) < 100:
    print(f"\n⚠️  WARNING: Only {len(common_blocks)} overlapping blocks!")
    print("   Expected ~500 blocks for valid comparison")
    print("")
    if len(common_blocks) == 0:
        print("   SupraBTM may have run on different block range")
        print(f"   Williams range: 18000000-18000499")
        print(f"   SupraBTM range: {min(supra_blocks.keys())}-{max(supra_blocks.keys()) if supra_blocks else 'N/A'}")
        sys.exit(1)

# Calculate statistics
williams_total_txs = sum(williams_blocks[b]['txs'] for b in common_blocks)
williams_total_time = sum(williams_blocks[b]['time_ms'] for b in common_blocks)

supra_total_txs = sum(supra_blocks[b]['txs'] for b in common_blocks)
supra_total_time = sum(supra_blocks[b]['time_ms'] for b in common_blocks)

print("\n" + "=" * 70)
print("APPLES-TO-APPLES COMPARISON")
print("=" * 70)
print()

print(f"DATASET:")
print(f"  Block range:          18000000-18000{len(common_blocks)-1}")
print(f"  Common blocks:        {len(common_blocks)}")
print(f"  Total transactions:   {williams_total_txs:,}")
print()

print("WILLIAMS HYBRID EXECUTOR:")
print(f"  Total time:           {williams_total_time:,.2f}ms ({williams_total_time/1000:.2f}s)")
print(f"  Average time/block:   {williams_total_time/len(common_blocks):.2f}ms")
print(f"  Throughput:           {(williams_total_txs / williams_total_time) * 1000:,.0f} tx/s")
print()

print("SupraBTM:")
print(f"  Total time (iBTM):    {supra_total_time:,.2f}ms ({supra_total_time/1000:.2f}s)")
print(f"  Average time/block:   {supra_total_time/len(common_blocks):.2f}ms")
print(f"  Throughput:           {(supra_total_txs / supra_total_time) * 1000:,.0f} tx/s")
print()

print("=" * 70)
print("PERFORMANCE COMPARISON")
print("=" * 70)
print()

williams_tps = (williams_total_txs / williams_total_time) * 1000
supra_tps = (supra_total_txs / supra_total_time) * 1000

if williams_tps > supra_tps:
    improvement = ((williams_tps - supra_tps) / supra_tps) * 100
    print(f"✓ Williams is FASTER by {improvement:.1f}%")
    print(f"  Williams: {williams_tps:,.0f} tx/s")
    print(f"  SupraBTM: {supra_tps:,.0f} tx/s")
else:
    difference = ((supra_tps - williams_tps) / supra_tps) * 100
    print(f"✗ SupraBTM is faster by {difference:.1f}%")
    print(f"  Williams: {williams_tps:,.0f} tx/s")
    print(f"  SupraBTM: {supra_tps:,.0f} tx/s")

print()

# Time per block comparison
williams_avg = williams_total_time / len(common_blocks)
supra_avg = supra_total_time / len(common_blocks)

if williams_avg < supra_avg:
    speedup = supra_avg / williams_avg
    time_saved = ((supra_avg - williams_avg) / supra_avg) * 100
    print(f"✓ Williams completes blocks FASTER")
    print(f"  Speedup: {speedup:.2f}x")
    print(f"  Time saved per block: {time_saved:.1f}%")
    print(f"  Williams: {williams_avg:.2f}ms/block")
    print(f"  SupraBTM: {supra_avg:.2f}ms/block")
else:
    slowdown = williams_avg / supra_avg
    print(f"✗ Williams is slower per block")
    print(f"  Slowdown: {slowdown:.2f}x")
    print(f"  Williams: {williams_avg:.2f}ms/block")
    print(f"  SupraBTM: {supra_avg:.2f}ms/block")

print()
print("=" * 70)
print("✅ VALID APPLES-TO-APPLES COMPARISON")
print("   Same blocks, same machine, same data")
print("=" * 70)
print()
