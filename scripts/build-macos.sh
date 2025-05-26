#!/bin/bash

# Build script for macOS static library
set -e

# Set macOS deployment target to match Package.swift
export MACOSX_DEPLOYMENT_TARGET="14.0"

echo "🚀 Building iPad Rust Core for macOS..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo is not installed. Please install Rust first.${NC}"
    exit 1
fi

# Check if macOS targets are installed
echo -e "${YELLOW}🖥️  Checking macOS targets...${NC}"
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

echo -e "${YELLOW}🔧 Generating complete header file...${NC}"
python3 scripts/generate_header.py
cp include/ipad_rust_core_complete.h include/ipad_rust_core.h

# Create output directory
mkdir -p target/macos

# Build for macOS Intel (x86_64)
echo -e "${YELLOW}🔨 Building for macOS Intel (x86_64-apple-darwin)...${NC}"
cargo build --release --target x86_64-apple-darwin

# Build for macOS Apple Silicon (ARM64)
echo -e "${YELLOW}🔨 Building for macOS Apple Silicon (aarch64-apple-darwin)...${NC}"
cargo build --release --target aarch64-apple-darwin

# Create universal library
echo -e "${YELLOW}🔗 Creating universal macOS library...${NC}"
lipo -create \
    target/x86_64-apple-darwin/release/libipad_rust_core.a \
    target/aarch64-apple-darwin/release/libipad_rust_core.a \
    -output target/macos/libipad_rust_core.a

# Copy header file
cp include/ipad_rust_core.h target/macos/

echo -e "${GREEN}✅ macOS build complete!${NC}"
echo -e "${GREEN}📁 Output files:${NC}"
echo -e "   • target/macos/libipad_rust_core.a (Universal macOS)"
echo -e "   • target/macos/ipad_rust_core.h (C header)"

echo -e "${YELLOW}💡 Next steps:${NC}"
echo -e "   1. Add the .a file to your macOS project"
echo -e "   2. Add the header file to your bridging header"
echo -e "   3. Link against required system frameworks" 