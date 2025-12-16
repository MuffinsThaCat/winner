#!/bin/bash
# Simple test to verify 3-transaction state forwarding

cd "$(dirname "$0")"

# Create a minimal test block with 3 transactions
cat > /tmp/test_block.json << 'EOF'
{
  "number": "0x1",
  "miner": "0xCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
  "timestamp": "0x0",
  "gasLimit": "0x1000000",
  "transactions": [
    {
      "from": "0x1111111111111111111111111111111111111111",
      "to": "0x2222222222222222222222222222222222222222",
      "value": "0x8AC7230489E80000",
      "gas": "0x5208",
      "gasPrice": "0x3B9ACA00",
      "nonce": "0x0",
      "hash": "0x0000000000000000000000000000000000000000000000000000000000000001"
    },
    {
      "from": "0x2222222222222222222222222222222222222222",
      "to": "0x3333333333333333333333333333333333333333",
      "value": "0x4563918244F40000",
      "gas": "0x5208",
      "gasPrice": "0x3B9ACA00",
      "nonce": "0x0",
      "hash": "0x0000000000000000000000000000000000000000000000000000000000000002"
    },
    {
      "from": "0x3333333333333333333333333333333333333333",
      "to": "0x1111111111111111111111111111111111111111",
      "value": "0x1BC16D674EC80000",
      "gas": "0x5208",
      "gasPrice": "0x3B9ACA00",
      "nonce": "0x0",
      "hash": "0x0000000000000000000000000000000000000000000000000000000000000003"
    }
  ]
}
EOF

# Create temporary data directory
mkdir -p /tmp/test_data/blocks
cp /tmp/test_block.json /tmp/test_data/blocks/

echo "========================================="
echo "Testing 3 Conflicting Transactions"
echo "========================================="
echo ""
echo "Transaction 1: Alice (0x1111...) sends 10 ETH to Bob (0x2222...)"
echo "Transaction 2: Bob (0x2222...) sends 5 ETH to Charlie (0x3333...)"
echo "Transaction 3: Charlie (0x3333...) sends 2 ETH to Alice (0x1111...)"
echo ""
echo "CRITICAL: These transactions are CONFLICTING (same addresses)"
echo "They MUST execute sequentially with state forwarding"
echo ""
echo "========================================="
echo ""

# Run the executor
./target/release/williams-complete /tmp/test_data 1

# Cleanup
rm -rf /tmp/test_data /tmp/test_block.json

echo ""
echo "========================================="
echo "Test Complete"
echo "========================================="
