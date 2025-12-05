#!/usr/bin/env python3
"""Quick proof: Williams vs SupraBTM throughput comparison"""

# Williams results (blocks 18000000-18000499)
williams_txs = 0
williams_time_ms = 0

with open('results/williams_500_blocks.txt', 'r') as f:
    for line in f:
        parts = line.strip().split()
        if len(parts) >= 4 and parts[0].isdigit():
            block = int(parts[0])
            if 18000000 <= block <= 18000499:
                williams_txs += int(parts[2])
                williams_time_ms += float(parts[3].replace('ms', ''))

# SupraBTM results (their published 500 blocks)
supra_txs = 0
supra_time_ms = 0.0

with open('results/suprabtm_500_blocks.txt', 'r') as f:
    next(f)  # Skip header
    for line in f:
        parts = line.strip().split('\t')
        if len(parts) >= 5:
            supra_txs += int(parts[2])
            time_str = parts[4]
            if 'ms' in time_str:
                supra_time_ms += float(time_str.replace('ms', ''))
            elif 'µs' in time_str:
                supra_time_ms += float(time_str.replace('µs', '')) / 1000.0
            elif 'ns' in time_str:
                supra_time_ms += float(time_str.replace('ns', '')) / 1000000.0

williams_tps = (williams_txs / williams_time_ms) * 1000
supra_tps = (supra_txs / supra_time_ms) * 1000

print("=" * 70)
print("PROOF: WILLIAMS vs SupraBTM")
print("=" * 70)
print()
print("WILLIAMS (500 blocks, 18000000-18000499):")
print(f"  Transactions:  {williams_txs:,}")
print(f"  Time:          {williams_time_ms:.2f}ms ({williams_time_ms/1000:.2f}s)")
print(f"  Throughput:    {williams_tps:,.0f} tx/s")
print()
print("SupraBTM (500 blocks, published results):")
print(f"  Transactions:  {supra_txs:,}")
print(f"  Time:          {supra_time_ms:.2f}ms ({supra_time_ms/1000:.2f}s)")
print(f"  Throughput:    {supra_tps:,.0f} tx/s")
print()
print("=" * 70)
print("RESULT:")
print("=" * 70)

if williams_tps > supra_tps:
    improvement = ((williams_tps - supra_tps) / supra_tps) * 100
    print(f"✓ WILLIAMS WINS: +{improvement:.1f}% faster")
    print(f"  {williams_tps:,.0f} tx/s vs {supra_tps:,.0f} tx/s")
else:
    diff = ((supra_tps - williams_tps) / supra_tps) * 100
    print(f"✗ SupraBTM faster: +{diff:.1f}%")
    print(f"  {supra_tps:,.0f} tx/s vs {williams_tps:,.0f} tx/s")

print()
print("Note: Different block sets due to data availability.")
print("Williams optimized for blocks 18M+, SupraBTM tested on blocks 14M/21M.")
print("=" * 70)
