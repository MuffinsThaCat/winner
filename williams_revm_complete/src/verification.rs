// State verification utilities
// Uses the state tracking fields (pre_state, post_state, final_state_root) from BlockExecutionResult

use revm::primitives::{Address, U256, B256, AccountInfo};
use std::collections::HashMap;
use std::sync::Arc;

/// Verify and analyze state changes between pre and post execution
pub fn verify_block_execution(
    final_state_root: &B256,
    pre_state: &HashMap<Address, AccountInfo>,
    post_state: &HashMap<Address, AccountInfo>,
) -> StateVerification {
    let mut modified_accounts = 0;
    let mut balance_changes = Vec::new();
    let mut nonce_changes = Vec::new();
    
    // Compare pre and post state
    for (addr, post_info) in post_state {
        if let Some(pre_info) = pre_state.get(addr) {
            if pre_info.balance != post_info.balance {
                modified_accounts += 1;
                balance_changes.push((*addr, pre_info.balance, post_info.balance));
            }
            if pre_info.nonce != post_info.nonce {
                nonce_changes.push((*addr, pre_info.nonce, post_info.nonce));
            }
        } else {
            // New account created
            modified_accounts += 1;
        }
    }
    
    StateVerification {
        state_root: *final_state_root,
        accounts_modified: modified_accounts,
        accounts_created: post_state.len().saturating_sub(pre_state.len()),
        balance_changes,
        nonce_changes,
        storage_changes: 0, // Would need storage tracking
    }
}

/// Print detailed state changes
pub fn print_state_changes(
    block_number: u64,
    final_state_root: &B256,
    pre_state: &HashMap<Address, AccountInfo>,
    post_state: &HashMap<Address, AccountInfo>,
) {
    println!("\n📊 STATE CHANGES (Block {}):", block_number);
    println!("  State root: 0x{}", hex::encode(&final_state_root[..8]));
    
    let mut balance_changes = 0;
    let mut nonce_changes = 0;
    
    for (addr, post_info) in post_state {
        if let Some(pre_info) = pre_state.get(addr) {
            if pre_info.balance != post_info.balance {
                balance_changes += 1;
                let diff = if post_info.balance >= pre_info.balance {
                    post_info.balance.saturating_sub(pre_info.balance)
                } else {
                    pre_info.balance.saturating_sub(post_info.balance)
                };
                println!("  💰 0x{:x}: balance {} → {} (Δ{})",
                    addr, pre_info.balance, post_info.balance, diff);
            }
            if pre_info.nonce != post_info.nonce {
                nonce_changes += 1;
                println!("  🔢 0x{:x}: nonce {} → {}",
                    addr, pre_info.nonce, post_info.nonce);
            }
        }
    }
    
    println!("  Total: {} balance changes, {} nonce changes", balance_changes, nonce_changes);
}

/// State verification result
#[derive(Debug, Clone)]
pub struct StateVerification {
    pub state_root: B256,
    pub accounts_modified: usize,
    pub accounts_created: usize,
    pub balance_changes: Vec<(Address, U256, U256)>,
    pub nonce_changes: Vec<(Address, u64, u64)>,
    pub storage_changes: usize,
}

impl StateVerification {
    pub fn print(&self) {
        println!("\n✅ STATE VERIFICATION:");
        println!("  State root: 0x{}", hex::encode(&self.state_root[..8]));
        println!("  Accounts modified: {}", self.accounts_modified);
        println!("  Accounts created: {}", self.accounts_created);
        println!("  Storage changes: {}", self.storage_changes);
        println!("  Balance changes: {}", self.balance_changes.len());
        println!("  Nonce changes: {}", self.nonce_changes.len());
    }
}

/// Get state diff between two snapshots
pub fn state_diff(
    pre_state: &HashMap<Address, AccountInfo>,
    post_state: &HashMap<Address, AccountInfo>,
) -> StateDiff {
    let mut accounts_added = Vec::new();
    let mut accounts_modified = Vec::new();
    let mut total_balance_delta = U256::ZERO;
    
    // Find added and modified accounts
    for (addr, post_info) in post_state {
        if let Some(pre_info) = pre_state.get(addr) {
            if pre_info.balance != post_info.balance || 
               pre_info.nonce != post_info.nonce ||
               pre_info.code_hash != post_info.code_hash {
                accounts_modified.push(*addr);
                
                // Track balance changes
                if post_info.balance >= pre_info.balance {
                    total_balance_delta = total_balance_delta.saturating_add(
                        post_info.balance.saturating_sub(pre_info.balance)
                    );
                }
            }
        } else {
            accounts_added.push(*addr);
            total_balance_delta = total_balance_delta.saturating_add(post_info.balance);
        }
    }
    
    // Find removed accounts
    let accounts_removed = pre_state.keys()
        .filter(|&addr| !post_state.contains_key(addr))
        .copied()
        .collect();
    
    StateDiff {
        accounts_added,
        accounts_modified,
        accounts_removed,
        total_balance_delta,
    }
}

/// State difference between two snapshots
#[derive(Debug, Clone)]
pub struct StateDiff {
    pub accounts_added: Vec<Address>,
    pub accounts_modified: Vec<Address>,
    pub accounts_removed: Vec<Address>,
    pub total_balance_delta: U256,
}

impl StateDiff {
    pub fn print(&self) {
        println!("\n🔍 STATE DIFF:");
        println!("  Accounts added: {}", self.accounts_added.len());
        println!("  Accounts modified: {}", self.accounts_modified.len());
        println!("  Accounts removed: {}", self.accounts_removed.len());
        println!("  Total balance Δ: {}", self.total_balance_delta);
    }
}
