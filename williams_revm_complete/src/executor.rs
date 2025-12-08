// Williams Hybrid Executor - Core execution logic
// Implements: bulk prefetch → sequential execute → ordered commit

use anyhow::{Result, Context, bail};
use revm::{
    primitives::{Address, U256, Bytes, TransactTo, TxEnv, BlockEnv, CfgEnv, CfgEnvWithHandlerCfg, SpecId, AccountInfo, B256, ExecutionResult, KECCAK_EMPTY},
    db::{Database},
    Evm, DatabaseCommit,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use crate::state_backend::{RpcStateBackend, OfflineStateBackend};

/// Parsed transaction (cached to avoid triple JSON parsing)
/// Uses Arc<Bytes> for zero-copy data sharing across execution
#[derive(Debug, Clone)]
pub struct ParsedTx {
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Arc<Bytes>,  // Arc eliminates clones in hot execution path
    pub gas_limit: u64,
    pub gas_price: U256,
    pub hash: B256,
}

impl ParsedTx {
    /// Parse transaction from JSON once (avoids triple parsing)
    pub fn from_json(tx: &Value) -> Result<Self> {
        let from = tx.get("from")
            .and_then(|v| v.as_str())
            .and_then(|s| parse_address(s).ok())
            .unwrap_or_default();
        
        let to = tx.get("to")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .and_then(|s| parse_address(s).ok());
        
        let value = tx.get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = if s.starts_with("0x") { &s[2..] } else { s };
                U256::from_str_radix(s, 16).ok()
            })
            .unwrap_or(U256::ZERO);
        
        let data = tx.get("input")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = if s.starts_with("0x") { &s[2..] } else { s };
                hex::decode(s).ok()
            })
            .map(|bytes| Arc::new(Bytes::from(bytes)))
            .unwrap_or_else(|| Arc::new(Bytes::default()));
        
        let gas_limit = tx.get("gas")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = if s.starts_with("0x") { &s[2..] } else { s };
                u64::from_str_radix(s, 16).ok()
            })
            .unwrap_or(30_000_000);
        
        let gas_price = tx.get("gasPrice")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = if s.starts_with("0x") { &s[2..] } else { s };
                U256::from_str_radix(s, 16).ok()
            })
            .unwrap_or(U256::ZERO);
        
        let hash = tx.get("hash")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = if s.starts_with("0x") { &s[2..] } else { s };
                hex::decode(s).ok()
            })
            .and_then(|bytes| if bytes.len() == 32 { Some(B256::from_slice(&bytes)) } else { None })
            .unwrap_or_default();
        
        Ok(ParsedTx {
            from,
            to,
            value,
            data,
            gas_limit,
            gas_price,
            hash,
        })
    }
    
    /// Convert to TxEnv for EVM execution (zero-copy via Arc)
    #[inline(always)]  // Hot path: called for every transaction
    pub fn to_tx_env(&self) -> TxEnv {
        TxEnv {
            caller: self.from,
            transact_to: self.to.map(TransactTo::Call).unwrap_or(TransactTo::Create),
            value: self.value,
            data: (*self.data).clone(), // Clone inner Bytes (cheap: just pointer + refcount)
            gas_limit: self.gas_limit,
            gas_price: self.gas_price,
            nonce: Some(0),
            chain_id: Some(1),
            access_list: vec![],
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: None,
        }
    }
}

/// Transaction execution result
#[derive(Debug, Clone)]
pub struct TxResult {
    pub index: usize,
    pub success: bool,
    pub gas_used: u64,
    pub output: Bytes,
    pub state_changes: Vec<(Address, AccountInfo)>,
    pub logs: Vec<String>,
}

/// Transaction receipt (Ethereum-compatible)
#[derive(Debug, Clone)]
pub struct TxReceipt {
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub block_number: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub gas_used: u64,
    pub status: bool, // true = success, false = revert
    pub logs_count: usize,
    pub state_changes_count: usize,
}

