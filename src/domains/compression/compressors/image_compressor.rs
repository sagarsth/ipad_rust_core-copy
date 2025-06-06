 //! Image compression implementation

use async_trait::async_trait;
use image::{ImageFormat, GenericImageView, DynamicImage, ImageEncoder};
use tokio::task;

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
        
        // Run image operations in a blocking task to avoid blocking the runtime
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            println!("🖼️ [IMAGE_COMPRESSOR] Detecting image format...");
            
            // Detect image format
            let format = image::guess_format(&data)
                .map_err(|e| {
                    println!("❌ [IMAGE_COMPRESSOR] Failed to detect image format: {}", e);
                    DomainError::Internal(format!("Failed to detect image format: {}", e))
                })?;
            
            println!("🖼️ [IMAGE_COMPRESSOR] Detected format: {:?}", format);
            
            // Load image
            println!("🖼️ [IMAGE_COMPRESSOR] Loading image from memory...");
            let img = image::load_from_memory(&data)
                .map_err(|e| {
                    println!("❌ [IMAGE_COMPRESSOR] Failed to load image: {}", e);
                    DomainError::Internal(format!("Failed to load image: {}", e))
                })?;
            
            println!("🖼️ [IMAGE_COMPRESSOR] Image loaded successfully: {}x{}", img.width(), img.height());
            
            let result = match method {
                CompressionMethod::Lossy => {
                    println!("🖼️ [IMAGE_COMPRESSOR] Applying lossy compression...");
                    compress_lossy(img, format, quality)
                },
                CompressionMethod::Lossless => {
                    println!("🖼️ [IMAGE_COMPRESSOR] Applying lossless compression...");
                    compress_lossless(img, format)
                },
                _ => {
                    println!("🖼️ [IMAGE_COMPRESSOR] Applying default compression...");
                    compress_default(img, format, quality)
                },
            };
            
            match result {
                Ok(compressed_data) => {
                    println!("✅ [IMAGE_COMPRESSOR] Compression successful: {} bytes -> {} bytes", 
                             data.len(), compressed_data.len());
                    Ok(compressed_data)
                },
                Err(e) => {
                    println!("❌ [IMAGE_COMPRESSOR] Compression failed: {:?}", e);
                    Err(e)
                }
            }
        }).await.map_err(|e| {
            println!("❌ [IMAGE_COMPRESSOR] Task join error: {}", e);
            DomainError::Internal(format!("Task join error: {}", e))
        })?
    }
}

fn compress_lossy(img: DynamicImage, format: ImageFormat, quality: u8) -> DomainResult<Vec<u8>> {
    let mut output = Vec::new();
    
    match format {
        ImageFormat::Jpeg => {
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
            encoder.encode_image(&img)
                .map_err(|e| DomainError::Internal(format!("JPEG encoding error: {}", e)))?;
        },
        ImageFormat::Png => {
            // For PNG, convert to an optimized color type based on image content
            let png = img.to_rgba8();
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut output, 
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive
            );
            encoder.write_image(
                &png, 
                png.width(), 
                png.height(), 
                image::ColorType::Rgba8
            ).map_err(|e| DomainError::Internal(format!("PNG encoding error: {}", e)))?;
        },
        ImageFormat::WebP => {
            #[cfg(feature = "webp")]
            {
                // If webp feature is enabled, use a webp encoder
                let quality = (quality as f32) / 100.0;
                webp::Encoder::from_image(&img)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?
                    .encode_lossless()
                    .write_to(&mut output)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?;
            }
            #[cfg(not(feature = "webp"))]
            {
                // Fall back to PNG
                let png = img.to_rgba8();
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut output, 
                    image::codecs::png::CompressionType::Best,
                    image::codecs::png::FilterType::Adaptive
                );
                encoder.write_image(
                    &png, 
                    png.width(), 
                    png.height(), 
                    image::ColorType::Rgba8
                ).map_err(|e| DomainError::Internal(format!("PNG encoding error: {}", e)))?;
            }
        },
        _ => {
            // For other formats, convert to JPEG with the specified quality
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality);
            encoder.encode_image(&img)
                .map_err(|e| DomainError::Internal(format!("JPEG encoding error: {}", e)))?;
        }
    }
    
    Ok(output)
}

fn compress_lossless(img: DynamicImage, format: ImageFormat) -> DomainResult<Vec<u8>> {
    let mut output = Vec::new();
    
    match format {
        ImageFormat::Png => {
            // For PNG, use best compression
            let png = img.to_rgba8();
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut output, 
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive
            );
            encoder.write_image(
                &png, 
                png.width(), 
                png.height(), 
                image::ColorType::Rgba8
            ).map_err(|e| DomainError::Internal(format!("PNG encoding error: {}", e)))?;
        },
        ImageFormat::WebP => {
            #[cfg(feature = "webp")]
            {
                // If webp feature is enabled, use a webp encoder
                webp::Encoder::from_image(&img)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?
                    .encode_lossless()
                    .write_to(&mut output)
                    .map_err(|e| DomainError::Internal(format!("WebP encoding error: {}", e)))?;
            }
            #[cfg(not(feature = "webp"))]
            {
                // Fall back to PNG
                let png = img.to_rgba8();
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut output, 
                    image::codecs::png::CompressionType::Best,
                    image::codecs::png::FilterType::Adaptive
                );
                encoder.write_image(
                    &png, 
                    png.width(), 
                    png.height(), 
                    image::ColorType::Rgba8
                ).map_err(|e| DomainError::Internal(format!("PNG encoding error: {}", e)))?;
            }
        },
        _ => {
            // For other formats, convert to PNG with best compression
            let png = img.to_rgba8();
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut output, 
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive
            );
            encoder.write_image(
                &png, 
                png.width(), 
                png.height(), 
                image::ColorType::Rgba8
            ).map_err(|e| DomainError::Internal(format!("PNG encoding error: {}", e)))?;
        }
    }
    
    Ok(output)
}

fn compress_default(img: DynamicImage, format: ImageFormat, quality: u8) -> DomainResult<Vec<u8>> {
    match format {
        ImageFormat::Jpeg | ImageFormat::WebP => compress_lossy(img, format, quality),
        _ => compress_lossless(img, format),
    }
}