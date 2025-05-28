# 📱 Storyboard Setup Guide for actionaid2

## 🎯 UI Elements to Add

You need to add these UI elements to your `Main.storyboard`:

### 1. Status Label (UILabel)
- **Purpose**: Shows current status ("Ready", "Running tests", etc.)
- **Position**: Top of the screen
- **Constraints**: 
  - Top: Safe Area + 20
  - Leading: 20, Trailing: -20
  - Height: 30

### 2. Test Button (UIButton)
- **Purpose**: Triggers the test suite
- **Position**: Below status label
- **Text**: "Run Tests"
- **Constraints**:
  - Top: Status Label + 20
  - Leading: 20, Trailing: -20
  - Height: 50

### 3. Results Text View (UITextView)
- **Purpose**: Displays test results
- **Position**: Below button, fills remaining space
- **Constraints**:
  - Top: Test Button + 20
  - Leading: 20, Trailing: -20
  - Bottom: Safe Area - 20

## 🔗 Step-by-Step Connection Guide

### Step 1: Open Storyboard
1. Open `actionaid2.xcodeproj` in Xcode
2. Click on `Main.storyboard` in the navigator
3. You should see a single View Controller scene

### Step 2: Add UI Elements

#### Add Status Label:
1. From Object Library (+ button), drag a **Label** to the top of the view
2. Double-click to edit text: "iPad Rust Core Test"
3. Set constraints:
   - Control-drag from label to Safe Area (top): **20 points**
   - Control-drag from label to view (leading): **20 points**
   - Control-drag from label to view (trailing): **20 points**
   - Set height constraint: **30 points**

#### Add Test Button:
1. Drag a **Button** below the label
2. Double-click to edit text: "Run Tests"
3. Set constraints:
   - Control-drag from button to label (vertical): **20 points**
   - Control-drag from button to view (leading): **20 points**
   - Control-drag from button to view (trailing): **20 points**
   - Set height constraint: **50 points**

#### Add Results Text View:
1. Drag a **Text View** below the button
2. Set constraints:
   - Control-drag from text view to button (vertical): **20 points**
   - Control-drag from text view to view (leading): **20 points**
   - Control-drag from text view to view (trailing): **20 points**
   - Control-drag from text view to Safe Area (bottom): **20 points**

### Step 3: Connect Outlets and Actions

#### Open Assistant Editor:
1. Click the **Assistant Editor** button (two circles) in the toolbar
2. Make sure `ViewController.swift` is shown on the right

#### Connect Status Label:
1. Control-drag from the **Label** to the code
2. Connect to: `@IBOutlet weak var statusLabel: UILabel!`

#### Connect Test Button (Outlet):
1. Control-drag from the **Button** to the code
2. Connect to: `@IBOutlet weak var testButton: UIButton!`

#### Connect Test Button (Action):
1. Control-drag from the **Button** to the code again
2. Connect to: `@IBAction func runTests(_ sender: UIButton)`

#### Connect Results Text View:
1. Control-drag from the **Text View** to the code
2. Connect to: `@IBOutlet weak var resultTextView: UITextView!`

## 🎨 UI Styling (Optional)

### Status Label Styling:
- Font: System Bold, 18pt
- Alignment: Center
- Text Color: Label (default)

### Button Styling:
- Background Color: System Blue
- Text Color: White
- Corner Radius: 8
- Font: System, 16pt

### Text View Styling:
- Font: Monospaced System, 12pt
- Background Color: System Gray 6
- Corner Radius: 8
- Editable: NO
- Scrollable: YES

## 🔧 Build Settings Configuration

After setting up the UI, configure these build settings:

### 1. Bridging Header
- Target → Build Settings → Swift Compiler - General
- **Objective-C Bridging Header**: `actionaid2/actionaid2-Bridging-Header.h`

### 2. Search Paths
- **Library Search Paths**: `$(SRCROOT)/actionaid2/Libraries`
- **Header Search Paths**: `$(SRCROOT)/actionaid2/Libraries`

### 3. Frameworks
- Target → General → Frameworks, Libraries, and Embedded Content
- Add: `SystemConfiguration.framework`
- Add: `Security.framework`

### 4. Other Linker Flags
- Build Settings → Linking → Other Linker Flags
- Add: `-framework SystemConfiguration -framework Security`

## ✅ Testing Checklist

Before running:
- [ ] All UI elements are added to storyboard
- [ ] All outlets are connected (no broken connections)
- [ ] Action is connected to button
- [ ] Bridging header is configured
- [ ] Library search paths are set
- [ ] Required frameworks are added
- [ ] Project builds without errors

## 🚀 Running the App

1. Select a simulator or device target
2. Press **Cmd+R** to build and run
3. The app should launch with your UI
4. Tap "Run Tests" to execute the Rust library tests
5. Results will appear in the text view

## 🐛 Troubleshooting

### "Bridging header not found"
- Check the bridging header path in Build Settings
- Ensure the file exists at the specified location

### "Library not found"
- Verify library files are in the Libraries folder
- Check Library Search Paths in Build Settings

### "Undefined symbols"
- Ensure you're using the correct .a file for your target
- Check that all required frameworks are linked

### UI elements not responding
- Verify all outlets and actions are properly connected
- Check for broken connections in Interface Builder

## 🎉 Success!

Once everything is set up correctly, you'll have a fully functional iOS app that tests your Rust library! The app will:

- Initialize the database in the iOS Documents directory
- Test user creation and authentication
- Test project operations
- Display comprehensive results
- Handle errors gracefully

Your iPad Rust Core is now ready for production use! 🚀 