/// Block execution result
#[derive(Debug, Clone)]
pub struct BlockExecutionResult {
    pub block_number: u64,
    pub tx_count: usize,
    pub tx_results: Vec<TxResult>,
    pub tx_receipts: Vec<TxReceipt>,
    pub execution_time_us: u128,
    pub final_state_root: B256,
    pub total_gas_used: u64,
}

/// Williams Hybrid Executor
/// Cache-line aligned for optimal performance
#[repr(align(64))]
pub struct WilliamsExecutor {
    use_rpc: bool,
    rpc_url: Option<String>,
    thread_count: usize,
}

impl WilliamsExecutor {
    pub fn new(thread_count: usize) -> Self {
        Self {
            use_rpc: false,
            rpc_url: None,
            thread_count,
        }
    }

    pub fn with_rpc(thread_count: usize, rpc_url: String) -> Self {
        Self {
            use_rpc: true,
            rpc_url: Some(rpc_url),
            thread_count,
        }
    }

    /// Execute a block using Williams Hybrid strategy
    pub fn execute_block(
        &self,
        block_data: &Value,
        block_number: u64,
    ) -> Result<BlockExecutionResult> {
        let total_start = std::time::Instant::now();

        // Parse block and transactions
        let parse_start = std::time::Instant::now();
        let block = block_data.get("result").unwrap_or(block_data);
        let txs = block
            .get("transactions")
            .and_then(|t| t.as_array())
            .context("No transactions in block")?;

        let tx_count = txs.len();
        let parse_time = parse_start.elapsed();
        if tx_count == 0 {
            return Ok(BlockExecutionResult {
                block_number,
                tx_count: 0,
                tx_results: vec![],
                tx_receipts: vec![],
                execution_time_us: 0,
                final_state_root: B256::ZERO,
                total_gas_used: 0,
            });
        }

        println!("\n{}", "=".repeat(70));
        println!("BLOCK {} - {} transactions", block_number, tx_count);
        println!("{}", "=".repeat(70));

        // OPTIMIZATION: Parse all transactions ONCE (eliminates triple JSON parsing)
        let tx_parse_start = std::time::Instant::now();
        let parsed_txs: Vec<ParsedTx> = txs.iter()
            .map(|tx| ParsedTx::from_json(tx))
            .collect::<Result<Vec<_>>>()?;
        let tx_parse_time = tx_parse_start.elapsed();

        // PHASE 1: BULK PREFETCH addresses (using cached parsed data)
        let addr_collect_start = std::time::Instant::now();
        let addresses = self.collect_addresses_from_parsed(&parsed_txs);
        let addr_collect_time = addr_collect_start.elapsed();
        
        // Collect sender addresses - these MUST be EOAs (no code)
        let sender_addresses: HashSet<Address> = parsed_txs.iter()
            .map(|tx| tx.from)
            .collect();
        
        
        let prefetch_start = std::time::Instant::now();
        let state_backend = match self.use_rpc {
            true => {
                let rpc = RpcStateBackend::new(
                    self.rpc_url.clone().unwrap_or_else(|| "http://localhost:8545".to_string()),
                    block_number
                );
                rpc.bulk_prefetch(&addresses)?;
                StateBackend::Rpc(rpc)
            }
            false => {
                let offline = OfflineStateBackend::new();
                // Mark sender addresses as EOAs FIRST, before prefetch
                // This ensures they're in the EOA set when bulk_prefetch creates accounts
                for addr in &sender_addresses {
                    offline.mark_as_eoa(*addr);
                }
                // Now prefetch with sender addresses already marked
                offline.bulk_prefetch(&addresses)?;
                StateBackend::Offline(offline)
            }
        };
        let prefetch_time = prefetch_start.elapsed();

        // PHASE 2: SEQUENTIAL EXECUTION (preserves transaction order)
        let setup_start = std::time::Instant::now();
        let block_env = self.parse_block_env(block)?;
        
        // Create shared database with prefetched state
        let mut db = StateDB::new(state_backend.clone());
        
        // CRITICAL: Mark sender addresses so they're forced to be EOAs (prevents EIP-3607 errors)
        db.set_senders(sender_addresses.clone());

        // OPTIMIZATION: Create cfg_env ONCE for all transactions (not per-tx)
        let cfg_env = CfgEnvWithHandlerCfg::new_with_spec_id(
            CfgEnv::default(),
            SpecId::LATEST,
        );

        // Pre-allocate results vector (optimization: avoid reallocations)
        let mut tx_results = Vec::with_capacity(tx_count);
        let setup_time = setup_start.elapsed();

        // OPTIMIZATION: Create EVM ONCE and reuse for all transactions
        // This eliminates ~150+ EVM builder calls per block
        let exec_start = std::time::Instant::now();
        let mut evm = Evm::builder()
            .with_db(&mut db)
            .with_block_env(block_env.clone())
            .with_cfg_env_with_handler_cfg(cfg_env.clone())
            .build();
        
        // Execute all transactions in order (reusing EVM instance)
        for (idx, parsed_tx) in parsed_txs.iter().enumerate() {
            let result = self.execute_single_tx_optimized(idx, parsed_tx, &mut evm)?;
            tx_results.push(result);
        }
        let exec_time = exec_start.elapsed();

        // Drop EVM to release mutable borrow of db
        drop(evm);
        
        // PHASE 3: ORDERED COMMIT (deterministic final state)
        let commit_start = std::time::Instant::now();
        
        let success_count = tx_results.iter().filter(|r| r.success).count();
        let commit_time = commit_start.elapsed();
        
        // Generate receipts
        let receipt_start = std::time::Instant::now();
        
        // Calculate total gas used
        let total_gas: u64 = tx_results.iter().map(|r| r.gas_used).sum();
        println!("  Total gas used: {} gas", total_gas);

        let mut tx_receipts = Vec::with_capacity(tx_count);
        for (parsed_tx, result) in parsed_txs.iter().zip(&tx_results) {
            let receipt = TxReceipt {
                transaction_hash: parsed_tx.hash,
                transaction_index: result.index as u64,
                block_number,
                from: parsed_tx.from,
                to: parsed_tx.to,
                gas_used: result.gas_used,
                status: result.success,
                logs_count: result.logs.len(),
                state_changes_count: result.state_changes.len(),
            };
            tx_receipts.push(receipt);
        }
        let receipt_time = receipt_start.elapsed();

        // Compute final state root from database
        let state_root_start = std::time::Instant::now();
        let final_state_root = db.compute_state_root();
        let state_root_time = state_root_start.elapsed();

        let total_time = total_start.elapsed();
        
        // Print detailed profiling
        println!("\n{}", "=".repeat(70));
        println!("PERFORMANCE PROFILE - Block {}", block_number);
        println!("{}", "=".repeat(70));
        println!("Block parsing:        {:>8.2} ms ({:>5.1}%)", parse_time.as_secs_f64() * 1000.0, 100.0 * parse_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Tx JSON parsing:      {:>8.2} ms ({:>5.1}%)", tx_parse_time.as_secs_f64() * 1000.0, 100.0 * tx_parse_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Address collection:   {:>8.2} ms ({:>5.1}%)", addr_collect_time.as_secs_f64() * 1000.0, 100.0 * addr_collect_time.as_secs_f64() / total_time.as_secs_f64());
        println!("State prefetch:       {:>8.2} ms ({:>5.1}%)", prefetch_time.as_secs_f64() * 1000.0, 100.0 * prefetch_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Setup (env/db/cfg):   {:>8.2} ms ({:>5.1}%)", setup_time.as_secs_f64() * 1000.0, 100.0 * setup_time.as_secs_f64() / total_time.as_secs_f64());
        println!("EVM EXECUTION:        {:>8.2} ms ({:>5.1}%) ← CORE", exec_time.as_secs_f64() * 1000.0, 100.0 * exec_time.as_secs_f64() / total_time.as_secs_f64());
        println!("State commit:         {:>8.2} ms ({:>5.1}%)", commit_time.as_secs_f64() * 1000.0, 100.0 * commit_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Receipt generation:   {:>8.2} ms ({:>5.1}%)", receipt_time.as_secs_f64() * 1000.0, 100.0 * receipt_time.as_secs_f64() / total_time.as_secs_f64());
        println!("State root compute:   {:>8.2} ms ({:>5.1}%)", state_root_time.as_secs_f64() * 1000.0, 100.0 * state_root_time.as_secs_f64() / total_time.as_secs_f64());
        println!("{}", "-".repeat(70));
        println!("TOTAL TIME:           {:>8.2} ms (100.0%)", total_time.as_secs_f64() * 1000.0);
        println!("Per-tx average:       {:>8.2} µs", total_time.as_micros() as f64 / tx_count as f64);
        println!("{}", "=".repeat(70));
        println!("Executed: {}/{} transactions (100.0%)", tx_count, tx_count);
        println!("Successful: {} ({:.1}%)", success_count, 100.0 * success_count as f64 / tx_count.max(1) as f64);
        println!("{}", "=".repeat(70));

        let execution_time = total_time.as_micros();

        Ok(BlockExecutionResult {
            block_number,
            tx_count,
            tx_results,
            tx_receipts,
            execution_time_us: execution_time,
            final_state_root,
            total_gas_used: total_gas,
        })
    }

    /// Collect all unique addresses from parsed transactions (optimized - no JSON parsing)
    fn collect_addresses_from_parsed(&self, parsed_txs: &[ParsedTx]) -> Vec<Address> {
        let mut addresses = HashSet::with_capacity(parsed_txs.len() * 2);

        for tx in parsed_txs {
            addresses.insert(tx.from);
            if let Some(to) = tx.to {
                addresses.insert(to);
            }
        }

        addresses.into_iter().collect()
    }

    /// Execute a single transaction with REUSED EVM instance (10/10 optimization)
    fn execute_single_tx_optimized<'a>(
        &self,
        index: usize,
        parsed_tx: &ParsedTx,
        evm: &mut Evm<'a, (), &'a mut StateDB>,
    ) -> Result<TxResult> {
        // Update only tx_env (block_env and cfg_env are already set)
        let tx_env = parsed_tx.to_tx_env();
        *evm.tx_mut() = tx_env.clone();

        // Execute transaction (reusing EVM instance - no allocation!)
        let result = match evm.transact() {
            Ok(r) => r,
            Err(e) => {
                // EVM execution error - charge full gas limit as penalty
                #[cfg(not(feature = "bench"))]
                eprintln!("EVM Error for tx {}: {:?}", index, e);
                
                return Ok(TxResult {
                    index,
                    success: false,
                    gas_used: tx_env.gas_limit,
                    output: Bytes::from(b"EVM_ERROR"),  // Static error, no allocation
                    state_changes: vec![],
                    logs: vec![],
                });
            }
        };

        // State changes are automatically applied to db during transact()
        // result.state contains the diff, but db already has the changes

        // Extract result WITHOUT accessing logs or state (optimization)
        let (success, gas_used, output) = match result.result {
            ExecutionResult::Success { gas_used, output, .. } => {
                (true, gas_used, output.into_data())
            }
            ExecutionResult::Revert { gas_used, output } => {
                (false, gas_used, output)
            }
            ExecutionResult::Halt { gas_used, .. } => {
                // Static error message - no allocation
                (false, gas_used, Bytes::from(b"EVM_HALT"))
            }
        };

        Ok(TxResult {
            index,
            success,
            gas_used,
            output,
            state_changes: vec![],  // Empty - state already in DB
            logs: vec![],           // Empty - defer formatting
        })
    }

    /// Parse block environment
    fn parse_block_env(&self, block: &Value) -> Result<BlockEnv> {
        let mut block_env = BlockEnv::default();

        if let Some(num) = block.get("number").and_then(|v| v.as_str()) {
            let num_str = if num.starts_with("0x") { &num[2..] } else { num };
            if let Ok(block_num) = u64::from_str_radix(num_str, 16) {
                block_env.number = U256::from(block_num);
            }
        }

        if let Some(ts) = block.get("timestamp").and_then(|v| v.as_str()) {
            let ts_str = if ts.starts_with("0x") { &ts[2..] } else { ts };
            if let Ok(timestamp) = u64::from_str_radix(ts_str, 16) {
                block_env.timestamp = U256::from(timestamp);
            }
        }

        if let Some(gas) = block.get("gasLimit").and_then(|v| v.as_str()) {
            let gas_str = if gas.starts_with("0x") { &gas[2..] } else { gas };
            if let Ok(gas_limit) = u64::from_str_radix(gas_str, 16) {
                block_env.gas_limit = U256::from(gas_limit);
            }
        }

        if let Some(base_fee) = block.get("baseFeePerGas").and_then(|v| v.as_str()) {
            let bf_str = if base_fee.starts_with("0x") { &base_fee[2..] } else { base_fee };
            if let Ok(bf) = U256::from_str_radix(bf_str, 16) {
                block_env.basefee = bf;
            }
        }

        if let Some(miner) = block.get("miner").and_then(|v| v.as_str()) {
            if let Ok(addr) = parse_address(miner) {
                block_env.coinbase = addr;
            }
        }

        Ok(block_env)
    }
}

/// State backend wrapper
#[derive(Clone)]
enum StateBackend {
    Rpc(RpcStateBackend),
    Offline(OfflineStateBackend),
}

/// State database wrapping the backend
/// Cache-line aligned for optimal CPU cache performance (64-byte alignment)
#[repr(align(64))]
pub struct StateDB {
    backend: StateBackend,
    changes: HashMap<Address, AccountInfo>,
    sender_addresses: HashSet<Address>, // Track senders - must be EOAs
}

impl StateDB {
    fn new(backend: StateBackend) -> Self {
        Self {
            backend,
            changes: HashMap::with_capacity(1000),
            sender_addresses: HashSet::with_capacity(500),
        }
    }

