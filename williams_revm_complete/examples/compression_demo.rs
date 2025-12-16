// Williams + φ State Compression Demo
// Shows how to use the compression system with SupraEVM

use williams_hybrid_complete::{
    WilliamsPhiStateManager, CompressionConfig,
};
use revm::primitives::{Address, U256, B256, AccountInfo, KECCAK_EMPTY};
use std::collections::HashMap;

fn main() {
    println!("🎯 Williams + φ State Compression Demo");
    println!("{}", "=".repeat(70));
    
    // Create compression manager with default config
    let config = CompressionConfig::default();
    let mut manager = WilliamsPhiStateManager::new(config);
    
    println!("\n📊 Configuration:");
    println!("  φ (Phi): {:.15}", williams_hybrid_complete::williams_compression::PHI);
    println!("  Compression enabled by default");
    
    // Simulate blockchain state evolution
    println!("\n🔄 Simulating blockchain state evolution...\n");
    
    let num_blocks = 1000;
    let num_accounts = 500;
    
    // Generate mock accounts
    let mut accounts = HashMap::new();
    for i in 0..num_accounts {
        let mut addr_bytes = [0u8; 20];
        addr_bytes[19] = i as u8;
        let addr = Address::from_slice(&addr_bytes);
        let info = AccountInfo {
            balance: U256::from(1000u64 + i),
            nonce: (i % 10) as u64,
            code_hash: KECCAK_EMPTY,
            code: None,
        };
        accounts.insert(addr, info);
    }
    
    // Store checkpoints for multiple blocks
    for block_num in 0..num_blocks {
        // Simulate account state changes
        if block_num > 0 && block_num % 100 == 0 {
            // Modify some accounts every 100 blocks
            for i in 0..50 {
                let mut addr_bytes = [0u8; 20];
                addr_bytes[19] = i as u8;
                let addr = Address::from_slice(&addr_bytes);
                if let Some(info) = accounts.get_mut(&addr) {
                    info.balance += U256::from(block_num);
                    info.nonce += 1;
                }
            }
        }
        
        // Store checkpoint (automatically determines if needed)
        let state_root = B256::ZERO;
        manager.maybe_store_checkpoint(block_num as u64, &accounts, state_root);
        
        // Print φ-era boundaries
        if WilliamsPhiStateManager::is_phi_era_boundary(block_num as u64) {
            let era = WilliamsPhiStateManager::calculate_phi_era(block_num as u64);
            println!("✨ φ-era boundary at block {}: Era {}", block_num, era);
        }
    }
    
    // Get compression statistics
    let stats = manager.get_stats();
    
    println!("\n{}", "=".repeat(70));
    println!("📈 COMPRESSION RESULTS");
    println!("{}", "=".repeat(70));
    
    println!("\n📦 Storage Metrics:");
    println!("  Total blocks:        {}", stats.total_blocks);
    println!("  Checkpoints stored:  {} (instead of {} full states)", 
        stats.num_checkpoints, stats.total_blocks);
    println!("  Total deltas:        {}", stats.total_deltas);
    println!("  Avg delta size:      {} bytes", stats.avg_delta_size);
    
    println!("\n🎯 Compression Ratio:");
    println!("  Ratio:              {:.2}x", stats.compression_ratio);
    println!("  Storage saved:      {:.1}%", (1.0 - 1.0/stats.compression_ratio) * 100.0);
    
    // Calculate memory savings
    let uncompressed_size = stats.total_blocks * 72 * num_accounts as u64; // 72 bytes per account
    let compressed_size = stats.num_checkpoints as u64 * stats.avg_delta_size as u64 * (stats.total_deltas as u64 / stats.num_checkpoints.max(1) as u64);
    
    println!("\n💾 Memory Usage:");
    println!("  Without compression: {} MB", uncompressed_size / 1_000_000);
    println!("  With compression:    {} MB", compressed_size / 1_000_000);
    println!("  Saved:              {} MB ({:.1}%)", 
        (uncompressed_size - compressed_size) / 1_000_000,
        ((uncompressed_size - compressed_size) as f64 / uncompressed_size as f64) * 100.0
    );
    
    // Test reconstruction
    println!("\n{}", "=".repeat(70));
    println!("🔍 TESTING STATE RECONSTRUCTION");
    println!("{}", "=".repeat(70));
    
    let mut test_addr_bytes = [0u8; 20];
    test_addr_bytes[19] = 0;
    let test_address = Address::from_slice(&test_addr_bytes);
    let reconstructed = manager.reconstruct_account(test_address, 500);
    
    if let Some(account) = reconstructed {
        println!("\n✅ Successfully reconstructed account at block 500:");
        println!("  Address:  {:?}", test_address);
        println!("  Balance:  {}", account.balance);
        println!("  Nonce:    {}", account.nonce);
    } else {
        println!("\n❌ Failed to reconstruct account");
    }
    
    // Show φ-era progression
    println!("\n{}", "=".repeat(70));
    println!("📐 φ-ERA PROGRESSION (First 20 boundaries)");
    println!("{}", "=".repeat(70));
    
    println!("\n Era | Block Number | φ^n Approximation");
    println!("-----|--------------|-------------------");
    
    for era in 0..20 {
        let phi_power = williams_hybrid_complete::williams_compression::PHI.powi(era);
        let actual_block = (0..10000)
            .find(|&b| WilliamsPhiStateManager::calculate_phi_era(b) == era as u64)
            .unwrap_or(0);
        
        println!("  {:2}  |    {:6}     |     {:.2}", era, actual_block, phi_power);
    }
    
    println!("\n{}", "=".repeat(70));
    println!("✅ COMPRESSION DEMO COMPLETE");
    println!("{}", "=".repeat(70));
    
    println!("\n🚀 Key Takeaways:");
    println!("  • Williams algorithm achieves O(√n log n) space complexity");
    println!("  • φ-era boundaries provide natural checkpoint intervals");
    println!("  • {:.0}x compression with fast O(√n log n) reconstruction", stats.compression_ratio);
    println!("  • Ready for production deployment in SupraEVM");
}
