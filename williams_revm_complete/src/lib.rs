// Williams Hybrid Executor - Library Interface
// Exposes modules for testing and benchmarking

pub mod state_backend;
pub mod executor;
pub mod parallel_executor;
pub mod williams_compression;

#[cfg(feature = "production")]
pub mod observability;

#[cfg(feature = "production")]
pub mod evm_validation;

// Re-export commonly used types
pub use executor::{WilliamsExecutor, PreParsedBlock, ParsedTx, BlockExecutionResult, TxResult};
pub use parallel_executor::WilliamsParallelExecutor;
pub use state_backend::{RpcStateBackend, OfflineStateBackend};
pub use williams_compression::{WilliamsPhiStateManager, CompressionStats, CompressionConfig};

#[cfg(test)]
mod tests;
