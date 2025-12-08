// State Backend - Loads real state from RPC or local cache
// This replaces EmptyDB with actual account data

use anyhow::{Result, Context};
use revm::primitives::{Address, U256, Bytes, Bytecode, B256, AccountInfo, KECCAK_EMPTY};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::{Arc, RwLock};
use serde_json::{json, Value};

/// Real state backend that fetches from RPC
#[derive(Clone)]
pub struct RpcStateBackend {
    rpc_url: String,
    block_number: u64,
    cache: Arc<RwLock<FxHashMap<Address, AccountInfo>>>,
    code_cache: Arc<RwLock<FxHashMap<Address, Bytecode>>>,
    storage_cache: Arc<RwLock<FxHashMap<(Address, U256), U256>>>,
    client: reqwest::blocking::Client,
}

impl RpcStateBackend {
    pub fn new(rpc_url: String, block_number: u64) -> Self {
        Self {
            rpc_url,
            block_number,
            // Pre-allocate with estimated capacity (typical block: ~200 unique addresses)
            cache: Arc::new(RwLock::new(FxHashMap::with_capacity_and_hasher(256, Default::default()))),
            code_cache: Arc::new(RwLock::new(FxHashMap::with_capacity_and_hasher(64, Default::default()))),
            storage_cache: Arc::new(RwLock::new(FxHashMap::with_capacity_and_hasher(512, Default::default()))),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Bulk prefetch account data for multiple addresses (PARALLEL)
    pub fn bulk_prefetch(&self, addresses: &[Address]) -> Result<()> {
        use rayon::prelude::*;
        use std::time::Instant;
        
        let start = Instant::now();
        println!("⚡ PARALLEL bulk prefetch: {} addresses", addresses.len());
        
        // OPTIMIZATION: Dynamic chunk sizing based on thread count
        // Larger chunks = better connection reuse, fewer context switches
        // Formula: threads * 8 accounts per thread (empirically optimal for RPC calls)
        let thread_count = rayon::current_num_threads();
        let chunk_size = (thread_count * 8).max(32).min(200); // Min 32, max 200
        let total_chunks = (addresses.len() + chunk_size - 1) / chunk_size;
        
        println!("  Using dynamic chunk size: {} ({}x threads * 8)", chunk_size, thread_count);
        
        // Process chunks in parallel - each chunk fetches sequentially to reuse HTTP connection
        let results: Vec<(Address, AccountInfo)> = addresses
            .par_chunks(chunk_size)
            .enumerate()
            .flat_map(|(chunk_idx, chunk)| {
                // Progress indicator every 5 chunks
                if chunk_idx % 5 == 0 {
                    let progress = (chunk_idx * 100) / total_chunks.max(1);
                    print!("\rPrefetching: {}% ({}/{})", progress, chunk_idx, total_chunks);
                }
                
                // Fetch all addresses in this chunk (sequential within chunk for connection reuse)
                chunk.iter()
                    .filter_map(|addr| {
                        self.fetch_account_info(*addr)
                            .ok()
                            .map(|info| (*addr, info))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        
        // Batch insert into cache (single write lock)
        let mut cache = self.cache.write().unwrap();
        cache.reserve(results.len());
        for (addr, info) in results.iter() {
            cache.insert(*addr, info.clone());
        }
        drop(cache);
        
        let elapsed = start.elapsed();
        let rate = addresses.len() as f64 / elapsed.as_secs_f64();
        println!("\r✓ Parallel prefetch complete: {:.2}ms ({:.0} addrs/sec, {}x parallel)", 
            elapsed.as_secs_f64() * 1000.0,
            rate,
            rayon::current_num_threads()
        );
        
        Ok(())
    }

    /// Fetch account info from RPC
    fn fetch_account_info(&self, address: Address) -> Result<AccountInfo> {
        let block_tag = format!("0x{:x}", self.block_number);
        
        // Get balance
        let balance_req = json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [format!("0x{:x}", address), &block_tag],
            "id": 1
        });
        
        let balance_resp: Value = self.client
            .post(&self.rpc_url)
            .json(&balance_req)
            .send()?
            .json()?;
        
        let balance = if let Some(bal_str) = balance_resp.get("result").and_then(|v| v.as_str()) {
            U256::from_str_radix(bal_str.trim_start_matches("0x"), 16).unwrap_or(U256::ZERO)
        } else {
            U256::ZERO
        };
        
        // Get nonce
        let nonce_req = json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionCount",
            "params": [format!("0x{:x}", address), &block_tag],
            "id": 2
        });
        
        let nonce_resp: Value = self.client
            .post(&self.rpc_url)
            .json(&nonce_req)
            .send()?
            .json()?;
        
        let nonce = if let Some(nonce_str) = nonce_resp.get("result").and_then(|v| v.as_str()) {
            u64::from_str_radix(nonce_str.trim_start_matches("0x"), 16).unwrap_or(0)
        } else {
            0
        };
        
        // Get code
        let code_req = json!({
            "jsonrpc": "2.0",
            "method": "eth_getCode",
            "params": [format!("0x{:x}", address), &block_tag],
            "id": 3
        });
        
        let code_resp: Value = self.client
            .post(&self.rpc_url)
            .json(&code_req)
            .send()?
            .json()?;
        
        let code_hash = if let Some(code_str) = code_resp.get("result").and_then(|v| v.as_str()) {
            if code_str == "0x" || code_str.len() <= 2 {
                B256::ZERO
            } else {
                // Has code - compute hash
                let code_bytes = hex::decode(code_str.trim_start_matches("0x")).unwrap_or_default();
                let bytecode = Bytecode::new_raw(Bytes::from(code_bytes.clone()));
                self.code_cache.write().unwrap().insert(address, bytecode);
                revm::primitives::keccak256(&code_bytes)
            }
        } else {
            B256::ZERO
        };
        
        Ok(AccountInfo {
            balance,
            nonce,
            code_hash,
            code: None,
        })
    }

    /// Get account info (from cache or fetch)
    pub fn get_account(&self, address: Address) -> Result<AccountInfo> {
        // Check cache first
        if let Some(info) = self.cache.read().unwrap().get(&address) {
            return Ok(info.clone());
        }
        
        // Fetch if not in cache
        let info = self.fetch_account_info(address)?;
        self.cache.write().unwrap().insert(address, info.clone());
        Ok(info)
    }

    /// Get code for an address
    pub fn get_code(&self, address: Address) -> Option<Bytecode> {
        self.code_cache.read().unwrap().get(&address).cloned()
    }

    /// Get storage value
    pub fn get_storage(&self, address: Address, index: U256) -> Result<U256> {
        let key = (address, index);
        
        // Check cache
        if let Some(value) = self.storage_cache.read().unwrap().get(&key) {
            return Ok(*value);
        }
        
        // Fetch from RPC
        let block_tag = format!("0x{:x}", self.block_number);
        let req = json!({
            "jsonrpc": "2.0",
            "method": "eth_getStorageAt",
            "params": [
                format!("0x{:x}", address),
                format!("0x{:x}", index),
                &block_tag
            ],
            "id": 4
        });
        
        let resp: Value = self.client
            .post(&self.rpc_url)
            .json(&req)
            .send()?
            .json()?;
        
        let value = if let Some(val_str) = resp.get("result").and_then(|v| v.as_str()) {
            U256::from_str_radix(val_str.trim_start_matches("0x"), 16).unwrap_or(U256::ZERO)
        } else {
            U256::ZERO
        };
        
        self.storage_cache.write().unwrap().insert(key, value);
        Ok(value)
    }

    /// Get cached state size (for reporting)
    pub fn cache_size(&self) -> (usize, usize, usize) {
        (
            self.cache.read().unwrap().len(),
            self.code_cache.read().unwrap().len(),
            self.storage_cache.read().unwrap().len(),
        )
    }
}

/// Offline state backend for testing without RPC
/// Uses reasonable defaults for accounts
#[derive(Clone)]
pub struct OfflineStateBackend {
    cache: Arc<RwLock<FxHashMap<Address, AccountInfo>>>,
    storage: Arc<RwLock<FxHashMap<(Address, U256), U256>>>,
    bytecode: Arc<RwLock<FxHashMap<B256, Bytecode>>>,
    eoa_addresses: Arc<RwLock<FxHashSet<Address>>>, // Addresses that MUST be EOAs
    default_balance: U256,
}

impl OfflineStateBackend {
    pub fn new() -> Self {
        // Default: 1000 ETH per account (enough for any transaction)
        // This is for benchmarking - we want transactions to succeed, not fail due to insufficient funds
        let default_balance = U256::from(1000u64) * U256::from(10u128.pow(18));
        
        Self {
            // Pre-allocate with estimated capacity for typical block
            cache: Arc::new(RwLock::new(FxHashMap::with_capacity_and_hasher(256, Default::default()))),
            storage: Arc::new(RwLock::new(FxHashMap::with_capacity_and_hasher(512, Default::default()))),
            bytecode: Arc::new(RwLock::new(FxHashMap::with_capacity_and_hasher(64, Default::default()))),
            eoa_addresses: Arc::new(RwLock::new(FxHashSet::with_capacity_and_hasher(128, Default::default()))),
            default_balance,
        }
    }

    /// Mark an address as EOA (must not have code)
    /// Used for sender addresses to prevent EIP-3607 errors
    pub fn mark_as_eoa(&self, address: Address) {
        self.eoa_addresses.write().unwrap().insert(address);
        
        // CRITICAL: Also update cache immediately to remove any code
        let mut cache = self.cache.write().unwrap();
        if let Some(info) = cache.get_mut(&address) {
            info.code_hash = KECCAK_EMPTY;
            info.code = None;
        }
    }

    pub fn bulk_prefetch(&self, addresses: &[Address]) -> Result<()> {
        use rayon::prelude::*;
        use std::time::Instant;
        
        let start = Instant::now();
        
        // OPTIMIZATION: Build all accounts in parallel (pure computation, no I/O)
        // All addresses are EOAs by default for benchmarking (prevents EIP-3607 errors)
        let new_accounts: Vec<(Address, AccountInfo)> = addresses
            .par_iter()
            .map(|addr| {
                let info = AccountInfo {
                    balance: self.default_balance,
                    nonce: 0,
                    code_hash: KECCAK_EMPTY,
                    code: None,
                };
                (*addr, info)
            })
            .collect();
        
        // Single write lock for batch insert (minimizes lock contention)
        let mut cache = self.cache.write().unwrap();
        cache.reserve(new_accounts.len());
        
        // Read EOA set once inside write lock (already have exclusive access)
        let eoa_addresses = self.eoa_addresses.read().unwrap();
        
        for (addr, info) in new_accounts {
            cache.entry(addr).or_insert(info);
            
            // CRITICAL: If marked as EOA, force code to KECCAK_EMPTY
            if eoa_addresses.contains(&addr) {
                if let Some(cached_info) = cache.get_mut(&addr) {
                    cached_info.code_hash = KECCAK_EMPTY;
                    cached_info.code = None;
                }
            }
        }
        drop(eoa_addresses);
        drop(cache);
        
        let elapsed = start.elapsed();
        if elapsed.as_micros() > 100 {
            println!("  ⚡ Offline prefetch: {:.3}ms ({} accounts, parallel)", 
                elapsed.as_secs_f64() * 1000.0,
                addresses.len()
            );
        }
        
        Ok(())
    }

    pub fn get_account(&self, address: Address) -> AccountInfo {
        let cache = self.cache.read().unwrap();
        if let Some(info) = cache.get(&address) {
            return info.clone();
        }
        drop(cache);
        
        // Check if this is a marked EOA address
        let is_eoa = self.eoa_addresses.read().unwrap().contains(&address);
        
        // Not in cache - treat as EOA
        // For offline benchmarking, all addresses are EOAs with sufficient balance
        // Especially sender addresses must be EOAs to avoid EIP-3607
        let info = AccountInfo {
            balance: self.default_balance,
            nonce: 0,
            code_hash: KECCAK_EMPTY, // Always KECCAK_EMPTY for EOAs
            code: None,
        };
        
        self.cache.write().unwrap().insert(address, info.clone());
        info
    }

    pub fn get_storage(&self, address: Address, index: U256) -> U256 {
        let storage = self.storage.read().unwrap();
        storage.get(&(address, index)).copied().unwrap_or(U256::ZERO)
    }

    pub fn set_storage(&self, address: Address, index: U256, value: U256) {
        self.storage.write().unwrap().insert((address, index), value);
    }

    pub fn set_bytecode(&self, code_hash: B256, code: Bytecode) {
        self.bytecode.write().unwrap().insert(code_hash, code);
    }

    pub fn get_bytecode(&self, code_hash: B256) -> Option<Bytecode> {
        self.bytecode.read().unwrap().get(&code_hash).cloned()
    }

    pub fn update_account(&self, address: Address, info: AccountInfo) {
        self.cache.write().unwrap().insert(address, info);
    }
}
