 //! Image compression implementation

use async_trait::async_trait;
use image::{ImageFormat, GenericImageView, DynamicImage, ImageEncoder, ColorType};
use tokio::task;
use std::io::Cursor;

use crate::errors::{DomainError, DomainResult};
use super::Compressor;
use crate::domains::compression::types::CompressionMethod;

/// Image compressor using the `image` crate for lossy/lossless compression
#[derive(Clone)]
pub struct ImageCompressor;

#[async_trait]
impl Compressor for ImageCompressor {
    async fn can_handle(&self, mime_type: &str, extension: Option<&str>) -> bool {
        matches!(mime_type, 
            "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/tiff"
        ) || matches!(extension, 
            Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("webp") | Some("tif") | Some("tiff")
        )
    }
    
    async fn compress(
        &self,
        data: Vec<u8>,
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        // Force quality_level into valid range
        let quality = quality_level.clamp(1, 100) as u8;
        
        println!("🖼️ [IMAGE_COMPRESSOR] Starting image compression: {} bytes, method: {:?}, quality: {}", 
                 data.len(), method, quality);
        
        // Store length before moving data
        let original_len = data.len();
        
        // Skip EXIF and color space processing for better performance
        // Process the image directly for faster compression
        let compressed_data = task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            let img = image::load_from_memory(&data)
                .map_err(|e| DomainError::Internal(format!("Failed to load image: {}", e)))?;
            
            // Determine format and compression method
            let format = ImageFormat::Jpeg; // Always output as JPEG for better compression
            
            match method {
                CompressionMethod::Lossy => {
                    compress_lossy_improved(img, format, quality)
                },
                CompressionMethod::Lossless => {
                    // For lossless, use PNG with high compression
                    compress_lossless_improved(img, ImageFormat::Png, quality.clamp(1, 9))
                },
                _ => {
                    // No compression, just re-encode
                    let mut output = Vec::new();
                    img.write_to(&mut Cursor::new(&mut output), format)
                        .map_err(|e| DomainError::Internal(format!("Failed to encode image: {}", e)))?;
                    Ok(output)
                }
            }
        }).await.map_err(|e| DomainError::Internal(format!("Compression task failed: {}", e)))??;
        
        println!("✅ [IMAGE_COMPRESSOR] Compression completed: {} -> {} bytes", 
                 original_len, compressed_data.len());
        
        Ok(compressed_data)
    }
}

fn compress_lossy_improved(img: DynamicImage, format: ImageFormat, quality: u8) -> DomainResult<Vec<u8>> {
    let mut output = Vec::new();
    
    match format {
        ImageFormat::Jpeg => {
            // For JPEG, use the specified quality
            let quality = quality.clamp(60, 95); // Better range for JPEG
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
            encoder.encode_image(&img)
                .map_err(|e| DomainError::Internal(format!("JPEG encoding error: {}", e)))?;
        },
        ImageFormat::Png => {
            // For PNG, convert to JPEG with lossy compression since PNG is already lossless
            let quality = quality.clamp(75, 90); // Higher quality for PNG->JPEG conversion
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
            encoder.encode_image(&img)
                .map_err(|e| DomainError::Internal(format!("PNG->JPEG encoding error: {}", e)))?;
        },
        ImageFormat::WebP => {
            #[cfg(feature = "webp")]
            {
                // Use lossy WebP compression
                let quality = (quality as f32) / 100.0;
                webp::Encoder::from_image(&img)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?
                    .encode_lossy(quality)
                    .write_to(&mut output)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?;
            }
            #[cfg(not(feature = "webp"))]
            {
                // Fall back to JPEG
                let quality = quality.clamp(75, 90);
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
                encoder.encode_image(&img)
                    .map_err(|e| DomainError::Internal(format!("WebP->JPEG fallback error: {}", e)))?;
            }
        },
        _ => {
            // For other formats, convert to JPEG with the specified quality
            let quality = quality.clamp(70, 85);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
            encoder.encode_image(&img)
                .map_err(|e| DomainError::Internal(format!("JPEG encoding error: {}", e)))?;
        }
    }
    
    Ok(output)
}

fn compress_lossless_improved(img: DynamicImage, format: ImageFormat, quality: u8) -> DomainResult<Vec<u8>> {
    let mut output = Vec::new();
    
    match format {
        ImageFormat::Png => {
            // For PNG, use best compression with optimized color type
            let color_type = match img.color() {
                ColorType::L8 => ColorType::L8,
                ColorType::La8 => ColorType::La8,
                ColorType::Rgb8 => ColorType::Rgb8,
                _ => ColorType::Rgba8,
            };
            
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut output, 
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive
            );
            
            let raw_data = match color_type {
                ColorType::L8 => img.to_luma8().into_raw(),
                ColorType::La8 => img.to_luma_alpha8().into_raw(),
                ColorType::Rgb8 => img.to_rgb8().into_raw(),
                _ => img.to_rgba8().into_raw(),
            };
            
            encoder.write_image(&raw_data, img.width(), img.height(), color_type)
                .map_err(|e| DomainError::Internal(format!("PNG encoding error: {}", e)))?;
        },
        ImageFormat::WebP => {
            #[cfg(feature = "webp")]
            {
                // Use lossless WebP compression
                webp::Encoder::from_image(&img)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?
                    .encode_lossless()
                    .write_to(&mut output)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?;
            }
            #[cfg(not(feature = "webp"))]
            {
                // Fall back to PNG
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut output, 
                    image::codecs::png::CompressionType::Best,
                    image::codecs::png::FilterType::Adaptive
                );
                let rgba = img.to_rgba8();
                encoder.write_image(&rgba, img.width(), img.height(), ColorType::Rgba8)
                    .map_err(|e| DomainError::Internal(format!("WebP->PNG fallback error: {}", e)))?;
            }
        },
        _ => {
            // For JPEG and other lossy formats, convert to PNG for lossless
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut output, 
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive
            );
            let rgba = img.to_rgba8();
            encoder.write_image(&rgba, img.width(), img.height(), ColorType::Rgba8)
                .map_err(|e| DomainError::Internal(format!("Lossless PNG encoding error: {}", e)))?;
        }
    }
    
    Ok(output)
}

/// Strip EXIF metadata from JPEG data (simplified for performance)
pub async fn strip_exif_metadata(data: &[u8]) -> DomainResult<Vec<u8>> {
    // For now, just return the original data to avoid performance issues
    // In production, you could implement more sophisticated EXIF stripping
    println!("📷 [IMAGE_COMPRESSOR] Skipping EXIF stripping for performance");
    Ok(data.to_vec())
}

/// Convert image to sRGB color space for consistent compression (simplified)
pub async fn convert_to_srgb(data: &[u8]) -> DomainResult<Vec<u8>> {
    // For performance, skip complex color space conversion for now
    println!("🎨 [IMAGE_COMPRESSOR] Skipping color space conversion for performance");
    Ok(data.to_vec())
}