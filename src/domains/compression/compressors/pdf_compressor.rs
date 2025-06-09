//! PDF compression using lopdf (pure Rust, iOS-compatible)

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::task;
use lopdf::{Document, Object, ObjectId};

use crate::errors::{DomainError, DomainResult};
use super::Compressor;
use crate::domains::compression::types::CompressionMethod;

/// PDF compressor using lopdf for iOS-compatible compression
#[derive(Clone)]
pub struct PdfCompressor {
    // Remove ghostscript_path since we're using pure Rust
}

impl PdfCompressor {
    pub fn new(_ghostscript_path: Option<String>) -> Self {
        // Ignore ghostscript path, use lopdf instead
        Self {}
    }
}

#[async_trait]
impl Compressor for PdfCompressor {
    async fn can_handle(&self, mime_type: &str, extension: Option<&str>) -> bool {
        mime_type == "application/pdf" || extension == Some("pdf")
    }
    
    async fn compress(
        &self,
        data: Vec<u8>,
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        println!("📄 [PDF_COMPRESSOR] Starting PDF compression: {} bytes, method: {:?}, quality: {}", 
                 data.len(), method, quality_level);
        
        // Run PDF operations in blocking task
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            compress_pdf_with_lopdf(data, quality_level)
        }).await.map_err(|e| {
            println!("❌ [PDF_COMPRESSOR] Task join error: {}", e);
            DomainError::Internal(format!("Task join error: {}", e))
        })?
    }
}

/// Compress PDF using lopdf library
fn compress_pdf_with_lopdf(data: Vec<u8>, quality_level: i32) -> DomainResult<Vec<u8>> {
    use lopdf::{Document, Object, ObjectId};
    
    println!("📄 [PDF_COMPRESSOR] Loading PDF document...");
    
    // Load the PDF document
    let mut doc = Document::load_mem(&data)
        .map_err(|e| {
            println!("❌ [PDF_COMPRESSOR] Failed to load PDF: {}", e);
            DomainError::Internal(format!("Failed to load PDF: {}", e))
        })?;
    
    println!("📄 [PDF_COMPRESSOR] PDF loaded successfully");
    
    // Apply compression optimizations based on quality level
    match quality_level {
        0..=3 => {
            // Aggressive compression
            println!("📄 [PDF_COMPRESSOR] Applying aggressive compression...");
            doc.compress();
            doc.prune_objects();
            doc.delete_zero_length_streams();
            remove_unnecessary_objects(&mut doc);
            optimize_images_in_pdf(&mut doc, 60)?;
        },
        4..=6 => {
            // Balanced compression
            println!("📄 [PDF_COMPRESSOR] Applying balanced compression...");
            doc.compress();
            doc.prune_objects();
            optimize_images_in_pdf(&mut doc, 75)?;
        },
        7..=10 => {
            // Light compression
            println!("📄 [PDF_COMPRESSOR] Applying light compression...");
            doc.compress();
            doc.prune_objects();
        },
        _ => {
            // Minimal compression
            println!("📄 [PDF_COMPRESSOR] Applying minimal compression...");
            doc.compress();
        }
    }
    
    // Save the optimized document
    let mut output = Vec::new();
    doc.save_to(&mut output)
        .map_err(|e| {
            println!("❌ [PDF_COMPRESSOR] Failed to save PDF: {}", e);
            DomainError::Internal(format!("Failed to save PDF: {}", e))
        })?;
    
    println!("✅ [PDF_COMPRESSOR] PDF compression successful: {} bytes -> {} bytes", 
             data.len(), output.len());
    
    Ok(output)
}

/// Remove unnecessary objects from PDF
fn remove_unnecessary_objects(doc: &mut Document) {
    // Remove metadata that might not be essential
    let objects_to_remove: Vec<ObjectId> = doc.objects.iter()
        .filter_map(|(id, obj)| {
            match obj {
                Object::Dictionary(dict) => {
                    // Remove certain types of metadata
                    if dict.has(b"Type") {
                        if let Ok(Object::Name(ref name)) = dict.get(b"Type") {
                            match name.as_slice() {
                                b"Metadata" | b"StructTreeRoot" => Some(*id),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                _ => None,
            }
        })
        .collect();
    
    for id in objects_to_remove {
        doc.objects.remove(&id);
    }
}

/// Optimize images within the PDF (placeholder implementation)
fn optimize_images_in_pdf(doc: &mut Document, _quality: u8) -> DomainResult<()> {
    // This is a simplified implementation
    // In a full implementation, you would:
    // 1. Find all image objects in the PDF
    // 2. Extract image data
    // 3. Recompress using image compression
    // 4. Replace the image data in the PDF
    
    // For now, we'll just ensure streams are compressed
    for (_id, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            if !stream.dict.has(b"Filter") {
                // Add compression filter if not present
                stream.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                stream.compress();
            }
        }
    }
    
    Ok(())
}