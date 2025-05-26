#!/usr/bin/env python3
"""
Generate complete C header file from Rust FFI functions
This script scans all FFI files and extracts function signatures
"""

import os
import re
import glob
from typing import List, Tuple

def extract_ffi_functions(file_path: str) -> List[Tuple[str, str, str]]:
    """Extract FFI function signatures from a Rust file"""
    functions = []
    
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Pattern to match FFI function declarations, making return type optional
    # Group 1: func_name, Group 2: params, Group 3: return_type (optional)
    pattern = r'#\[unsafe\(no_mangle\)\]\s*pub unsafe extern "C" fn\s+(\w+)\s*\((.*?)\)(?:\s*->\s*([^{}\n]+))?'
    
    matches = re.finditer(pattern, content, re.MULTILINE | re.DOTALL)
    
    for match in matches:
        func_name = match.group(1)
        params_str = match.group(2).strip() # Renamed to avoid conflict
        return_type_match = match.group(3) # This can be None

        # ---- START DEBUG PRINT ----
        if func_name == "export_projects_all":
            print(f"DEBUG: func_name: {func_name}")
            print(f"DEBUG: params_str: {params_str}")
            print(f"DEBUG: return_type_match (group 3): {return_type_match}")
        # ---- END DEBUG PRINT ----
        
        c_params = convert_params_to_c(params_str)
        c_return = convert_return_to_c(return_type_match)
        
        functions.append((func_name, c_params, c_return))
    
    return functions

def convert_params_to_c(params: str) -> str:
    """Convert Rust parameter types to C types"""
    if not params.strip():
        return "void"
    
    # Remove parameter names, keep only types
    param_parts = []
    for param in params.split(','):
        param = param.strip()
        if not param:  # Skip empty parameters
            continue
            
        if ':' in param:
            # Extract type after colon
            param_type = param.split(':')[-1].strip()
        else:
            param_type = param
        
        # Convert Rust types to C types
        param_type = re.sub(r'\*const c_char', 'const char*', param_type)
        param_type = re.sub(r'\*mut \*mut c_char', 'char**', param_type)
        param_type = re.sub(r'\*mut c_char', 'char*', param_type)
        param_type = re.sub(r'c_int', 'int32_t', param_type)
        param_type = re.sub(r'bool', 'bool', param_type)
        
        if param_type:  # Only add non-empty types
            param_parts.append(param_type)
    
    return ', '.join(param_parts) if param_parts else "void"

def convert_return_to_c(return_type_match: str | None) -> str:
    """Convert Rust return type to C type"""
    if return_type_match is None: # No "->" part, so it's a void Rust function
        return 'void'
        
    return_type_str = return_type_match.strip()

    # For debugging:
    # print(f"DEBUG: Raw return_type_match: '{return_type_match}', Stripped: '{return_type_str}'")

    if return_type_str == 'c_int':
        return 'int32_t'
    elif return_type_str == '()': # Explicit "-> ()"
        return 'void'
    elif return_type_str == 'bool':
        return 'bool'
    elif return_type_str == '*mut c_char' or return_type_str == '*const c_char':
        return 'char*'
    elif not return_type_str: # Empty match after "->"
        return 'void'
    else:
        print(f"Warning: Unrecognized Rust return type '{return_type_str}' (from original '{return_type_match}'), defaulting to 'void' in C header.")
        return 'void'

def generate_header():
    """Generate the complete header file"""
    
    header_content = '''#ifndef IPAD_RUST_CORE_H
#define IPAD_RUST_CORE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// AUTO-GENERATED C HEADER FOR IPAD RUST CORE FFI
// This file was generated automatically from Rust FFI functions
// DO NOT EDIT MANUALLY - regenerate using scripts/generate_header.py
// ============================================================================

'''

    # Get all FFI files
    ffi_files = glob.glob('src/ffi/*.rs')
    ffi_files.sort()
    
    total_functions = 0
    
    for ffi_file in ffi_files:
        if ffi_file.endswith('/mod.rs') or ffi_file.endswith('/error.rs'):
            continue
            
        module_name = os.path.basename(ffi_file).replace('.rs', '')
        functions = extract_ffi_functions(ffi_file)
        
        if functions:
            header_content += f'''
// ============================================================================
// {module_name.upper()} FUNCTIONS ({len(functions)} functions)
// ============================================================================

'''
            
            for func_name, params, return_type in functions:
                header_content += f'{return_type} {func_name}({params});\n'
            
            total_functions += len(functions)
    
    header_content += '''
#ifdef __cplusplus
}
#endif

#endif // IPAD_RUST_CORE_H
'''

    # Write the header file
    with open('include/ipad_rust_core_complete.h', 'w') as f:
        f.write(header_content)
    
    print(f"✅ Generated complete header with {total_functions} functions")
    print(f"📄 Saved to: include/ipad_rust_core_complete.h")
    
    return total_functions

if __name__ == "__main__":
    generate_header() 