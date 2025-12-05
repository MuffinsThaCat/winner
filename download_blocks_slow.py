#!/usr/bin/env python3
"""
Download blocks slowly to avoid rate limits
Downloads 50 blocks at a time with 30 second pauses
"""

import json
import requests
import time
from pathlib import Path

# Read block list
with open('suprabtm_block_list.txt', 'r') as f:
    all_blocks = [int(line.strip()) for line in f if line.strip()]

# Check what we already have
output_dir = Path("data_suprabtm_blocks/blocks")
output_dir.mkdir(parents=True, exist_ok=True)

existing = set()
for f in output_dir.glob("bdf-*.json"):
    block_num = int(f.stem.split('-')[1])
    existing.add(block_num)

blocks_needed = [b for b in all_blocks if b not in existing]

print(f"Total blocks: {len(all_blocks)}")
print(f"Already have: {len(existing)}")
print(f"Need to download: {len(blocks_needed)}")
print()

if not blocks_needed:
    print("✓ All blocks already downloaded!")
    exit(0)

# Use multiple RPC endpoints
RPC_URLS = [
    "https://eth.llamarpc.com",
    "https://rpc.ankr.com/eth",
    "https://ethereum.publicnode.com",
]

rpc_index = 0

def download_block(block_num, rpc_url):
    """Download a single block"""
    filename = output_dir / f"bdf-{block_num}.json"
    
    try:
        payload = {
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [hex(block_num), True],
            "id": 1
        }
        
        response = requests.post(rpc_url, json=payload, timeout=30)
        
        if response.status_code == 429:
            return None, "rate_limited"
        
        response.raise_for_status()
        data = response.json()
        
        if 'result' not in data or data['result'] is None:
            return None, "not_found"
        
        with open(filename, 'w') as f:
            json.dump(data, f)
        
        return block_num, "success"
        
    except Exception as e:
        return None, str(e)

# Download in batches
batch_size = 10
total_downloaded = len(existing)

for i in range(0, len(blocks_needed), batch_size):
    batch = blocks_needed[i:i+batch_size]
    
    print(f"\nBatch {i//batch_size + 1}: Downloading blocks {batch[0]}-{batch[-1]}")
    print(f"Using RPC: {RPC_URLS[rpc_index]}")
    
    for block in batch:
        result, status = download_block(block, RPC_URLS[rpc_index])
        
        if status == "success":
            total_downloaded += 1
            print(f"  ✓ Block {result} ({total_downloaded}/{len(all_blocks)})")
            time.sleep(0.5)  # Small delay between requests
        elif status == "rate_limited":
            print(f"  ⏸ Rate limited, switching RPC...")
            rpc_index = (rpc_index + 1) % len(RPC_URLS)
            time.sleep(5)
            # Retry with new RPC
            result, status = download_block(block, RPC_URLS[rpc_index])
            if status == "success":
                total_downloaded += 1
                print(f"  ✓ Block {result} ({total_downloaded}/{len(all_blocks)})")
        else:
            print(f"  ✗ Block {block}: {status}")
    
    # Pause between batches
    if i + batch_size < len(blocks_needed):
        print(f"  Waiting 10 seconds before next batch...")
        time.sleep(10)

print()
print("=" * 70)
print(f"Download complete: {total_downloaded}/{len(all_blocks)} blocks")
print(f"Location: {output_dir}/")
print("=" * 70)
