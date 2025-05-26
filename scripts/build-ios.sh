#!/bin/bash

# Build script for iOS static library
set -e

echo "🚀 Building iPad Rust Core for iOS..."

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

# Check if iOS targets are installed
echo -e "${YELLOW}📱 Checking iOS targets...${NC}"
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-ios-sim

echo -e "${YELLOW}🔧 Generating complete header file...${NC}"
python3 scripts/generate_header.py
cp include/ipad_rust_core_complete.h include/ipad_rust_core.h

# Create output directory
mkdir -p target/ios

# Build for iOS device (ARM64)
echo -e "${YELLOW}🔨 Building for iOS device (aarch64-apple-ios)...${NC}"
cargo build --release --target aarch64-apple-ios

# Build for iOS simulator (x86_64)
echo -e "${YELLOW}🔨 Building for iOS simulator (x86_64-apple-ios)...${NC}"
cargo build --release --target x86_64-apple-ios

# Build for iOS simulator (ARM64 - M1 Macs)
echo -e "${YELLOW}🔨 Building for iOS simulator ARM64 (aarch64-apple-ios-sim)...${NC}"
cargo build --release --target aarch64-apple-ios-sim

# Create universal library for simulator
echo -e "${YELLOW}🔗 Creating universal simulator library...${NC}"
lipo -create \
    target/x86_64-apple-ios/release/libipad_rust_core.a \
    target/aarch64-apple-ios-sim/release/libipad_rust_core.a \
    -output target/ios/libipad_rust_core_sim.a

# Copy device library
cp target/aarch64-apple-ios/release/libipad_rust_core.a target/ios/libipad_rust_core_device.a

# Copy header file
cp include/ipad_rust_core.h target/ios/

echo -e "${GREEN}✅ iOS build complete!${NC}"
echo -e "${GREEN}📁 Output files:${NC}"
echo -e "   • target/ios/libipad_rust_core_device.a (iOS device)"
echo -e "   • target/ios/libipad_rust_core_sim.a (iOS simulator)"
echo -e "   • target/ios/ipad_rust_core.h (C header)"

echo -e "${YELLOW}💡 Next steps:${NC}"
echo -e "   1. Add both .a files to your Xcode project"
echo -e "   2. Add the header file to your bridging header"
echo -e "   3. Configure build settings for each target" 