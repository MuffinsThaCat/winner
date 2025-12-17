// Williams Hybrid Executor - Library Interface
// Exposes modules for testing and benchmarking

pub mod state_backend;
pub mod executor;
pub mod parallel_executor;

#[cfg(feature = "production")]
pub mod observability;

#[cfg(feature = "production")]
pub mod evm_validation;
pub mod verification;

// Re-export commonly used types
pub use executor::{WilliamsExecutor, PreParsedBlock, ParsedTx, BlockExecutionResult, TxResult};
pub use state_backend::{RpcStateBackend, OfflineStateBackend};
pub use parallel_executor::WilliamsParallelExecutor;
pub use verification::{verify_block_execution, print_state_changes, StateVerification, StateDiff, state_diff};

#[cfg(test)]
mod tests;
