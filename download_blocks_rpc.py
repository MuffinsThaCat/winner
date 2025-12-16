#!/usr/bin/env python3
"""
Download blocks via Ethereum JSON-RPC
Uses a public RPC endpoint (or your own)
"""

import json
import requests
import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

# Read block list
with open('suprabtm_block_list.txt', 'r') as f:
    blocks = [int(line.strip()) for line in f if line.strip()]

print(f"Need to download {len(blocks)} blocks")
print(f"Range: {min(blocks)} to {max(blocks)}")
print()

# Create output directory
output_dir = Path("data_suprabtm_blocks/blocks")
output_dir.mkdir(parents=True, exist_ok=True)

# RPC endpoints (you can use your own or public ones)
RPC_URLS = [
    "https://eth.llamarpc.com",
    "https://rpc.ankr.com/eth",
    "https://ethereum.publicnode.com",
    "https://1rpc.io/eth",
]

def download_block(block_num, rpc_url):
    """Download a single block with full transactions"""
    filename = output_dir / f"bdf-{block_num}.json"
    
    # Skip if already exists
    if filename.exists():
        return block_num, "exists"
    
    try:
        # Get block with full transaction details
        payload = {
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [hex(block_num), True],  # True = full tx details
            "id": 1
        }
        
        response = requests.post(rpc_url, json=payload, timeout=30)
        response.raise_for_status()
        
        data = response.json()
        
        if 'result' not in data or data['result'] is None:
            return block_num, f"error: block not found"
        
        # Save the response
        with open(filename, 'w') as f:
            json.dump(data, f)
        
        return block_num, "success"
        
    except Exception as e:
        return block_num, f"error: {str(e)}"

# Download blocks in parallel
print(f"Downloading from: {RPC_URLS[0]}")
print("This may take 10-30 minutes depending on RPC rate limits...")
print()

downloaded = 0
errors = []
rpc_index = 0

with ThreadPoolExecutor(max_workers=10) as executor:
    # Submit all download tasks
    futures = {
        executor.submit(download_block, block, RPC_URLS[rpc_index % len(RPC_URLS)]): block 
        for block in blocks
    }
    
    # Process as they complete
    for future in as_completed(futures):
        block_num = futures[future]
        block_num_result, status = future.result()
        
        if status == "success":
            downloaded += 1
            if downloaded % 10 == 0:
                print(f"Progress: {downloaded}/{len(blocks)} blocks downloaded ({downloaded*100//len(blocks)}%)")
        elif status == "exists":
            downloaded += 1
        else:
            errors.append((block_num_result, status))
            print(f"✗ Block {block_num_result}: {status}")
        
        # Rate limiting
        time.sleep(0.1)

print()
print("=" * 70)
print(f"✓ Downloaded: {downloaded}/{len(blocks)} blocks")
if errors:
    print(f"✗ Errors: {len(errors)}")
    for block, error in errors[:10]:
        print(f"  Block {block}: {error}")
print()
print("Next step:")
print("  cd williams_revm_complete")
print("  ./target/release/williams-complete ../data_suprabtm_blocks 16")
print("=" * 70)
