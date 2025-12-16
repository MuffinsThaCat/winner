#!/usr/bin/env python3
"""
Download the exact blocks that SupraBTM tested (500 blocks from ranges 14M and 21M)
Uses public Ethereum dataset from Google BigQuery
"""

import json
import os
import subprocess
import sys
from pathlib import Path

# Read the block list
with open('suprabtm_block_list.txt', 'r') as f:
    blocks = [int(line.strip()) for line in f if line.strip()]

print(f"Found {len(blocks)} blocks to download")
print(f"Range: {min(blocks)} to {max(blocks)}")
print()

# Create output directory
output_dir = Path("data_suprabtm_blocks/blocks")
output_dir.mkdir(parents=True, exist_ok=True)

print("Downloading blocks from Google BigQuery...")
print("This requires 'gcloud' and 'bq' command-line tools")
print()

# Check if bq is available
try:
    subprocess.run(['bq', 'version'], capture_output=True, check=True)
except (FileNotFoundError, subprocess.CalledProcessError):
    print("ERROR: 'bq' command not found!")
    print()
    print("Install Google Cloud SDK:")
    print("  brew install google-cloud-sdk")
    print("  gcloud init")
    print("  gcloud auth application-default login")
    sys.exit(1)

# Download blocks in batches
batch_size = 50
for i in range(0, len(blocks), batch_size):
    batch = blocks[i:i+batch_size]
    block_list = ','.join(str(b) for b in batch)
    
    print(f"Downloading batch {i//batch_size + 1}/{(len(blocks)-1)//batch_size + 1}: blocks {batch[0]}-{batch[-1]}")
    
    query = f"""
    SELECT 
        number,
        hash,
        parent_hash,
        nonce,
        sha3_uncles,
        logs_bloom,
        transactions_root,
        state_root,
        receipts_root,
        miner,
        difficulty,
        total_difficulty,
        size,
        extra_data,
        gas_limit,
        gas_used,
        timestamp,
        transaction_count,
        base_fee_per_gas,
        (SELECT ARRAY_AGG(
            STRUCT(
                hash,
                nonce,
                transaction_index,
                from_address,
                to_address,
                value,
                gas,
                gas_price,
                input,
                receipt_cumulative_gas_used,
                receipt_gas_used,
                receipt_contract_address,
                receipt_root,
                receipt_status,
                block_timestamp,
                block_number,
                block_hash,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                transaction_type,
                receipt_effective_gas_price
            )
        ) FROM `bigquery-public-data.crypto_ethereum.transactions` 
        WHERE block_number = b.number) as transactions
    FROM `bigquery-public-data.crypto_ethereum.blocks` b
    WHERE number IN ({block_list})
    ORDER BY number
    """
    
    # Run query and save results
    try:
        result = subprocess.run(
            ['bq', 'query', '--use_legacy_sql=false', '--format=json', query],
            capture_output=True,
            text=True,
            check=True
        )
        
        # Parse and save each block
        data = json.loads(result.stdout)
        for block in data:
            block_num = block['number']
            filename = output_dir / f"bdf-{block_num}.json"
            
            # Convert to expected format
            formatted = {
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "number": hex(int(block['number'])),
                    "hash": block['hash'],
                    "parentHash": block['parent_hash'],
                    "nonce": block['nonce'],
                    "sha3Uncles": block['sha3_uncles'],
                    "logsBloom": block['logs_bloom'],
                    "transactionsRoot": block['transactions_root'],
                    "stateRoot": block['state_root'],
                    "receiptsRoot": block['receipts_root'],
                    "miner": block['miner'],
                    "difficulty": hex(int(block['difficulty'])),
                    "gasLimit": hex(int(block['gas_limit'])),
                    "gasUsed": hex(int(block['gas_used'])),
                    "timestamp": hex(int(block['timestamp'])),
                    "extraData": block['extra_data'],
                    "size": hex(int(block['size'])),
                    "baseFeePerGas": hex(int(block['base_fee_per_gas'])) if block.get('base_fee_per_gas') else None,
                    "transactions": block.get('transactions', [])
                }
            }
            
            with open(filename, 'w') as f:
                json.dump(formatted, f)
            
            print(f"  ✓ Block {block_num} saved")
            
    except subprocess.CalledProcessError as e:
        print(f"ERROR downloading batch: {e}")
        print(e.stderr)
        sys.exit(1)

print()
print(f"✓ Downloaded {len(blocks)} blocks to {output_dir}")
print()
print("Next steps:")
print(f"  1. Run Williams: cd williams_revm_complete && ./target/release/williams-complete ../data_suprabtm_blocks 16")
print(f"  2. Compare results with SupraBTM's published data")
