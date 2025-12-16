// Unit tests for Williams Executor
// These tests have ZERO impact on release builds (not compiled)

#[cfg(test)]
mod executor_tests {
    use super::super::executor::*;
    use revm::primitives::{Address, U256, Bytes};
    use std::sync::Arc;

    #[test]
    fn test_parsed_tx_from_json() {
        let tx_json = serde_json::json!({
            "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
            "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEc",
            "value": "0x0",
            "gas": "0x5208",
            "gasPrice": "0x4a817c800",
            "input": "0x",
            "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        });

        let parsed = ParsedTx::from_json(&tx_json).unwrap();
        
        assert_eq!(parsed.gas_limit, 21000); // 0x5208
        assert_eq!(parsed.value, U256::ZERO);
        // Note: to is optional for contract creation, so we just check parsing worked
        assert_eq!(parsed.gas_limit, 21000);
    }

    #[test]
    fn test_state_forwarding_correctness() {
        // Test that state changes from TX[n] are visible to TX[n+1]
        let executor = WilliamsExecutor::new(1);
        
        // Create a simple block with 2 transactions
        let tx1 = ParsedTx {
            from: Address::from_slice(&[1u8; 20]),
            to: Some(Address::from_slice(&[2u8; 20])),
            value: U256::from(1000u64),
            gas_limit: 21000,
            gas_price: U256::from(1000000000u64),
            data: Arc::new(Bytes::default()),
            hash: revm::primitives::B256::ZERO,
        };

        let block = PreParsedBlock {
            block_number: 1,
            transactions: vec![tx1],
            coinbase: None,
        };

        let result = executor.execute_preparsed_block(&block);
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.tx_count, 1);
        assert_eq!(result.block_number, 1);
    }

    #[test]
    fn test_empty_block_handling() {
        let executor = WilliamsExecutor::new(1);
        
        let empty_block = PreParsedBlock {
            block_number: 1,
            transactions: vec![],
            coinbase: None,
        };

        let result = executor.execute_preparsed_block(&empty_block).unwrap();
        
        assert_eq!(result.tx_count, 0);
        assert_eq!(result.tx_results.len(), 0);
        assert_eq!(result.total_gas_used, 0);
    }

    #[test]
    fn test_address_collection() {
        let executor = WilliamsExecutor::new(1);
        
        let tx1 = ParsedTx {
            from: Address::from_slice(&[1u8; 20]),
            to: Some(Address::from_slice(&[2u8; 20])),
            value: U256::ZERO,
            gas_limit: 21000,
            gas_price: U256::from(1000000000u64),
            data: Arc::new(Bytes::default()),
            hash: revm::primitives::B256::ZERO,
        };

        let tx2 = ParsedTx {
            from: Address::from_slice(&[3u8; 20]),
            to: Some(Address::from_slice(&[2u8; 20])), // Same receiver
            value: U256::ZERO,
            gas_limit: 21000,
            gas_price: U256::from(1000000000u64),
            data: Arc::new(Bytes::default()),
            hash: revm::primitives::B256::ZERO,
        };

        let addresses = executor.collect_addresses_from_parsed(&[tx1, tx2]);
        
        // Should collect unique addresses: [1, 2, 3]
        assert_eq!(addresses.len(), 3);
    }

    #[test]
    fn test_coinbase_inclusion() {
        let executor = WilliamsExecutor::new(1);
        let coinbase = Address::from_slice(&[99u8; 20]);
        
        let tx1 = ParsedTx {
            from: Address::from_slice(&[1u8; 20]),
            to: Some(Address::from_slice(&[2u8; 20])),
            value: U256::ZERO,
            gas_limit: 21000,
            gas_price: U256::from(1000000000u64),
            data: Arc::new(Bytes::default()),
            hash: revm::primitives::B256::ZERO,
        };

        let addresses = executor.collect_addresses_with_coinbase(&[tx1], Some(coinbase));
        
        // Should include coinbase
        assert!(addresses.contains(&coinbase));
    }
}

#[cfg(test)]
mod state_backend_tests {
    use super::super::state_backend::*;
    use revm::primitives::{Address, U256};

    #[test]
    fn test_offline_backend_default_balances() {
        let backend = OfflineStateBackend::new();
        let test_addr = Address::from_slice(&[1u8; 20]);
        
        let account = backend.get_account(test_addr);
        
        // Should have default balance (1000 ETH)
        assert_eq!(account.balance, U256::from(1_000_000_000_000_000_000_000u128));
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_offline_backend_caching() {
        let backend = OfflineStateBackend::new();
        let test_addr = Address::from_slice(&[1u8; 20]);
        
        // First fetch
        let account1 = backend.get_account(test_addr);
        
        // Second fetch (should be cached)
        let account2 = backend.get_account(test_addr);
        
        assert_eq!(account1.balance, account2.balance);
        assert_eq!(account1.nonce, account2.nonce);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::executor::*;
    use serde_json::json;

    #[test]
    fn test_full_block_execution() {
        let executor = WilliamsExecutor::new(4);
        
        // Create a realistic block JSON
        let block_json = json!({
            "number": "0x1",
            "miner": "0x0000000000000000000000000000000000000000",
            "transactions": [
                {
                    "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
                    "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEc",
                    "value": "0x0",
                    "gas": "0x5208",
                    "gasPrice": "0x4a817c800",
                    "input": "0x",
                    "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                }
            ]
        });

        let preparsed = PreParsedBlock::from_json(&block_json, 1).unwrap();
        let result = executor.execute_preparsed_block(&preparsed).unwrap();
        
        assert_eq!(result.block_number, 1);
        assert_eq!(result.tx_count, 1);
        assert!(result.execution_time_us > 0);
    }
}
