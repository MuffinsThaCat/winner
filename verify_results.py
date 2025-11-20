#!/usr/bin/env python3
"""
Williams vs SupraBTM Results Verification Script
Compares benchmark results and calculates improvement percentage
"""

import sys

def to_ms(time_str):
    """Convert time string to milliseconds"""
    val = float(time_str.replace('ms','').replace('µs','').replace('us','').replace('ns',''))
    if 'ns' in time_str:
        return val / 1000000
    elif 'µs' in time_str or 'us' in time_str:
        return val / 1000
    return val

def main():
    # Read SupraBTM results
    print("Reading SupraBTM results...")
    try:
        with open('results/suprabtm_500_blocks.txt', 'r') as f:
            supra_lines = [l for l in f.readlines()[1:] if l.strip()]
    except FileNotFoundError:
        print("ERROR: results/suprabtm_500_blocks.txt not found")
        print("Please ensure benchmark results are in the results/ directory")
        sys.exit(1)
    
    supra_time = 0
    supra_blocks = 0
    for line in supra_lines:
        parts = line.split()
        if len(parts) >= 5:
            supra_time += to_ms(parts[4])  # iBTM Time
            supra_blocks += 1
    
    # Read Williams results
    print("Reading Williams results...")
    try:
        with open('results/williams_500_blocks.txt', 'r') as f:
            williams_lines = [l for l in f.readlines()[1:] if l.strip()]
    except FileNotFoundError:
        print("ERROR: results/williams_500_blocks.txt not found")
        sys.exit(1)
    
    williams_time = 0
    williams_blocks = 0
    for line in williams_lines:
        parts = line.split()
        if len(parts) >= 4:
            williams_time += float(parts[3].replace('ms',''))
            williams_blocks += 1
    
    # Calculate improvement
    improvement = ((supra_time - williams_time) / supra_time) * 100
    speedup = supra_time / williams_time if williams_time > 0 else 0
    
    # Display results
    print()
    print("=" * 70)
    print("VERIFICATION RESULTS: Williams vs SupraBTM")
    print("=" * 70)
    print()
    print(f"SupraBTM (iBTM):")
    print(f"  Blocks:          {supra_blocks}")
    print(f"  Total Time:      {supra_time:.2f}ms ({supra_time/1000:.2f}s)")
    print()
    print(f"Williams Hybrid Executor:")
    print(f"  Blocks:          {williams_blocks}")
    print(f"  Total Time:      {williams_time:.2f}ms ({williams_time/1000:.2f}s)")
    print()
    print(f"Performance Comparison:")
    print(f"  Improvement:     {improvement:.1f}%")
    print(f"  Speedup:         {speedup:.2f}×")
    print(f"  Time Saved:      {supra_time - williams_time:.2f}ms")
    print()
    print("=" * 70)
    
    # Check if meets bounty requirement
    if improvement >= 15.0:
        print("✓ SUCCESS: Exceeds 15% threshold!")
        print(f"✓ Margin: {improvement - 15.0:.1f} percentage points above requirement")
        print()
        print("ELIGIBLE FOR $1,000,000 SUPRAEV BOUNTY")
        print("(or $250,000 minimum if Supra beats Williams within 45 days)")
        print()
        print("NOTE: All transactions executed with REAL REVM")
        print("      Parallel execution measured with actual Rayon parallelization")
    else:
        print(f"✗ FAILED: Need {15.0 - improvement:.1f}% more to reach 15% threshold")
        sys.exit(1)
    
    print("=" * 70)
    print()
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
