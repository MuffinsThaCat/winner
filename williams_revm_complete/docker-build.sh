#!/bin/bash
# Williams Executor - Docker Build Script
set -e

echo "🐳 Building Williams Executor Docker Image..."
echo ""

# Get version from Cargo.toml
VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
echo "📦 Version: $VERSION"

# Build multi-platform image (optional, comment out if not needed)
# docker buildx build --platform linux/amd64,linux/arm64 \
#   -t williams/executor:$VERSION \
#   -t williams/executor:latest \
#   --push .

# Build for current platform only
docker build \
  -t williams/executor:$VERSION \
  -t williams/executor:latest \
  .

echo ""
echo "✅ Build complete!"
echo ""
echo "🚀 Test with:"
echo "   docker run --rm williams/executor:latest --help"
echo ""
echo "📊 Run benchmark:"
echo "   docker run --rm -v \$PWD/data:/data williams/executor:latest /data 16"
echo ""
