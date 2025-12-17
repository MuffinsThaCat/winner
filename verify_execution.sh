#!/bin/bash
# Verify that transactions are actually executing through EVM

echo "═══════════════════════════════════════════════════════════════════"
echo "VERIFICATION TEST: Are Transactions Really Executing?"
echo "═══════════════════════════════════════════════════════════════════"
echo ""

# Run on just 10 blocks to see detailed output
./williams_revm_complete/target/release/williams-complete data_bdf 16 2>&1 | head -500 | grep -E "EVM EXECUTION|gas_used|Executed|transact"

echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "If we see 'EVM EXECUTION' times and gas_used values, txs are real"
echo "═══════════════════════════════════════════════════════════════════"
