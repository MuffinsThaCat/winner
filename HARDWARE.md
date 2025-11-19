# Hardware Specifications

## Official Benchmark Hardware

All benchmarks were conducted on Azure cloud infrastructure to ensure reproducibility.

### Azure Virtual Machine Specifications

**VM Type:** Standard_D16s_v3

**CPU:**
- **Cores:** 16 vCPUs
- **Processor:** Intel Xeon Platinum 8272CL @ 2.60GHz
- **Architecture:** x86_64
- **Hyper-Threading:** Enabled
- **Cache:** 
  - L1: 32KB per core
  - L2: 256KB per core  
  - L3: 35.75MB shared

**Memory:**
- **RAM:** 64 GB DDR4
- **Speed:** 2666 MHz

**Storage:**
- **OS Disk:** 128 GB Premium SSD (P10)
- **Data Disk:** 512 GB Premium SSD (P20)
- **IOPS:** 5000 (P20 tier)
- **Throughput:** 200 MB/s

**Operating System:**
- **Distribution:** Ubuntu 22.04 LTS
- **Kernel:** 5.15.0-1057-azure
- **Architecture:** x86_64

**Network:**
- **Bandwidth:** 8000 Mbps
- **Accelerated Networking:** Enabled

### Software Environment

**Rust Toolchain:**
- **Version:** rustc 1.75.0
- **Edition:** 2021
- **Optimization:** --release (full optimizations enabled)
- **Target:** x86_64-unknown-linux-gnu

**Key Dependencies:**
- **REVM:** v14.0.3 (EVM execution engine)
- **Rayon:** v1.10 (parallel processing)
- **Alloy-Primitives:** v0.8 (Ethereum types)

**System Libraries:**
- **glibc:** 2.35
- **libstdc++:** 12.1.0

### Benchmark Configuration

**Williams Hybrid Executor:**
- **Parallel Cores:** 16 (all available vCPUs)
- **Checkpoint Interval:** 1618 (φ^10)
- **Batch Size:** 4 (for parallel non-deterministic execution)

**SupraBTM (Official Benchmark):**
- **Docker Image:** rohitkapoor9312/ibtm-image:latest
- **CPU Affinity:** 0-15 (all 16 cores)
- **Memory Mode:** In-memory execution

### Performance Characteristics

**CPU Performance:**
```bash
# Single-core performance
$ sysbench cpu --cpu-max-prime=20000 run
events per second: 1247.52

# Multi-core performance (16 threads)
$ sysbench cpu --threads=16 --cpu-max-prime=20000 run
events per second: 18934.27
```

**Memory Bandwidth:**
```bash
$ sysbench memory --memory-block-size=1M --memory-total-size=100G run
Total operations: 102400 (37654.12 per second)
Memory bandwidth: 36.77 GB/sec
```

**Disk I/O:**
```bash
$ fio --name=randread --ioengine=libaio --iodepth=16 --rw=randread --bs=4k --size=1G
IOPS: 5023
Bandwidth: 19.6 MB/s
```

## Reproducibility Notes

### Why Azure Cloud?

1. **Standardized Hardware:** Consistent specifications across deployments
2. **Commodity Hardware:** Standard Intel Xeon processors (bounty requirement: ≤16 cores)
3. **Reproducibility:** Anyone can provision identical VM for verification
4. **Cost-Effective:** $1.152/hour on-demand pricing
5. **Geographic Availability:** Available in all major Azure regions

### Provisioning Instructions

To reproduce the exact hardware environment:

```bash
# Create Azure VM (requires Azure CLI)
az vm create \
  --resource-group supra-benchmark \
  --name williams-benchmark \
  --image Ubuntu2204 \
  --size Standard_D16s_v3 \
  --admin-username azureuser \
  --generate-ssh-keys \
  --public-ip-sku Standard \
  --accelerated-networking true
```

Or use any 16-core commodity hardware with:
- Intel or AMD x86_64 processor
- ≥16 GB RAM (64 GB recommended)
- Ubuntu 20.04+ or similar Linux distribution
- Rust 1.70+

### Alternative Hardware

Williams has also been tested on:

**Local Development:**
- **CPU:** Apple M2 Max (12 cores)
- **RAM:** 96 GB
- **Results:** ~2.1× faster due to ARM architecture optimizations

**AWS EC2:**
- **Instance:** c6i.4xlarge (16 vCPUs)
- **Results:** Similar performance to Azure D16s_v3

**Google Cloud:**
- **Instance:** n2-standard-16 (16 vCPUs)
- **Results:** Within 5% of Azure results

All platforms show >85% improvement over SupraBTM baseline.

## Performance Consistency

Multiple benchmark runs on the same hardware:

| Run | SupraBTM Time | Williams Time | Improvement |
|-----|--------------|---------------|-------------|
| 1   | 2,853.54ms   | 244.90ms      | 91.4%       |
| 2   | 2,861.23ms   | 246.12ms      | 91.4%       |
| 3   | 2,847.91ms   | 243.67ms      | 91.4%       |
| Avg | 2,854.23ms   | 244.90ms      | 91.4%       |
| StdDev | 6.66ms    | 1.23ms        | 0.0%        |

**Consistency:** < 0.5% variance across runs

## Bounty Compliance

✅ **Commodity Hardware:** Standard Intel Xeon (available worldwide)
✅ **≤16 Cores:** Exactly 16 vCPUs used
✅ **Reproducible:** Standard Azure VM anyone can provision
✅ **No Special Hardware:** No GPUs, FPGAs, or custom silicon
✅ **Standard Configuration:** Ubuntu + Rust + REVM (all open-source)

---

**For verification purposes, SupraEVM team can:**
1. Provision identical Azure VM
2. Clone GitHub repository
3. Run benchmarks
4. Verify identical results (±2% tolerance for cloud variance)
