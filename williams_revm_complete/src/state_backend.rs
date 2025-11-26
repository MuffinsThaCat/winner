// State Backend - Loads real state from RPC or local cache
// This replaces EmptyDB with actual account data

use anyhow::{Result, Context};
use revm::primitives::{Address, U256, Bytes, Bytecode, B256, AccountInfo};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde_json::{json, Value};

/// Real state backend that fetches from RPC
#[derive(Clone)]
pub struct RpcStateBackend {
    rpc_url: String,
    block_number: u64,
    cache: Arc<RwLock<HashMap<Address, AccountInfo>>>,
    code_cache: Arc<RwLock<HashMap<Address, Bytecode>>>,
    storage_cache: Arc<RwLock<HashMap<(Address, U256), U256>>>,
    client: reqwest::blocking::Client,
}

impl RpcStateBackend {
    pub fn new(rpc_url: String, block_number: u64) -> Self {
        Self {
            rpc_url,
            block_number,
            cache: Arc::new(RwLock::new(HashMap::new())),
            code_cache: Arc::new(RwLock::new(HashMap::new())),
            storage_cache: Arc::new(RwLock::new(HashMap::new())),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Bulk prefetch account data for multiple addresses
    pub fn bulk_prefetch(&self, addresses: &[Address]) -> Result<()> {
        println!("Bulk prefetching {} unique addresses...", addresses.len());
        
        for (idx, addr) in addresses.iter().enumerate() {
            if idx % 10 == 0 {
                print!("\rPrefetching: {}/{}", idx, addresses.len());
            }
            
            // Fetch account info
            if let Ok(info) = self.fetch_account_info(*addr) {
                self.cache.write().unwrap().insert(*addr, info);
            }
        }
        
        println!("\r✓ Prefetched {}/{} accounts", addresses.len(), addresses.len());
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
    cache: Arc<RwLock<HashMap<Address, AccountInfo>>>,
    default_balance: U256,
}

impl OfflineStateBackend {
    pub fn new() -> Self {
        // Default: 10 ETH per account (realistic for testing, enough for most transactions)
        // Note: This matches typical testnet configurations
        let default_balance = U256::from(10u64) * U256::from(10u128.pow(18));
        
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_balance,
        }
    }

    pub fn bulk_prefetch(&self, addresses: &[Address]) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        for addr in addresses {
            cache.entry(*addr).or_insert(AccountInfo {
                balance: self.default_balance,
                nonce: 0,
                code_hash: B256::ZERO,
                code: None,
            });
        }
        Ok(())
    }

    pub fn get_account(&self, address: Address) -> AccountInfo {
        let cache = self.cache.read().unwrap();
        if let Some(info) = cache.get(&address) {
            return info.clone();
        }
        drop(cache);
        
        // Not in cache - insert with default balance
        let info = AccountInfo {
            balance: self.default_balance,
            nonce: 0,
            code_hash: B256::ZERO,
            code: None,
        };
        
        self.cache.write().unwrap().insert(address, info.clone());
        info
    }

    pub fn get_storage(&self, _address: Address, _index: U256) -> U256 {
        U256::ZERO
    }

    pub fn update_account(&self, address: Address, info: AccountInfo) {
        self.cache.write().unwrap().insert(address, info);
    }
}
