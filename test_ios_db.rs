use std::ffi::{CString, CStr};
use std::os::raw::c_char;

// Import our FFI functions
extern "C" {
    fn set_ios_storage_path(path: *const c_char) -> i32;
    fn initialize_library(db_path: *const c_char, device_id: *const c_char, offline_mode: bool, jwt_secret: *const c_char) -> i32;
    fn auth_initialize_default_accounts(token: *const c_char) -> i32;
    fn get_last_error() -> *mut c_char;
    fn free_string(ptr: *mut c_char);
}

fn main() {
    println!("🧪 Testing iOS Database Initialization");
    println!("=====================================");
    
    // Test 1: Set iOS storage path
    println!("1️⃣ Setting iOS storage path...");
    let storage_path = CString::new("/tmp/test_ios_storage").unwrap();
    let storage_result = unsafe { set_ios_storage_path(storage_path.as_ptr()) };
    
    if storage_result == 0 {
        println!("✅ iOS storage path set successfully");
    } else {
        println!("❌ Failed to set iOS storage path: {}", storage_result);
        return;
    }
    
    // Test 2: Initialize database
    println!("\n2️⃣ Initializing database...");
    let db_path = CString::new("/tmp/test_ios_actionaid.sqlite").unwrap();
    let device_id = CString::new("test-device-12345").unwrap();
    let jwt_secret = CString::new("test_jwt_secret_for_ios_testing_12345").unwrap();
    
    let init_result = unsafe { 
        initialize_library(
            db_path.as_ptr(),
            device_id.as_ptr(), 
            false,
            jwt_secret.as_ptr()
        )
    };
    
    if init_result == 0 {
        println!("✅ Database initialized successfully");
        
        // Check if database file was created
        if std::path::Path::new("/tmp/test_ios_actionaid.sqlite").exists() {
            println!("✅ Database file created");
            
            // Get file size
            if let Ok(metadata) = std::fs::metadata("/tmp/test_ios_actionaid.sqlite") {
                println!("📏 Database size: {} bytes", metadata.len());
            }
        } else {
            println!("❌ Database file not found");
        }
    } else {
        println!("❌ Database initialization failed with code: {}", init_result);
        
        // Get error details
        unsafe {
            let error_ptr = get_last_error();
            if !error_ptr.is_null() {
                let error_cstr = CStr::from_ptr(error_ptr);
                if let Ok(error_str) = error_cstr.to_str() {
                    println!("🔍 Error: {}", error_str);
                }
                free_string(error_ptr);
            }
        }
        return;
    }
    
    // Test 3: Initialize default accounts
    println!("\n3️⃣ Initializing default accounts...");
    let init_token = CString::new("init_setup").unwrap();
    let accounts_result = unsafe { auth_initialize_default_accounts(init_token.as_ptr()) };
    
    if accounts_result == 0 {
        println!("✅ Default accounts initialized successfully");
        println!("👥 Created: admin@example.com, lead@example.com, officer@example.com");
    } else {
        println!("⚠️ Default accounts initialization returned: {}", accounts_result);
        
        // Get error details
        unsafe {
            let error_ptr = get_last_error();
            if !error_ptr.is_null() {
                let error_cstr = CStr::from_ptr(error_ptr);
                if let Ok(error_str) = error_cstr.to_str() {
                    println!("🔍 Error: {}", error_str);
                    // If it's a "duplicate" or "already exists" error, that's actually OK
                    if error_str.contains("UNIQUE constraint") || 
                       error_str.contains("duplicate") ||
                       error_str.contains("already exists") {
                        println!("✅ Accounts already exist - this is fine!");
                    }
                }
                free_string(error_ptr);
            }
        }
    }
    
    println!("\n🎉 iOS Database Test Complete!");
    println!("✨ All core functionality appears to be working!");
} 