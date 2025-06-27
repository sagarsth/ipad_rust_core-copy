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
# Get configuration from environment or default to Debug
CONFIGURATION=${BUILD_CONFIGURATION:-Debug}
echo "Building with configuration: $CONFIGURATION"

# Set appropriate logging level based on configuration
if [ "$CONFIGURATION" = "Release" ]; then
    echo "Setting minimal logging for Release build..."
    export RUST_LOG=error
    export IPAD_RUST_VERBOSE=false
else
    echo "Setting detailed logging for Debug build..."
    export RUST_LOG=debug
    export IPAD_RUST_VERBOSE=true
fi

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

if [ "$CONFIGURATION" = "Release" ]; then
    # For App Store distribution - build for all iOS devices
    echo "Building for App Store distribution..."
    xcodebuild -project ActionAidSwiftUI.xcodeproj \
        -scheme ActionAidSwiftUI \
        -destination "generic/platform=iOS" \
        -configuration Release \
        clean build
else
    # For development - build for both simulator and device
    echo "Building for development and testing..."
    xcodebuild -project ActionAidSwiftUI.xcodeproj \
        -scheme ActionAidSwiftUI \
        -destination "generic/platform=iOS Simulator" \
        -destination "generic/platform=iOS" \
        -configuration Debug \
        clean build
fi
cd ..
print_success "Swift project built"

print_step "Launching app on simulator..."
cd ActionAidSwiftUI
xcrun simctl launch booted sagar.ActionAidSwiftUI
cd ..
print_success "App launched"

# Only do database checking for Debug builds (development)
if [ "$CONFIGURATION" = "Debug" ]; then
    print_step "Waiting for app to initialize..."
    sleep 5

    # Dynamically get the currently booted simulator's device ID
    DEVICE_ID=$(xcrun simctl list devices | grep "(Booted)" | head -1 | sed -E 's/.*\(([^)]+)\).*/\1/')

    if [ -z "$DEVICE_ID" ]; then
        print_error "No booted simulator found. Cannot check database files."
    else
        print_step "Checking for database files on device: $DEVICE_ID"
        
        # First, find the most recent app container
        APP_DATA_DIR="$HOME/Library/Developer/CoreSimulator/Devices/$DEVICE_ID/data/Containers/Data/Application"
        
        if [ -d "$APP_DATA_DIR" ]; then
            LATEST_APP=$(find "$APP_DATA_DIR" -name "*" -type d -maxdepth 1 -print0 | xargs -0 ls -dt | head -1 2>/dev/null)

            if [ -n "$LATEST_APP" ]; then
                print_success "Found app directory: $LATEST_APP"
                
                echo "Checking for database files..."
                find "$LATEST_APP" -name "*.sqlite" -o -name "*.db" 2>/dev/null || echo "No database files found yet"
                
                echo -e "\nChecking ActionAid directory:"
                ls -la "$LATEST_APP/Documents/ActionAid/" 2>/dev/null || echo "ActionAid directory not found yet"
                
                echo -e "\nChecking storage directory:"
                ls -la "$LATEST_APP/Documents/ActionAid/storage/" 2>/dev/null || echo "Storage directory not found yet"
            else
                print_error "Could not find app directory in simulator"
            fi
        else
            print_error "Simulator app data directory not found"
        fi
    fi
else
    print_step "Release build - skipping database checks"
fi

# Calculate and display elapsed time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
echo -e "\n${GREEN}Total build time: $ELAPSED seconds${NC}"