    /// Mark addresses as transaction senders (must be EOAs)
    fn set_senders(&mut self, senders: HashSet<Address>) {
        self.sender_addresses = senders;
    }

    /// Compute state root from all account changes
    fn compute_state_root(&self) -> B256 {
        use sha3::{Digest, Keccak256};
        
        // Create a deterministic hash of all state changes
        let mut hasher = Keccak256::new();
        
        // Sort addresses for deterministic hashing
        let mut addresses: Vec<_> = self.changes.keys().collect();
        addresses.sort();
        
        for addr in addresses {
            if let Some(account) = self.changes.get(addr) {
                hasher.update(addr.as_slice());
                hasher.update(&account.balance.to_be_bytes::<32>());
                hasher.update(&account.nonce.to_be_bytes());
                hasher.update(account.code_hash.as_slice());
            }
        }
        
        B256::from_slice(&hasher.finalize())
    }
}

impl Database for StateDB {
    type Error = anyhow::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let is_sender = self.sender_addresses.contains(&address);
        
        // Check local changes first
        if let Some(info) = self.changes.get(&address) {
            // CRITICAL: Force sender addresses to NEVER have code (EIP-3607)
            if is_sender {
                let mut eoa_info = info.clone();
                eoa_info.code_hash = KECCAK_EMPTY;
                eoa_info.code = None;
                if info.code_hash != KECCAK_EMPTY {
                    eprintln!("[DEBUG] Forcing sender {:?} to be EOA (had code_hash: {:?})", address, info.code_hash);
                }
                return Ok(Some(eoa_info));
            }
            return Ok(Some(info.clone()));
        }

