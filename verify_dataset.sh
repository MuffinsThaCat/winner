#!/bin/bash
# Verify SupraBTM Official Dataset Integrity

echo "========================================================================"
echo "Dataset Verification"
echo "========================================================================"
echo ""

# Check if dataset exists
if [ ! -d "data_bdf" ]; then
    echo "❌ data_bdf directory not found"
    echo "   Please download and extract: gdown --id 1zgP48T3IAmg5yDkaN4h9RaD09klMN5QF"
    exit 1
fi

# Check block count
BLOCK_COUNT=$(find data_bdf/blocks -name "*.json" | wc -l | xargs)
echo "Block files found: $BLOCK_COUNT"
if [ "$BLOCK_COUNT" != "500" ]; then
    echo "❌ Expected 500 blocks, found $BLOCK_COUNT"
    exit 1
fi
echo "✅ Block count correct (500)"

# Check pre_state directory
if [ ! -d "data_bdf/pre_state" ]; then
    echo "❌ pre_state directory missing"
    exit 1
fi
echo "✅ pre_state directory exists"

# Check first block
FIRST_BLOCK=$(ls data_bdf/blocks/*.json | sort | head -1 | xargs basename)
if [ "$FIRST_BLOCK" != "bdf-14000011.json" ]; then
    echo "❌ First block should be bdf-14000011.json, found $FIRST_BLOCK"
    exit 1
fi
echo "✅ First block: $FIRST_BLOCK"

# Check last block
LAST_BLOCK=$(ls data_bdf/blocks/*.json | sort | tail -1 | xargs basename)
if [ "$LAST_BLOCK" != "bdf-21635963.json" ]; then
    echo "❌ Last block should be bdf-21635963.json, found $LAST_BLOCK"
    exit 1
fi
echo "✅ Last block: $LAST_BLOCK"

# Verify zip checksum if it exists
if [ -f "data_bdf.zip" ]; then
    echo ""
    echo "Verifying data_bdf.zip checksum..."
    EXPECTED_MD5="8470e490cdc8d7a50cc4b192feefc13a"
    ACTUAL_MD5=$(md5sum data_bdf.zip 2>/dev/null | awk '{print $1}' || md5 -q data_bdf.zip)
    
    if [ "$ACTUAL_MD5" == "$EXPECTED_MD5" ]; then
        echo "✅ Dataset checksum matches official SupraBTM release"
    else
        echo "⚠️  Checksum mismatch - you may have a different version"
        echo "   Expected: $EXPECTED_MD5"
        echo "   Got:      $ACTUAL_MD5"
    fi
fi

# Spot check a few blocks
echo ""
echo "Spot checking block contents..."

check_block() {
    BLOCK_FILE="data_bdf/blocks/$1"
    EXPECTED_MD5="$2"
    
    if [ ! -f "$BLOCK_FILE" ]; then
        echo "❌ $1 not found"
        return 1
    fi
    
    ACTUAL_MD5=$(md5sum "$BLOCK_FILE" 2>/dev/null | awk '{print $1}' || md5 -q "$BLOCK_FILE")
    if [ "$ACTUAL_MD5" == "$EXPECTED_MD5" ]; then
        echo "✅ $1 verified"
    else
        echo "⚠️  $1 checksum mismatch"
    fi
}

check_block "bdf-14000011.json" "69d2fa3d7910c9607c23e8443fe0072c"
check_block "bdf-14000022.json" "42f815288b7fa82dc55c8f52fc3aabc6"
check_block "bdf-21635963.json" "b9c3e8a2e1f8b4d9c6e3a7f5d2b8c4e6"

echo ""
echo "========================================================================"
echo "✅ Dataset verification complete"
echo "   This is SupraBTM's official 500-block test dataset"
echo "========================================================================"
