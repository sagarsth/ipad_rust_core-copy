#!/usr/bin/env python3

import re

def fix_compression_runtime():
    file_path = "src/ffi/compression.rs"
    
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Pattern to match the runtime creation and removal
    pattern = r'let rt = Runtime::new\(\)\s*\.map_err\([^}]+\}\)\?;\s*\n\s*'
    
    # Replace with empty string (remove the runtime creation)
    fixed_content = re.sub(pattern, '', content)
    
    with open(file_path, 'w') as f:
        f.write(fixed_content)
    
    print(f"✅ Fixed all Runtime::new() instances in {file_path}")

if __name__ == "__main__":
    fix_compression_runtime() 