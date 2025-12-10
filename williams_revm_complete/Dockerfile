# Williams Executor - Optimized Docker Image
# Builds a minimal, reproducible image for benchmarking

# Stage 1: Builder  
FROM rustlang/rust:nightly-slim as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy Cargo files first (for layer caching)
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo

# Create dummy src to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    mkdir -p src/bin tests && \
    touch src/lib.rs

# Build dependencies (cached layer)
# Override target-cpu=native with generic x86-64 for Docker compatibility
ENV RUSTFLAGS="-C opt-level=3"
RUN cargo build --release

# Remove dummy src
RUN rm -rf src

# Copy actual source code
COPY src ./src
COPY tests ./tests

# Build Williams with optimizations
# This uses the .cargo/config.toml settings (target-cpu=native, aggressive inlining)
RUN cargo build --release --features quiet

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash williams

# Copy binary from builder
COPY --from=builder /build/target/release/williams-complete /usr/local/bin/

# Set up directories
RUN mkdir -p /data /out && \
    chown -R williams:williams /data /out

# Switch to non-root user
USER williams
WORKDIR /home/williams

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/williams-complete"]

# Default: expect data directory as first arg, core count as second
# Usage: docker run -v $PWD/data:/data williams/executor:latest /data 16
