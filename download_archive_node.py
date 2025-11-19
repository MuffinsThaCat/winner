#!/usr/bin/env python3
"""
Download 100K Ethereum blocks from Archive Node
Supports: Infura, Alchemy, QuickNode, or local node
"""

import requests
import json
import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
from tqdm import tqdm
import argparse

# Configuration
DEFAULT_START_BLOCK = 18000000  # Recent blocks with good activity
DEFAULT_NUM_BLOCKS = 100000
DEFAULT_OUTPUT_DIR = Path("./data_100k")
DEFAULT_WORKERS = 20  # Parallel download threads

# Archive node endpoints (add your API keys)
ENDPOINTS = {
    "infura": "https://mainnet.infura.io/v3/YOUR_API_KEY",
    "alchemy": "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY",
    "quicknode": "https://YOUR_ENDPOINT.quiknode.pro/YOUR_API_KEY",
    "local": "http://localhost:8545"
}

class BlockDownloader:
    def __init__(self, rpc_url, output_dir, start_block, num_blocks, workers=20):
        self.rpc_url = rpc_url
        self.output_dir = Path(output_dir)
        self.start_block = start_block
        self.num_blocks = num_blocks
        self.workers = workers
        
        # Create output directories
        self.blocks_dir = self.output_dir / "blocks"
        self.blocks_dir.mkdir(parents=True, exist_ok=True)
        
        # Stats
        self.downloaded = 0
        self.failed = []
        self.total_txs = 0
        
    def rpc_call(self, method, params):
        """Make JSON-RPC call to Ethereum node"""
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        }
        
        try:
            response = requests.post(
                self.rpc_url,
                json=payload,
                headers={"Content-Type": "application/json"},
                timeout=30
            )
            response.raise_for_status()
            result = response.json()
            
            if "error" in result:
                raise Exception(f"RPC Error: {result['error']}")
            
            return result.get("result")
            
        except Exception as e:
            raise Exception(f"RPC call failed: {e}")
    
    def download_block(self, block_number):
        """Download a single block with full transaction data"""
        try:
            # Check if file already exists and is valid
            filename = self.blocks_dir / f"bdf-{block_number}.json"
            if filename.exists():
                try:
                    # Verify file is valid JSON and has data
                    with open(filename, 'r') as f:
                        existing = json.load(f)
                        if existing.get("result") and existing["result"].get("number"):
                            # File exists and is valid, skip
                            tx_count = len(existing["result"].get("transactions", []))
                            return {
                                "block": block_number,
                                "success": True,
                                "tx_count": tx_count,
                                "file": str(filename),
                                "skipped": True
                            }
                except:
                    # File corrupt, will re-download
                    pass
            
            # Get block with full transaction objects
            block_hex = hex(block_number)
            block_data = self.rpc_call("eth_getBlockByNumber", [block_hex, True])
            
            if not block_data:
                raise Exception(f"Block {block_number} returned null")
            
            # Save block data
            with open(filename, 'w') as f:
                json.dump({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": block_data
                }, f, indent=2)
            
            # Count transactions
            tx_count = len(block_data.get("transactions", []))
            
            return {
                "block": block_number,
                "success": True,
                "tx_count": tx_count,
                "file": str(filename)
            }
            
        except Exception as e:
            return {
                "block": block_number,
                "success": False,
                "error": str(e)
            }
    
    def test_connection(self):
        """Test RPC connection and get latest block"""
        print("Testing connection to archive node...")
        try:
            latest = self.rpc_call("eth_blockNumber", [])
            latest_dec = int(latest, 16)
            print(f"✓ Connected! Latest block: {latest_dec:,}")
            
            # Verify we can access the start block
            test_block = self.rpc_call("eth_getBlockByNumber", [hex(self.start_block), False])
            if test_block:
                print(f"✓ Can access block {self.start_block:,}")
                return True
            else:
                print(f"✗ Cannot access block {self.start_block:,}")
                return False
                
        except Exception as e:
            print(f"✗ Connection failed: {e}")
            return False
    
    def download_all(self):
        """Download all blocks in parallel"""
        print("\n" + "=" * 70)
        print(f"DOWNLOADING {self.num_blocks:,} ETHEREUM BLOCKS")
        print("=" * 70)
        print(f"Start block: {self.start_block:,}")
        print(f"End block:   {self.start_block + self.num_blocks - 1:,}")
        print(f"Workers:     {self.workers}")
        print(f"Output dir:  {self.output_dir}")
        print()
        
        if not self.test_connection():
            print("\n✗ Cannot connect to archive node. Exiting.")
            return False
        
        print("\nStarting parallel download...")
        print("(This will take ~2-4 hours depending on node speed)\n")
        
        start_time = time.time()
        
        # Create list of blocks to download
        blocks_to_download = list(range(self.start_block, self.start_block + self.num_blocks))
        
        # Download in parallel with progress bar
        with ThreadPoolExecutor(max_workers=self.workers) as executor:
            # Submit all tasks
            futures = {
                executor.submit(self.download_block, block_num): block_num 
                for block_num in blocks_to_download
            }
            
            # Process results with progress bar
            with tqdm(total=self.num_blocks, desc="Downloading blocks", unit="block") as pbar:
                for future in as_completed(futures):
                    result = future.result()
                    
                    if result["success"]:
                        self.downloaded += 1
                        self.total_txs += result["tx_count"]
                    else:
                        self.failed.append(result)
                    
                    pbar.update(1)
                    
                    # Update description with stats
                    if self.downloaded > 0:
                        avg_txs = self.total_txs / self.downloaded
                        pbar.set_postfix({
                            "success": self.downloaded,
                            "failed": len(self.failed),
                            "avg_txs": f"{avg_txs:.1f}"
                        })
        
        elapsed = time.time() - start_time
        
        # Print summary
        print("\n" + "=" * 70)
        print("DOWNLOAD COMPLETE")
        print("=" * 70)
        print(f"Total blocks:       {self.num_blocks:,}")
        print(f"Successfully saved: {self.downloaded:,}")
        print(f"Failed:             {len(self.failed):,}")
        print(f"Total transactions: {self.total_txs:,}")
        print(f"Avg tx/block:       {self.total_txs / max(self.downloaded, 1):.1f}")
        print(f"Time elapsed:       {elapsed / 60:.1f} minutes")
        print(f"Speed:              {self.downloaded / elapsed:.2f} blocks/sec")
        print(f"Output directory:   {self.output_dir}")
        
        if self.failed:
            print(f"\n⚠ {len(self.failed)} blocks failed to download")
            print("Failed blocks:", [f["block"] for f in self.failed[:10]])
            if len(self.failed) > 10:
                print(f"... and {len(self.failed) - 10} more")
            
            # Retry failed blocks
            print("\nRetrying failed blocks...")
            self.retry_failed()
        
        print("\n✓ Dataset ready for benchmark!")
        return True
    
    def retry_failed(self):
        """Retry downloading failed blocks"""
        if not self.failed:
            return
        
        failed_blocks = [f["block"] for f in self.failed]
        self.failed = []
        
        with tqdm(total=len(failed_blocks), desc="Retrying", unit="block") as pbar:
            for block_num in failed_blocks:
                result = self.download_block(block_num)
                if result["success"]:
                    self.downloaded += 1
                    self.total_txs += result["tx_count"]
                else:
                    self.failed.append(result)
                pbar.update(1)
        
        print(f"Retry complete: {len(failed_blocks) - len(self.failed)} recovered")

def main():
    parser = argparse.ArgumentParser(description="Download Ethereum blocks from archive node")
    parser.add_argument("--rpc", required=True, help="RPC endpoint URL (with API key)")
    parser.add_argument("--start", type=int, default=DEFAULT_START_BLOCK, help="Start block number")
    parser.add_argument("--count", type=int, default=DEFAULT_NUM_BLOCKS, help="Number of blocks")
    parser.add_argument("--output", default=DEFAULT_OUTPUT_DIR, help="Output directory")
    parser.add_argument("--workers", type=int, default=DEFAULT_WORKERS, help="Parallel workers")
    
    args = parser.parse_args()
    
    downloader = BlockDownloader(
        rpc_url=args.rpc,
        output_dir=args.output,
        start_block=args.start,
        num_blocks=args.count,
        workers=args.workers
    )
    
    success = downloader.download_all()
    return 0 if success else 1

if __name__ == "__main__":
    exit(main())