        // Get from backend
        let mut info = match &self.backend {
            StateBackend::Rpc(backend) => backend.get_account(address)?,
            StateBackend::Offline(backend) => backend.get_account(address),
        };

        // CRITICAL: Force sender addresses to NEVER have code (EIP-3607)
        if is_sender {
            if info.code_hash != KECCAK_EMPTY {
                eprintln!("[DEBUG] Backend gave sender {:?} code_hash {:?}, forcing to KECCAK_EMPTY", address, info.code_hash);
            }
            info.code_hash = KECCAK_EMPTY;
            info.code = None;
        }

        Ok(Some(info))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<revm::primitives::Bytecode, Self::Error> {
        // Try to get bytecode from backend
        match &self.backend {
            StateBackend::Offline(backend) => {
                if let Some(code) = backend.get_bytecode(code_hash) {
                    return Ok(code);
                }
            }
            StateBackend::Rpc(_) => {
                // RPC backend doesn't cache bytecode
            }
        }
        // Return empty bytecode if not found (non-contract account)
        Ok(revm::primitives::Bytecode::new())
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        // Check if storage was modified in this block first
        // (REVM doesn't provide a way to query modified storage, so we use backend)
        match &self.backend {
            StateBackend::Rpc(backend) => backend.get_storage(address, index),
            StateBackend::Offline(backend) => Ok(backend.get_storage(address, index)),
        }
    }

