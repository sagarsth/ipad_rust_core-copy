# 📱 Xcode Project Structure Guide

## 🎯 Complete File Structure for Xcode

Here's exactly what your Xcode project should look like:

```
YourXcodeProject/                    ← Root project folder
├── YourXcodeProject.xcodeproj/      ← Xcode project file (double-click to open)
│   └── project.pbxproj              ← Project configuration
├── YourXcodeProject/                ← Source code folder (same name as project)
│   ├── AppDelegate.swift
│   ├── SceneDelegate.swift
│   ├── ViewController.swift          ← Replace with iOS test code
│   ├── Main.storyboard              ← Add UI elements here
│   ├── Assets.xcassets/
│   ├── LaunchScreen.storyboard
│   ├── Info.plist
│   │
│   ├── Libraries/                   ← Create this folder
│   │   ├── libipad_rust_core.a     ← Copy from target/ios/
│   │   └── ipad_rust_core.h        ← Copy from target/ios/
│   │
│   └── iPad-Rust-Core-Bridging-Header.h  ← Create this file
```

## 🤔 Why Two Folders with Same Name?

This is **standard Xcode convention**:

1. **Outer folder** (`YourXcodeProject/`): 
   - Root project directory
   - Contains the `.xcodeproj` file and source folder
   - This is what you see in Finder

2. **Inner folder** (`YourXcodeProject/`):
   - Contains all your source code files
   - Same name as the project (Xcode creates this automatically)
   - This is what you see inside Xcode navigator

## 📁 Real Example:
If you create a project called "iPadRustCoreTest", you'll get:

```
iPadRustCoreTest/                    ← Root folder
├── iPadRustCoreTest.xcodeproj/      ← Project file
└── iPadRustCoreTest/                ← Source folder
    ├── AppDelegate.swift
    ├── ViewController.swift
    └── ... other files
```

## 🚀 Step-by-Step Setup Instructions

### Step 1: Create New Xcode Project
1. Open Xcode
2. File → New → Project
3. Choose **iOS** → **App**
4. Fill in details:
   - Product Name: `iPadRustCoreTest` (or your choice)
   - Language: **Swift**
   - Interface: **Storyboard**
   - Minimum iOS: **13.0**

### Step 2: Add Required Files

#### A. Create Libraries Folder
1. Right-click on your project in Xcode
2. New Group → Name it "Libraries"
3. Copy these files into the Libraries folder:

```bash
# From your terminal, copy these files:
cp "target/ios/libipad_rust_core_device.a" ~/Desktop/
cp "target/ios/libipad_rust_core_sim.a" ~/Desktop/
cp "target/ios/ipad_rust_core.h" ~/Desktop/
```

#### B. Add Static Libraries to Xcode
1. Drag `libipad_rust_core_device.a` into Libraries folder
2. Drag `libipad_rust_core_sim.a` into Libraries folder
3. Drag `ipad_rust_core.h` into Libraries folder
4. When prompted: ✅ "Add to target"

#### C. Create Bridging Header
1. File → New → File → Header File
2. Name: `iPad-Rust-Core-Bridging-Header.h`
3. Add this content:

```c
#ifndef iPad_Rust_Core_Bridging_Header_h
#define iPad_Rust_Core_Bridging_Header_h

#import "ipad_rust_core.h"

#endif
```

### Step 3: Configure Build Settings

#### A. Set Bridging Header
1. Select your project → Build Settings
2. Search for "bridging"
3. Set **Objective-C Bridging Header** to:
   ```
   $(SRCROOT)/YourProjectName/iPad-Rust-Core-Bridging-Header.h
   ```

#### B. Configure Library Search Paths
1. Build Settings → Search for "Library Search Paths"
2. Add: `$(SRCROOT)/YourProjectName/Libraries`

#### C. Configure Header Search Paths
1. Build Settings → Search for "Header Search Paths"  
2. Add: `$(SRCROOT)/YourProjectName/Libraries`

#### D. Link Required Frameworks
1. Project → General → Frameworks, Libraries, and Embedded Content
2. Click **+** and add:
   - `SystemConfiguration.framework`
   - `Security.framework`

### Step 4: Add UI Elements to Storyboard

Open `Main.storyboard` and add:

1. **UILabel** (for status)
   - Connect to `@IBOutlet weak var statusLabel: UILabel!`

2. **UIButton** (for testing)
   - Title: "Run Tests"
   - Connect to `@IBAction func runTests(_ sender: UIButton)`

3. **UITextView** (for results)
   - Connect to `@IBOutlet weak var resultTextView: UITextView!`

### Step 5: Replace ViewController Code

Replace the content of `ViewController.swift` with the test code from:
```
iOS_Test_ViewController.swift
```

## 🔧 Build Configuration for Different Targets

### For iOS Device (Physical iPhone/iPad)
- Uses: `libipad_rust_core_device.a`
- Architecture: `arm64`

### For iOS Simulator
- Uses: `libipad_rust_core_sim.a` 
- Architecture: `x86_64` + `arm64` (universal)

### Xcode will automatically choose the right library based on your build target!

## 🎯 Final Project Structure in Xcode Navigator

```
📁 YourXcodeProject
├── 📁 YourXcodeProject
│   ├── 📄 AppDelegate.swift
│   ├── 📄 SceneDelegate.swift
│   ├── 📄 ViewController.swift (with test code)
│   ├── 📄 Main.storyboard (with UI elements)
│   ├── 📄 iPad-Rust-Core-Bridging-Header.h
│   ├── 📁 Libraries
│   │   ├── 📄 libipad_rust_core_device.a
│   │   ├── 📄 libipad_rust_core_sim.a
│   │   └── 📄 ipad_rust_core.h
│   ├── 📁 Assets.xcassets
│   ├── 📄 LaunchScreen.storyboard
│   └── 📄 Info.plist
└── 📁 Products
    └── 📄 YourXcodeProject.app
```

## ✅ Testing Checklist

- [ ] Project builds without errors
- [ ] Can run on iOS Simulator
- [ ] Can run on physical device
- [ ] Button triggers Rust functions
- [ ] Results display in text view
- [ ] No runtime crashes

## 🚨 Common Issues & Solutions

### "Library not found"
- Check Library Search Paths in Build Settings
- Ensure .a files are added to target

### "Header not found"
- Check Header Search Paths in Build Settings
- Verify bridging header path is correct

### "Undefined symbols"
- Ensure you're using the right .a file for your target
- Check that all required frameworks are linked

### "Module not found"
- Clean build folder (Cmd+Shift+K)
- Rebuild project

## 🎉 Success!

Once everything is set up correctly, you should be able to:
1. Build and run your app
2. Tap "Run Tests" button
3. See Rust library functions executing
4. View results in the text view

Your iPad Rust Core is now fully integrated with Xcode! 🚀 