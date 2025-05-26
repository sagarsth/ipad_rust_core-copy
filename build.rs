use std::env;
use std::path::PathBuf;

fn main() {
    // Get the target directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(out_dir);
    let target_dir = out_path.parent().unwrap().parent().unwrap().parent().unwrap();
    
    // Print cargo instructions for linking
    println!("cargo:rustc-link-lib=static=sqlite3");
    
    // For iOS/macOS targets, link against system frameworks
    let target = env::var("TARGET").unwrap();
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Security");
        if target.contains("ios") {
            println!("cargo:rustc-link-lib=framework=UIKit");
        } else if target.contains("darwin") {
            println!("cargo:rustc-link-lib=framework=AppKit");
        }
    }
    
    // Copy header file to target directory
    let header_src = "include/ipad_rust_core.h";
    let header_dst = target_dir.join("ipad_rust_core.h");
    
    if std::path::Path::new(header_src).exists() {
        if let Err(e) = std::fs::copy(header_src, &header_dst) {
            eprintln!("Warning: Failed to copy header file: {}", e);
        }
        println!("cargo:rerun-if-changed={}", header_src);
    }
    
    // Tell cargo to rerun if any source files change
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=migrations/");
} 