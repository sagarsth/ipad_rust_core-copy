#!/usr/bin/env python3
"""
Script to fix Tokio runtime conflicts in FFI files.
This script will:
1. Remove Runtime imports
2. Replace local block_on_async functions with calls to the centralized one
"""

import os
import re
import glob

def fix_ffi_file(file_path):
    """Fix a single FFI file by removing runtime creation and using centralized runtime."""
    print(f"Fixing {file_path}...")
    
    with open(file_path, 'r') as f:
        content = f.read()
    
    original_content = content
    
    # Remove Runtime import
    content = re.sub(r'use tokio::runtime::Runtime;\n', '', content)
    
    # Replace the block_on_async function definition
    old_pattern = r'''/// Helper function to run async code in a blocking context
fn block_on_async<F, T, E>\(future: F\) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
\{
    let rt = Runtime::new\(\)\.expect\(".*?"\);
    rt\.block_on\(future\)
\}'''
    
    new_replacement = '''/// Helper function to run async code in a blocking context
fn block_on_async<F, T, E>(future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    crate::ffi::block_on_async(future)
}'''
    
    content = re.sub(old_pattern, new_replacement, content, flags=re.MULTILINE | re.DOTALL)
    
    # Alternative pattern for different variations
    alt_pattern = r'''fn block_on_async<F, T, E>\(future: F\) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
\{
    let rt = Runtime::new\(\)\.expect\(".*?"\);
    rt\.block_on\(future\)
\}'''
    
    content = re.sub(alt_pattern, '''fn block_on_async<F, T, E>(future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    crate::ffi::block_on_async(future)
}''', content, flags=re.MULTILINE | re.DOTALL)
    
    # Handle compression.rs special cases with direct Runtime::new() calls
    content = re.sub(r'let rt = Runtime::new\(\)\s*\.map_err\([^}]+\}\)?;', 
                     '// Using centralized runtime', content)
    content = re.sub(r'rt\.block_on\(', 'crate::ffi::block_on_async(', content)
    
    # Write back if changed
    if content != original_content:
        with open(file_path, 'w') as f:
            f.write(content)
        print(f"  ✅ Fixed {file_path}")
        return True
    else:
        print(f"  ⏭️  No changes needed for {file_path}")
        return False

def main():
    """Main function to fix all FFI files."""
    print("🔧 Fixing Tokio runtime conflicts in FFI files...")
    
    # Get all FFI files except mod.rs and core.rs (already fixed)
    ffi_files = glob.glob("src/ffi/*.rs")
    ffi_files = [f for f in ffi_files if not f.endswith(('mod.rs', 'core.rs', 'error.rs'))]
    
    fixed_count = 0
    for file_path in ffi_files:
        if fix_ffi_file(file_path):
            fixed_count += 1
    
    print(f"\n🎉 Fixed {fixed_count} out of {len(ffi_files)} FFI files")
    print("✅ All runtime conflicts should now be resolved!")

if __name__ == "__main__":
    main() 