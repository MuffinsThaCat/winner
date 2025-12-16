#!/bin/bash
# Pre-flight check: Verify everything is ready for definitive benchmark

echo "======================================================================"
echo "PRE-FLIGHT CHECK: Definitive Benchmark Readiness"
echo "======================================================================"
echo ""

ALL_GOOD=true

# Check 1: Data directory
echo "[ 1/6 ] Checking data directory..."
if [ -d "data_100k/blocks" ]; then
    BLOCK_COUNT=$(ls data_100k/blocks/*.json | wc -l | tr -d ' ')
    echo "        ✓ Found data_100k/blocks/ with $BLOCK_COUNT files"
    
    # Check for specific blocks 18000000-18000499
    FIRST_BLOCK="data_100k/blocks/bdf-18000000.json"
    LAST_BLOCK="data_100k/blocks/bdf-18000499.json"
    
    if [ -f "$FIRST_BLOCK" ] && [ -f "$LAST_BLOCK" ]; then
        echo "        ✓ Blocks 18000000-18000499 are present"
    else
        echo "        ✗ Missing blocks in range 18000000-18000499"
        ALL_GOOD=false
    fi
else
    echo "        ✗ data_100k/blocks/ not found"
    ALL_GOOD=false
fi
echo ""

# Check 2: Williams executor
echo "[ 2/6 ] Checking Williams executor..."
if [ -d "williams_revm_complete" ]; then
    echo "        ✓ Williams directory exists"
    
    if [ -f "williams_revm_complete/target/release/williams-complete" ]; then
        echo "        ✓ Williams binary is built"
    else
        echo "        ⚠ Williams not built (will build during benchmark)"
    fi
else
    echo "        ✗ williams_revm_complete/ not found"
    ALL_GOOD=false
fi
echo ""

# Check 3: Docker
echo "[ 3/6 ] Checking Docker..."
DOCKER="/Applications/Docker.app/Contents/Resources/bin/docker"
if [ -f "$DOCKER" ]; then
    echo "        ✓ Docker Desktop found"
    
    if $DOCKER ps &> /dev/null; then
        echo "        ✓ Docker daemon is running"
    else
        echo "        ✗ Docker daemon not running (please start Docker Desktop)"
        ALL_GOOD=false
    fi
else
    echo "        ✗ Docker Desktop not found at $DOCKER"
    ALL_GOOD=false
fi
echo ""

# Check 4: SupraBTM Docker image
echo "[ 4/6 ] Checking SupraBTM Docker image..."
if $DOCKER images | grep -q "rohitkapoor9312/ibtm-image"; then
    echo "        ✓ SupraBTM image found locally"
else
    echo "        ⚠ SupraBTM image not cached (will download during benchmark)"
fi
echo ""

# Check 5: Disk space
echo "[ 5/6 ] Checking disk space..."
FREE_SPACE=$(df -h . | tail -1 | awk '{print $4}')
echo "        ✓ Free space: $FREE_SPACE"
echo ""

# Check 6: System resources
echo "[ 6/6 ] Checking system resources..."
CPU_COUNT=$(sysctl -n hw.ncpu 2>/dev/null || echo "unknown")
MEMORY=$(sysctl -n hw.memsize 2>/dev/null | awk '{print int($1/1024/1024/1024)"GB"}')
echo "        ✓ CPUs: $CPU_COUNT"
echo "        ✓ Memory: $MEMORY"
echo ""

# Final verdict
echo "======================================================================"
if [ "$ALL_GOOD" = true ]; then
    echo "✓ ALL CHECKS PASSED - Ready for definitive benchmark"
    echo ""
    echo "Run the benchmark with:"
    echo "  ./run_definitive_comparison.sh"
else
    echo "✗ SOME CHECKS FAILED - Please fix issues above"
    echo ""
    echo "Common fixes:"
    echo "  - Start Docker Desktop if not running"
    echo "  - Verify data_100k/ directory is present"
    echo "  - Check williams_revm_complete/ directory exists"
fi
echo "======================================================================"
echo ""
