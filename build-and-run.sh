#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}===> $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Exit on any error
set -e

# Start timer
START_TIME=$(date +%s)

print_step "Building Rust library..."
cargo build --release --lib
print_success "Rust build complete"

print_step "Building iOS libraries..."
./scripts/build-ios.sh
print_success "iOS libraries built"

print_step "Copying libraries to Swift project..."
cd ActionAidSwiftUI
cp ../target/ios/libipad_rust_core_sim.a SwiftUI_ActionAid/libipad_rust_core_sim.a
cp ../target/ios/libipad_rust_core_device.a SwiftUI_ActionAid/libipad_rust_core_device.a
cp ../target/ios/ipad_rust_core.h SwiftUI_ActionAid/ipad_rust_core.h
cd ..
print_success "Libraries copied"

print_step "Building Swift project..."
cd ActionAidSwiftUI
xcodebuild -project ActionAidSwiftUI.xcodeproj \
    -scheme ActionAidSwiftUI \
    -destination "platform=iOS Simulator,name=iPad (10th generation)" \
    build
cd ..
print_success "Swift project built"

print_step "Launching app on simulator..."
cd ActionAidSwiftUI
xcrun simctl launch booted sagar.ActionAidSwiftUI
cd ..
print_success "App launched"

print_step "Waiting for app to initialize..."
sleep 5

# Device ID - you might want to make this dynamic
DEVICE_ID="56B5FF5E-B35C-4C6F-B99F-661B7DAEF552"

print_step "Checking for database files..."
# First, find the most recent app container
LATEST_APP=$(find "/Users/sagarshrestha/Library/Developer/CoreSimulator/Devices/$DEVICE_ID/data/Containers/Data/Application" -name "*" -type d -maxdepth 1 -print0 | xargs -0 ls -dt | head -1)

if [ -n "$LATEST_APP" ]; then
    print_success "Found app directory: $LATEST_APP"
    
    echo "Checking for database files..."
    find "$LATEST_APP" -name "*.sqlite" -o -name "*.db" 2>/dev/null || true
    
    echo -e "\nChecking Library contents:"
    ls -la "$LATEST_APP/Library/" 2>/dev/null || true
    
    echo -e "\nChecking Database directory:"
    ls -la "$LATEST_APP/Library/Database/" 2>/dev/null || true
else
    print_error "Could not find app directory"
fi

# Calculate and display elapsed time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
echo -e "\n${GREEN}Total build time: $ELAPSED seconds${NC}"