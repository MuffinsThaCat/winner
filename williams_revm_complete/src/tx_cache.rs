// Transaction cache to avoid repeated JSON parsing
use revm::primitives::{Address, U256, Bytes};
use serde_json::Value;
use anyhow::Result;

/// Cached transaction data (parsed once)
pub struct CachedTx {
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Bytes,
    pub gas_limit: u64,
    pub gas_price: U256,
}

impl CachedTx {
    pub fn from_json(tx: &Value, parse_address_fn: fn(&str) -> Result<Address>) -> Result<Self> {
        let from = tx.get("from")
            .and_then(|v| v.as_str())
            .and_then(|s| parse_address_fn(s).ok());
        
        let to = tx.get("to")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .and_then(|s| parse_address_fn(s).ok());
        
        let value = tx.get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.trim_start_matches("0x");
                U256::from_str_radix(s, 16).ok()
            })
            .unwrap_or(U256::ZERO);
        
        let data = tx.get("input")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.trim_start_matches("0x");
                hex::decode(s).ok()
            })
            .map(Bytes::from)
            .unwrap_or_default();
        
        let gas_limit = tx.get("gas")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.trim_start_matches("0x");
                u64::from_str_radix(s, 16).ok()
            })
            .unwrap_or(30_000_000);
        
        let gas_price = tx.get("gasPrice")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.trim_start_matches("0x");
                U256::from_str_radix(s, 16).ok()
            })
            .unwrap_or(U256::ZERO);
        
        Ok(CachedTx {
            from,
            to,
            value,
            data,
            gas_limit,
            gas_price,
        })
    }
}
