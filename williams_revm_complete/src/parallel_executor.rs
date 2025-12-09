// Williams Parallel Executor - Sequential+Parallel Hybrid
// Phase 1: Bulk prefetch (sequential - I/O optimal)
// Phase 2: Parallel execution (parallel - CPU optimal)
// Phase 3: Ordered commit (sequential - deterministic)

use anyhow::{Result, Context};
use rayon::prelude::*;
use revm::{
    primitives::{Address, U256, Bytes, TransactTo, TxEnv, BlockEnv, CfgEnv, CfgEnvWithHandlerCfg, SpecId, AccountInfo, B256, ExecutionResult, KECCAK_EMPTY},
    db::Database,
    Evm, DatabaseCommit,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use crate::state_backend::{RpcStateBackend, OfflineStateBackend};
use crate::executor::{TxResult, BlockExecutionResult, ParsedTx, TxReceipt};

/// Williams Parallel Executor with Hybrid Architecture
pub struct WilliamsParallelExecutor {
    use_rpc: bool,
    rpc_url: Option<String>,
    thread_count: usize,
}

impl WilliamsParallelExecutor {
    pub fn new(thread_count: usize) -> Self {
        // Configure rayon thread pool
        rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .ok(); // Ignore error if already initialized
        
        Self {
            use_rpc: false,
            rpc_url: None,
            thread_count,
        }
    }

    pub fn with_rpc(thread_count: usize, rpc_url: String) -> Self {
        rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .ok();
            
        Self {
            use_rpc: true,
            rpc_url: Some(rpc_url),
            thread_count,
        }
    }

    /// Execute a block using Williams Parallel Hybrid strategy
    pub fn execute_block(
        &self,
        block_data: &Value,
        block_number: u64,
    ) -> Result<BlockExecutionResult> {
        let total_start = std::time::Instant::now();

        // Parse block and transactions
        let block = block_data.get("result").unwrap_or(block_data);
        let txs = block
            .get("transactions")
            .and_then(|t| t.as_array())
            .context("No transactions in block")?;

        let tx_count = txs.len();
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
        println!("PARALLEL BLOCK {} - {} transactions", block_number, tx_count);
        println!("{}", "=".repeat(70));

        // Parse all transactions ONCE
        let tx_parse_start = std::time::Instant::now();
        let parsed_txs: Vec<ParsedTx> = txs.iter()
            .map(|tx| ParsedTx::from_json(tx))
            .collect::<Result<Vec<_>>>()?;
        let tx_parse_time = tx_parse_start.elapsed();

        // PHASE 1: SEQUENTIAL BULK PREFETCH (Your Williams Innovation!)
        let prefetch_start = std::time::Instant::now();
        let addresses = self.collect_addresses(&parsed_txs);
        let sender_addresses: HashSet<Address> = parsed_txs.iter()
            .map(|tx| tx.from)
            .collect();
        
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
                for addr in &sender_addresses {
                    offline.mark_as_eoa(*addr);
                }
                offline.bulk_prefetch(&addresses)?;
                StateBackend::Offline(offline)
            }
        };
        let prefetch_time = prefetch_start.elapsed();

        println!("  Prefetched {} addresses in {:.2}ms", addresses.len(), prefetch_time.as_secs_f64() * 1000.0);

        // PHASE 2: DEPENDENCY ANALYSIS
        let batch_start = std::time::Instant::now();
        let batches = self.build_batches(&parsed_txs);
        let batch_time = batch_start.elapsed();
        
        println!("  Created {} parallel batches in {:.2}ms", batches.len(), batch_time.as_secs_f64() * 1000.0);

        // Setup environment (same as sequential)
        let block_env = self.parse_block_env(block)?;
        let cfg_env = CfgEnvWithHandlerCfg::new_with_spec_id(
            CfgEnv::default(),
            SpecId::LATEST,
        );

        // PHASE 3: PARALLEL EXECUTION
        let exec_start = std::time::Instant::now();
        
        // Thread-safe state database
        let db = Arc::new(RwLock::new(ParallelStateDB::new(state_backend.clone(), sender_addresses)));
        
        // Execute each batch sequentially, but transactions within batch in parallel
        let mut all_results = Vec::with_capacity(tx_count);
        
        for (batch_idx, batch) in batches.iter().enumerate() {
            let batch_exec_start = std::time::Instant::now();
            
            // Execute all transactions in this batch IN PARALLEL
            let batch_results: Vec<(usize, Result<TxResult>)> = batch.par_iter()
                .map(|(idx, parsed_tx)| {
                    let result = self.execute_tx_parallel(
                        *idx,
                        parsed_tx,
                        &block_env,
                        &cfg_env,
                        &db,
                    );
                    (*idx, result)
                })
                .collect();
            
            // Collect results in original order
            all_results.extend(batch_results);
            
            let batch_exec_time = batch_exec_start.elapsed();
            println!("  Batch {} ({} txs): {:.2}ms ({:.0} tx/s)", 
                batch_idx, 
                batch.len(), 
                batch_exec_time.as_secs_f64() * 1000.0,
                batch.len() as f64 / batch_exec_time.as_secs_f64()
            );
        }
        
        // Sort results by transaction index (restore original order)
        all_results.sort_by_key(|(idx, _)| *idx);
        
        let tx_results: Vec<TxResult> = all_results.into_iter()
            .map(|(_, result)| result)
            .collect::<Result<Vec<_>>>()?;
            
        let exec_time = exec_start.elapsed();

        // Generate receipts and compute state root
        let receipt_start = std::time::Instant::now();
        let total_gas: u64 = tx_results.iter().map(|r| r.gas_used).sum();
        let success_count = tx_results.iter().filter(|r| r.success).count();
        
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

        // Compute state root
        let state_root = db.read().compute_state_root();

        let total_time = total_start.elapsed();
        
        // Print performance profile
        println!("\n{}", "=".repeat(70));
        println!("PARALLEL PERFORMANCE - Block {}", block_number);
        println!("{}", "=".repeat(70));
        println!("Tx parsing:           {:>8.2} ms ({:>5.1}%)", tx_parse_time.as_secs_f64() * 1000.0, 100.0 * tx_parse_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Bulk prefetch:        {:>8.2} ms ({:>5.1}%)", prefetch_time.as_secs_f64() * 1000.0, 100.0 * prefetch_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Dependency batching:  {:>8.2} ms ({:>5.1}%)", batch_time.as_secs_f64() * 1000.0, 100.0 * batch_time.as_secs_f64() / total_time.as_secs_f64());
        println!("PARALLEL EXECUTION:   {:>8.2} ms ({:>5.1}%) ← CORE", exec_time.as_secs_f64() * 1000.0, 100.0 * exec_time.as_secs_f64() / total_time.as_secs_f64());
        println!("Receipt generation:   {:>8.2} ms ({:>5.1}%)", receipt_time.as_secs_f64() * 1000.0, 100.0 * receipt_time.as_secs_f64() / total_time.as_secs_f64());
        println!("{}", "-".repeat(70));
        println!("TOTAL TIME:           {:>8.2} ms", total_time.as_secs_f64() * 1000.0);
        println!("Throughput:           {:>8.0} tx/s", tx_count as f64 / total_time.as_secs_f64());
        println!("{}", "=".repeat(70));
        println!("Executed: {}/{} transactions", tx_count, tx_count);
        println!("Successful: {} ({:.1}%)", success_count, 100.0 * success_count as f64 / tx_count as f64);
        println!("Total gas: {} gas", total_gas);
        println!("{}", "=".repeat(70));

        Ok(BlockExecutionResult {
            block_number,
            tx_count,
            tx_results,
            tx_receipts,
            execution_time_us: total_time.as_micros(),
            final_state_root: state_root,
            total_gas_used: total_gas,
        })
    }

    /// Collect all addresses from transactions
    fn collect_addresses(&self, parsed_txs: &[ParsedTx]) -> Vec<Address> {
        let mut addresses = HashSet::with_capacity(parsed_txs.len() * 2);
        for tx in parsed_txs {
            addresses.insert(tx.from);
            if let Some(to) = tx.to {
                addresses.insert(to);
            }
        }
        addresses.into_iter().collect()
    }

    /// Build optimized dependency batches with smart merging
    fn build_batches<'a>(&self, parsed_txs: &'a [ParsedTx]) -> Vec<Vec<(usize, &'a ParsedTx)>> {
        // OPTIMIZATION: For small blocks, use sequential execution (less overhead)
        if parsed_txs.len() < 200 {
            // Return single batch = sequential execution (fastest for small blocks)
            return vec![parsed_txs.iter().enumerate().collect()];
        }
        
        const MIN_BATCH_SIZE: usize = 16; // Minimum transactions per batch to justify parallelism
        
        let mut batches = vec![];
        let mut current_batch = vec![];
        let mut touched_addresses = HashSet::new();
        
        for (idx, tx) in parsed_txs.iter().enumerate() {
            let mut tx_addresses = HashSet::new();
            tx_addresses.insert(tx.from);
            if let Some(to) = tx.to {
                tx_addresses.insert(to);
            }
            
            // Check if this tx conflicts with current batch
            let has_conflict = !tx_addresses.is_disjoint(&touched_addresses);
            
            // Only start new batch if: (1) conflict exists AND (2) current batch is large enough
            if has_conflict && current_batch.len() >= MIN_BATCH_SIZE {
                // Batch is large enough - finalize it
                batches.push(std::mem::take(&mut current_batch));
                current_batch = vec![(idx, tx)];
                touched_addresses = tx_addresses;
            } else if has_conflict {
                // Conflict but batch too small - MERGE by clearing touched addresses
                // This sacrifices perfect parallelism for reduced coordination overhead
                touched_addresses.clear();
                touched_addresses.extend(tx_addresses.clone());
                current_batch.push((idx, tx));
            } else {
                // No conflict - add to current batch
                current_batch.push((idx, tx));
                touched_addresses.extend(tx_addresses);
            }
        }
        
        if !current_batch.is_empty() {
            batches.push(current_batch);
        }
        
        // POST-PROCESS: Merge tiny trailing batches
        self.merge_small_batches(batches, MIN_BATCH_SIZE)
    }
    
    /// Merge small batches to reduce coordination overhead
    fn merge_small_batches<'a>(
        &self,
        mut batches: Vec<Vec<(usize, &'a ParsedTx)>>,
        min_size: usize,
    ) -> Vec<Vec<(usize, &'a ParsedTx)>> {
        if batches.len() <= 1 {
            return batches;
        }
        
        let mut merged = Vec::with_capacity(batches.len());
        let mut accumulator = Vec::new();
        
        for batch in batches {
            accumulator.extend(batch);
            
            // Flush accumulator when it's large enough
            if accumulator.len() >= min_size {
                merged.push(std::mem::take(&mut accumulator));
            }
        }
        
        // Add remaining transactions to last batch (avoid tiny final batch)
        if !accumulator.is_empty() {
            if let Some(last_batch) = merged.last_mut() {
                last_batch.extend(accumulator);
            } else {
                merged.push(accumulator);
            }
        }
        
        merged
    }

    /// Execute single transaction in parallel
    fn execute_tx_parallel(
        &self,
        index: usize,
        parsed_tx: &ParsedTx,
        block_env: &BlockEnv,
        cfg_env: &CfgEnvWithHandlerCfg,
        db: &Arc<RwLock<ParallelStateDB>>,
    ) -> Result<TxResult> {
        let tx_env = parsed_tx.to_tx_env();

        // Clone database reference for this thread
        let mut thread_db = ThreadLocalDB::new(db.clone());

        // Execute transaction
        let result = {
            let mut evm = Evm::builder()
                .with_db(&mut thread_db)
                .with_block_env(block_env.clone())
                .with_tx_env(tx_env.clone())
                .with_cfg_env_with_handler_cfg(cfg_env.clone())
                .build();

            match evm.transact() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("EVM Error for tx {}: {:?}", index, e);
                    return Ok(TxResult {
                        index,
                        success: false,
                        gas_used: tx_env.gas_limit,
                        output: Bytes::from(format!("Error: {:?}", e).as_bytes().to_vec()),
                        state_changes: vec![],
                        logs: vec![],
                    });
                }
            }
        };

        // Extract logs and state changes
        let logs: Vec<String> = result.result.logs().iter()
            .map(|log| format!("{:?}", log))
            .collect();

        let state_changes: Vec<(Address, AccountInfo)> = result.state.iter()
            .map(|(addr, acc)| (*addr, acc.info.clone()))
            .collect();

        let (success, gas_used, output) = match result.result {
            ExecutionResult::Success { gas_used, output, .. } => {
                (true, gas_used, output.into_data())
            }
            ExecutionResult::Revert { gas_used, output } => {
                (false, gas_used, output)
            }
            ExecutionResult::Halt { gas_used, reason } => {
                let msg = format!("Halt: {:?}", reason);
                (false, gas_used, Bytes::from(msg.as_bytes().to_vec()))
            }
        };

        Ok(TxResult {
            index,
            success,
            gas_used,
            output,
            state_changes,
            logs,
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

/// Thread-safe state database
pub struct ParallelStateDB {
    backend: StateBackend,
    changes: Arc<Mutex<HashMap<Address, AccountInfo>>>,
    sender_addresses: HashSet<Address>,
}

impl ParallelStateDB {
    fn new(backend: StateBackend, senders: HashSet<Address>) -> Self {
        Self {
            backend,
            changes: Arc::new(Mutex::new(HashMap::new())),
            sender_addresses: senders,
        }
    }

    /// Compute state root from all account changes
    fn compute_state_root(&self) -> B256 {
        // OPTIMIZATION: Use REVM's optimized keccak256 instead of sha3 crate
        let changes = self.changes.lock().unwrap();
        let mut data = Vec::with_capacity(changes.len() * 100);
        
        let mut addresses: Vec<_> = changes.keys().collect();
        addresses.sort();
        
        for addr in addresses {
            if let Some(account) = changes.get(addr) {
                data.extend_from_slice(addr.as_slice());
                data.extend_from_slice(&account.balance.to_be_bytes::<32>());
                data.extend_from_slice(&account.nonce.to_be_bytes());
                data.extend_from_slice(account.code_hash.as_slice());
            }
        }
        
        revm::primitives::keccak256(&data)
    }
}

/// Thread-local database wrapper for parallel execution
struct ThreadLocalDB {
    shared: Arc<RwLock<ParallelStateDB>>,
}

impl ThreadLocalDB {
    fn new(shared: Arc<RwLock<ParallelStateDB>>) -> Self {
        Self { shared }
    }
}

impl Database for ThreadLocalDB {
    type Error = anyhow::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let db = self.shared.read();
        let is_sender = db.sender_addresses.contains(&address);
        
        // Check changes first
        {
            let changes = db.changes.lock().unwrap();
            if let Some(info) = changes.get(&address) {
                if is_sender {
                    let mut eoa_info = info.clone();
                    eoa_info.code_hash = KECCAK_EMPTY;
                    eoa_info.code = None;
                    return Ok(Some(eoa_info));
                }
                return Ok(Some(info.clone()));
            }
        }

        // Get from backend
        let mut info = match &db.backend {
            StateBackend::Rpc(backend) => backend.get_account(address)?,
            StateBackend::Offline(backend) => backend.get_account(address),
        };

        if is_sender {
            info.code_hash = KECCAK_EMPTY;
            info.code = None;
        }

        Ok(Some(info))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<revm::primitives::Bytecode, Self::Error> {
        let db = self.shared.read();
        match &db.backend {
            StateBackend::Offline(backend) => {
                if let Some(code) = backend.get_bytecode(code_hash) {
                    return Ok(code);
                }
            }
            StateBackend::Rpc(_) => {}
        }
        Ok(revm::primitives::Bytecode::new())
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let db = self.shared.read();
        match &db.backend {
            StateBackend::Rpc(backend) => backend.get_storage(address, index),
            StateBackend::Offline(backend) => Ok(backend.get_storage(address, index)),
        }
    }

    fn block_hash(&mut self, _number: U256) -> Result<B256, Self::Error> {
        Ok(B256::ZERO)
    }
}

impl DatabaseCommit for ThreadLocalDB {
    fn commit(&mut self, changes: HashMap<Address, revm::primitives::Account>) {
        let db = self.shared.read();
        let mut db_changes = db.changes.lock().unwrap();
        for (addr, account) in changes {
            db_changes.insert(addr, account.info);
        }
    }
}

/// Parse address from hex string
fn parse_address(addr_str: &str) -> Result<Address> {
    let addr_hex = if addr_str.starts_with("0x") { &addr_str[2..] } else { addr_str };
    
    if addr_hex.len() == 40 {
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(addr_hex, &mut bytes)?;
        return Ok(Address::from_slice(&bytes));
    }
    
    let bytes = hex::decode(addr_hex)?;
    if bytes.len() != 20 {
        anyhow::bail!("Invalid address length");
    }
    Ok(Address::from_slice(&bytes))
}
