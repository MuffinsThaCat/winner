// Criterion benchmarks for executor performance
// Run with: cargo bench

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use williams_hybrid_complete::executor::{WilliamsExecutor, PreParsedBlock, ParsedTx};
use revm::primitives::{Address, U256, Bytes, B256};
use std::sync::Arc;

fn create_test_block(tx_count: usize) -> PreParsedBlock {
    let transactions: Vec<ParsedTx> = (0..tx_count)
        .map(|i| ParsedTx {
            from: Address::from_slice(&[(i % 256) as u8; 20]),
            to: Some(Address::from_slice(&[((i + 1) % 256) as u8; 20])),
            value: U256::from(1000u64),
            gas_limit: 21000,
            gas_price: U256::from(1000000000u64),
            data: Arc::new(Bytes::default()),
            hash: B256::ZERO,
            nonce: i as u64,
            chain_id: Some(1),
            tx_type: 0,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            access_list: vec![],
            v: 27,
            r: U256::ZERO,
            s: U256::ZERO,
        })
        .collect();
    
    PreParsedBlock {
        block_number: 1,
        transactions,
        coinbase: Some(Address::ZERO),
    }
}

fn bench_block_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_execution");
    group.sample_size(10);
    
    // Test large block to show real throughput
    let tx_count = 1000;
    group.bench_with_input(
        BenchmarkId::from_parameter(tx_count),
        &tx_count,
        |b, &tx_count| {
            let executor = WilliamsExecutor::new(4);
            let block = create_test_block(tx_count);
            
            b.iter(|| {
                executor.execute_preparsed_block(&block).expect("execution failed")
            });
        },
    );
    
    group.finish();
}

fn bench_address_collection(c: &mut Criterion) {
    let executor = WilliamsExecutor::new(4);
    let block = create_test_block(100);
    
    c.bench_function("address_collection_100tx", |b| {
        b.iter(|| {
            executor.collect_addresses_from_parsed(&block.transactions)
        });
    });
}

criterion_group!(benches, bench_block_execution, bench_address_collection);
criterion_main!(benches);
