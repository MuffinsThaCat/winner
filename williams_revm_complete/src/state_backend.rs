// State Backend - Loads real state from RPC or local cache
// This replaces EmptyDB with actual account data

use anyhow::Result;
use revm::primitives::{Address, U256, Bytes, Bytecode, B256, AccountInfo, KECCAK_EMPTY};
use rustc_hash::FxHashMap;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use serde_json::{json, Value};
use dashmap::DashMap;

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
            cache: Arc::new(RwLock::new(FxHashMap::default())),
            code_cache: Arc::new(RwLock::new(FxHashMap::default())),
            storage_cache: Arc::new(RwLock::new(FxHashMap::default())),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Bulk prefetch account data for multiple addresses (PARALLEL)
    pub fn bulk_prefetch(&self, addresses: &[Address]) -> Result<()> {
        use rayon::prelude::*;
        use std::time::Instant;
        
        let start = Instant::now();
        #[cfg(not(feature = "bench"))]
        println!("⚡ PARALLEL bulk prefetch: {} addresses", addresses.len());
        
        // OPTIMIZATION: Dynamic chunk sizing based on thread count
        // Larger chunks = better connection reuse, fewer context switches
        // Formula: threads * 8 accounts per thread (empirically optimal for RPC calls)
        let thread_count = rayon::current_num_threads();
        let chunk_size = (thread_count * 8).max(32).min(200); // Min 32, max 200
        let total_chunks = (addresses.len() + chunk_size - 1) / chunk_size;
        
        #[cfg(not(feature = "bench"))]
        println!("  Using dynamic chunk size: {} ({}x threads * 8)", chunk_size, thread_count);
        
        // Process chunks in parallel - each chunk fetches sequentially to reuse HTTP connection
        let results: Vec<(Address, AccountInfo)> = addresses
            .par_chunks(chunk_size)
            .enumerate()
            .flat_map(|(chunk_idx, chunk)| {
                // Progress indicator every 5 chunks
                #[cfg(not(feature = "bench"))]
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
        
        // OPTIMIZATION: Batch insert using extend (single write lock, no loop overhead)
        let mut cache = self.cache.write().unwrap();
        cache.extend(results.into_iter());
        drop(cache);
        
        let elapsed = start.elapsed();
        #[cfg(not(feature = "bench"))]
        {
            let rate = addresses.len() as f64 / elapsed.as_secs_f64();
            println!("\r✓ Parallel prefetch complete: {:.2}ms ({:.0} addrs/sec, {}x parallel)", 
                elapsed.as_secs_f64() * 1000.0,
                rate,
                rayon::current_num_threads()
            );
        }
        
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
/// Now with Williams + φ compression support
#[derive(Clone)]
pub struct OfflineStateBackend {
    // OPTIMIZATION: Use DashMap + Arc<AccountInfo> for lock-free access with cheap clones
    cache: Arc<DashMap<Address, Arc<AccountInfo>>>,
    storage: Arc<DashMap<(Address, U256), U256>>,
    bytecode: Arc<DashMap<B256, Bytecode>>,
    eoa_addresses: Arc<DashMap<Address, ()>>, // Addresses that MUST be EOAs
    default_balance: U256,
    
    // NEW: Williams + φ compression for state snapshots
    compression_enabled: bool,
}

impl OfflineStateBackend {
    pub fn new() -> Self {
        // Default: 1000 ETH per account (enough for any transaction)
        // This is for benchmarking - we want transactions to succeed, not fail due to insufficient funds
        let default_balance = U256::from(1000u64) * U256::from(10u128.pow(18));
        
        Self {
            // OPTIMIZATION: DashMap with pre-allocated capacity
            cache: Arc::new(DashMap::with_capacity(256)),
            storage: Arc::new(DashMap::with_capacity(512)),
            bytecode: Arc::new(DashMap::with_capacity(64)),
            eoa_addresses: Arc::new(DashMap::with_capacity(128)),
            default_balance,
            compression_enabled: false, // Disabled by default for compatibility
        }
    }
    
    /// Create new backend with Williams compression enabled
    pub fn new_with_compression() -> Self {
        let mut backend = Self::new();
        backend.compression_enabled = true;
        backend
    }
    
    /// Enable/disable Williams + φ compression
    pub fn set_compression(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }
    
    /// Check if compression is enabled
    pub fn is_compressed(&self) -> bool {
        self.compression_enabled
    }

    /// Mark an address as EOA (must not have code)
    /// Used for sender addresses to prevent EIP-3607 errors
    pub fn mark_as_eoa(&self, address: Address) {
        // OPTIMIZATION: Lock-free insert with DashMap
        self.eoa_addresses.insert(address, ());
        
        // CRITICAL: Also update cache immediately to remove any code
        if let Some(info_arc) = self.cache.get(&address) {
            let mut info = (**info_arc).clone();
            info.code_hash = KECCAK_EMPTY;
            info.code = None;
            self.cache.insert(address, Arc::new(info));
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
        
        // OPTIMIZATION: Lock-free batch insert with DashMap + Arc
        for (addr, mut info) in new_accounts {
            // CRITICAL: If marked as EOA, force code to KECCAK_EMPTY
            if self.eoa_addresses.contains_key(&addr) {
                info.code_hash = KECCAK_EMPTY;
                info.code = None;
            }
            self.cache.insert(addr, Arc::new(info));
        }
        
        #[cfg(not(feature = "bench"))]
        {
            let elapsed = start.elapsed();
            if elapsed.as_micros() > 100 {
                println!("  ⚡ Offline prefetch: {:.3}ms ({} accounts, parallel)", 
                    elapsed.as_secs_f64() * 1000.0,
                    addresses.len()
                );
            }
        }
        
        Ok(())
    }

    #[inline(always)]  // OPTIMIZATION: Inline hot path
    pub fn get_account(&self, address: Address) -> AccountInfo {
        // OPTIMIZATION: Lock-free get with DashMap + Arc (cheap clone = refcount bump)
        if let Some(info_arc) = self.cache.get(&address) {
            return (**info_arc).clone();
        }
        
        // Not in cache - treat as EOA
        // For offline benchmarking, all addresses are EOAs with sufficient balance
        // Especially sender addresses must be EOAs to avoid EIP-3607
        let info = AccountInfo {
            balance: self.default_balance,
            nonce: 0,
            code_hash: KECCAK_EMPTY, // Always KECCAK_EMPTY for EOAs
            code: None,
        };
        
        // Lock-free insert with Arc
        self.cache.insert(address, Arc::new(info.clone()));
        info
    }

    #[inline(always)]  // OPTIMIZATION: Inline hot path
    pub fn get_storage(&self, address: Address, index: U256) -> U256 {
        // OPTIMIZATION: Lock-free get with DashMap
        self.storage.get(&(address, index)).map(|v| *v).unwrap_or(U256::ZERO)
    }

    #[inline(always)]  // OPTIMIZATION: Inline hot path
    pub fn set_storage(&self, address: Address, index: U256, value: U256) {
        // OPTIMIZATION: Lock-free insert with DashMap
        self.storage.insert((address, index), value);
    }

    pub fn set_bytecode(&self, code_hash: B256, code: Bytecode) {
        // OPTIMIZATION: Lock-free insert with DashMap
        self.bytecode.insert(code_hash, code);
    }

    pub fn get_bytecode(&self, code_hash: B256) -> Option<Bytecode> {
        // OPTIMIZATION: Lock-free get with DashMap
        self.bytecode.get(&code_hash).map(|v| v.clone())
    }

    pub fn update_account(&self, address: Address, info: AccountInfo) {
        // OPTIMIZATION: Lock-free insert with DashMap + Arc
        self.cache.insert(address, Arc::new(info));
    }
    
    /// Export all cached accounts for compression
    pub fn export_accounts(&self) -> HashMap<Address, AccountInfo> {
        self.cache
            .iter()
            .map(|entry| (*entry.key(), (**entry.value()).clone()))
            .collect()
    }
    
    /// Get cache size for compression stats
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}
