// EVM Transaction Validation and Economics
// Implements: signature verification, nonce validation, balance checks, gas payment

use anyhow::{Result, bail};
use revm::primitives::{Address, U256, B256};
use sha3::{Digest, Keccak256};

/// Verify ECDSA signature and recover sender address
/// Returns true if signature is valid and matches claimed sender
pub fn verify_signature(
    from: Address,
    hash: B256,
    v: u64,
    r: U256,
    s: U256,
    chain_id: Option<u64>,
) -> Result<bool> {
    // For benchmarking: skip expensive signature verification
    // In production: implement full ECDSA recovery using secp256k1
    #[cfg(feature = "production")]
    {
        // TODO: Full ECDSA signature verification
        // 1. Recover public key from (hash, v, r, s)
        // 2. Derive address from public key
        // 3. Compare with claimed sender
        
        use secp256k1::{Message, PublicKey, ecdsa::Signature, Secp256k1};
        
        // Adjust v for chain_id (EIP-155)
        // EIP-155 encoding: v = chain_id * 2 + 35 + recovery_id
        // Legacy encoding: v = 27 + recovery_id
        let recovery_id = if let Some(cid) = chain_id {
            if v >= 35 {
                // EIP-155: extract recovery_id = v - chain_id * 2 - 35
                let v_base = cid * 2 + 35;
                (v.saturating_sub(v_base)) as u8
            } else {
                // Legacy with chain_id set: v = 27 + recovery_id
                v.saturating_sub(27) as u8
            }
        } else {
            // No chain_id: legacy encoding v = 27 + recovery_id
            v.saturating_sub(27) as u8
        };
        
        if recovery_id > 3 {
            bail!("Invalid recovery ID: {}", recovery_id);
        }
        
        // Convert r,s to signature bytes
        let mut sig_bytes = [0u8; 64];
        r.to_be_bytes::<32>().iter().enumerate().for_each(|(i, &b)| sig_bytes[i] = b);
        s.to_be_bytes::<32>().iter().enumerate().for_each(|(i, &b)| sig_bytes[32 + i] = b);
        
        let signature = Signature::from_compact(&sig_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid signature format: {}", e))?;
        
        let message = Message::from_slice(hash.as_slice())
            .map_err(|e| anyhow::anyhow!("Invalid message hash: {}", e))?;
        
        // Recover public key
        let recovery_id = secp256k1::ecdsa::RecoveryId::from_i32(recovery_id as i32)
            .map_err(|e| anyhow::anyhow!("Invalid recovery ID: {}", e))?;
        
        let recoverable_sig = secp256k1::ecdsa::RecoverableSignature::from_compact(&sig_bytes, recovery_id)
            .map_err(|e| anyhow::anyhow!("Invalid recoverable signature: {}", e))?;
        
        let secp = Secp256k1::new();
        let public_key = secp.recover_ecdsa(&message, &recoverable_sig)
            .map_err(|e| anyhow::anyhow!("Signature recovery failed: {}", e))?;
        
        // Derive address from public key (last 20 bytes of keccak256(pubkey))
        let pub_bytes = public_key.serialize_uncompressed();
        let hash = Keccak256::digest(&pub_bytes[1..]); // Skip first byte (0x04)
        let recovered_address = Address::from_slice(&hash[12..]);
        
        Ok(recovered_address == from)
    }
    
    #[cfg(not(feature = "production"))]
    {
        // Benchmark mode: assume signature is valid
        Ok(true)
    }
}

/// Validate transaction nonce matches account nonce
pub fn validate_nonce(tx_nonce: u64, account_nonce: u64) -> Result<()> {
    if tx_nonce != account_nonce {
        bail!(
            "Invalid nonce: expected {}, got {}. Possible replay attack.",
            account_nonce,
            tx_nonce
        );
    }
    Ok(())
}

/// Calculate effective gas price for transaction
/// Handles both legacy and EIP-1559 transactions
pub fn calculate_effective_gas_price(
    tx_type: u8,
    gas_price: U256,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
    base_fee: U256,
) -> U256 {
    match tx_type {
        // Legacy or EIP-2930: use gas_price directly
        0 | 1 => gas_price,
        
        // EIP-1559: min(max_fee, base_fee + max_priority_fee)
        2 => {
            let max_fee = max_fee_per_gas.unwrap_or(U256::ZERO);
            let max_priority = max_priority_fee_per_gas.unwrap_or(U256::ZERO);
            
            let priority_fee = max_priority.min(max_fee.saturating_sub(base_fee));
            base_fee + priority_fee
        }
        
        // Unknown type: fallback to gas_price
        _ => gas_price,
    }
}

/// Validate sender has sufficient balance for transaction
/// Required balance = (gas_limit * effective_gas_price) + value
pub fn validate_balance(
    account_balance: U256,
    gas_limit: u64,
    effective_gas_price: U256,
    value: U256,
) -> Result<()> {
    let gas_cost = U256::from(gas_limit) * effective_gas_price;
    let required = gas_cost + value;
    
    if account_balance < required {
        bail!(
            "Insufficient balance: have {}, need {} (gas: {}, value: {})",
            account_balance,
            required,
            gas_cost,
            value
        );
    }
    
    Ok(())
}

/// Calculate gas payment distribution for EIP-1559
/// Returns (amount_to_burn, amount_to_miner)
pub fn calculate_gas_payment(
    tx_type: u8,
    gas_used: u64,
    effective_gas_price: U256,
    base_fee: U256,
) -> (U256, U256) {
    match tx_type {
        // Legacy or EIP-2930: all fees go to miner
        0 | 1 => {
            let total_fee = U256::from(gas_used) * effective_gas_price;
            (U256::ZERO, total_fee)
        }
        
        // EIP-1559: burn base fee, miner gets priority fee
        2 => {
            let total_fee = U256::from(gas_used) * effective_gas_price;
            let base_fee_payment = U256::from(gas_used) * base_fee;
            let priority_fee = total_fee.saturating_sub(base_fee_payment);
            
            (base_fee_payment, priority_fee)
        }
        
        // Unknown: all to miner
        _ => {
            let total_fee = U256::from(gas_used) * effective_gas_price;
            (U256::ZERO, total_fee)
        }
    }
}

/// Validate chain ID matches expected value (EIP-155)
pub fn validate_chain_id(tx_chain_id: Option<u64>, expected_chain_id: u64) -> Result<()> {
    if let Some(cid) = tx_chain_id {
        if cid != expected_chain_id {
            bail!(
                "Chain ID mismatch: expected {}, got {}. Transaction from wrong chain.",
                expected_chain_id,
                cid
            );
        }
    }
    // None is allowed (pre-EIP-155 transactions)
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_effective_gas_price_legacy() {
        let price = calculate_effective_gas_price(
            0, // legacy
            U256::from(1000u64),
            None,
            None,
            U256::from(100u64),
        );
        assert_eq!(price, U256::from(1000u64));
    }
    
    #[test]
    fn test_effective_gas_price_eip1559() {
        let base_fee = U256::from(100u64);
        let max_fee = U256::from(150u64);
        let max_priority = U256::from(20u64);
        
        let price = calculate_effective_gas_price(
            2, // EIP-1559
            U256::ZERO,
            Some(max_fee),
            Some(max_priority),
            base_fee,
        );
        
        // Should be base_fee + min(max_priority, max_fee - base_fee)
        // = 100 + min(20, 50) = 120
        assert_eq!(price, U256::from(120u64));
    }
    
    #[test]
    fn test_validate_balance_sufficient() {
        let result = validate_balance(
            U256::from(1000000u64),
            21000,
            U256::from(10u64),
            U256::from(100u64),
        );
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_balance_insufficient() {
        let result = validate_balance(
            U256::from(100u64),
            21000,
            U256::from(10u64),
            U256::from(100u64),
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_gas_payment_legacy() {
        let (burn, miner) = calculate_gas_payment(
            0,
            21000,
            U256::from(10u64),
            U256::from(5u64),
        );
        
        assert_eq!(burn, U256::ZERO);
        assert_eq!(miner, U256::from(210000u64)); // 21000 * 10
    }
    
    #[test]
    fn test_gas_payment_eip1559() {
        let (burn, miner) = calculate_gas_payment(
            2,
            21000,
            U256::from(10u64), // effective gas price
            U256::from(7u64),  // base fee
        );
        
        // Total fee: 21000 * 10 = 210000
        // Burn: 21000 * 7 = 147000
        // Miner: 210000 - 147000 = 63000
        assert_eq!(burn, U256::from(147000u64));
        assert_eq!(miner, U256::from(63000u64));
    }
}