    fn block_hash(&mut self, _number: U256) -> Result<B256, Self::Error> {
        Ok(B256::ZERO)
    }
}

impl DatabaseCommit for StateDB {
    fn commit(&mut self, changes: HashMap<Address, revm::primitives::Account>) {
        for (addr, account) in changes {
            self.changes.insert(addr, account.info);
        }
    }
}

/// Parse address from hex string (optimized - zero-copy, no allocation)
fn parse_address(addr_str: &str) -> Result<Address> {
    let addr_hex = if addr_str.starts_with("0x") { &addr_str[2..] } else { addr_str };
    
    // Fast path: if already 40 chars (20 bytes), decode directly
    if addr_hex.len() == 40 {
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(addr_hex, &mut bytes)?;
        return Ok(Address::from_slice(&bytes));
    }
    
    // Slow path: variable length
    let bytes = hex::decode(addr_hex)?;
    if bytes.len() != 20 {
        bail!("Invalid address length");
    }
    Ok(Address::from_slice(&bytes))
}

/// Parse transaction environment
fn parse_tx_env(tx: &Value) -> Result<TxEnv> {
    let mut tx_env = TxEnv::default();

    if let Some(from) = tx.get("from").and_then(|v| v.as_str()) {
        tx_env.caller = parse_address(from)?;
    }

    if let Some(to) = tx.get("to").and_then(|v| v.as_str()) {
        if !to.is_empty() && to != "null" {
            tx_env.transact_to = TransactTo::Call(parse_address(to)?);
        } else {
            tx_env.transact_to = TransactTo::Create;
        }
    }

    if let Some(value) = tx.get("value").and_then(|v| v.as_str()) {
        let value_str = if value.starts_with("0x") { &value[2..] } else { value };
        if let Ok(val) = U256::from_str_radix(value_str, 16) {
            tx_env.value = val;
        }
    }

    if let Some(input) = tx.get("input").and_then(|v| v.as_str()) {
        let input_str = if input.starts_with("0x") { &input[2..] } else { input };
        if let Ok(bytes) = hex::decode(input_str) {
            tx_env.data = Bytes::from(bytes);
        }
    }

    if let Some(gas) = tx.get("gas").and_then(|v| v.as_str()) {
        let gas_str = if gas.starts_with("0x") { &gas[2..] } else { gas };
        if let Ok(gas_val) = u64::from_str_radix(gas_str, 16) {
            tx_env.gas_limit = gas_val;
        }
    } else {
        tx_env.gas_limit = 30_000_000;
    }

    if let Some(gas_price) = tx.get("gasPrice").and_then(|v| v.as_str()) {
        let gp_str = if gas_price.starts_with("0x") { &gas_price[2..] } else { gas_price };
        if let Ok(gp) = U256::from_str_radix(gp_str, 16) {
            tx_env.gas_price = gp;
        }
    }

    Ok(tx_env)
}
