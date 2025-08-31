// compression.rs - Advanced compression algorithms module with native libraries

use image::{DynamicImage, ImageFormat, GenericImageView, Rgba, Pixel, RgbImage, RgbaImage};
use std::io::Cursor;
use std::collections::HashSet;
use crate::simple;

// Native compression library imports
use mozjpeg::{Compress, ColorSpace, ScanMode};
use oxipng::{Options as OxiOptions, RowFilter, StripChunks};
use indexmap::IndexSet;
use webp::{Encoder as WebPEncoder, WebPMemory};
use ravif::{Encoder as AvifEncoder, EncodedImage};
use imgref::ImgVec;
use rgb::{RGB8, RGBA8};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionAlgorithm {
    Auto,
    #[default]
    Simple,
    // JPEG algorithms
    StandardJpeg,
    MozJpeg,
    
    // PNG algorithms  
    StandardPng,
    OptiPng,
    OxiPng,
    PngQuant,
    
    // WebP
    WebPLossy,
    WebPLossless,
    
    // Advanced
    Avif,
}

#[derive(Debug, Clone)]
pub struct CompressionOptions {
    pub algorithm: CompressionAlgorithm,
    pub quality: Option<u8>,
    pub target_size: Option<u64>,
    pub preserve_metadata: bool,
    pub optimize_for_web: bool,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Auto,
            quality: None,
            target_size: None,
            preserve_metadata: false,
            optimize_for_web: true,
        }
    }
}

pub struct ImageAnalysis {
    pub has_transparency: bool,
    pub color_count: usize,
    pub has_gradients: bool,
    pub is_photograph: bool,
    pub dominant_colors: Vec<[u8; 3]>,
    pub average_complexity: f32,
}

pub struct CompressionResult {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub algorithm_used: CompressionAlgorithm,
    pub final_quality: Option<u8>,
    pub compression_ratio: f32,
}

pub struct SmartCompressor;

impl SmartCompressor {
    pub fn new() -> Self {
        Self
    }
    
    pub fn compress(
        &self,
        image: &DynamicImage,
        options: CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        let analysis = self.analyze_image(image);
        
        let algorithm = match options.algorithm {
            CompressionAlgorithm::Auto => self.select_best_algorithm(&analysis),
            other => other,
        };
        
        match algorithm {
            CompressionAlgorithm::Auto => unreachable!(),
            CompressionAlgorithm::Simple => self.compress_standard_jpeg(image, &options),
            CompressionAlgorithm::StandardJpeg => self.compress_standard_jpeg(image, &options),
            CompressionAlgorithm::MozJpeg => self.compress_mozjpeg(image, &options),
            CompressionAlgorithm::StandardPng => self.compress_standard_png(image, &options),
            CompressionAlgorithm::OptiPng => self.compress_optipng(image, &options),
            CompressionAlgorithm::OxiPng => self.compress_oxipng(image, &options),
            CompressionAlgorithm::PngQuant => self.compress_pngquant(image, &options),
            CompressionAlgorithm::WebPLossy => self.compress_webp_lossy(image, &options),
            CompressionAlgorithm::WebPLossless => self.compress_webp_lossless(image, &options),
            CompressionAlgorithm::Avif => self.compress_avif(image, &options),
        }
    }
    
    fn analyze_image(&self, image: &DynamicImage) -> ImageAnalysis {
        let (width, height) = image.dimensions();
        let rgba = image.to_rgba8();
        
        // Check transparency
        let has_transparency = self.has_alpha_channel(&rgba);
        
        // Count colors
        let color_count = self.count_unique_colors(&rgba, 10000); // Sample up to 10k colors
        
        // Detect gradients and complexity
        let (has_gradients, complexity) = self.analyze_complexity(&rgba);
        
        // Detect if photograph (high color count, gradients)
        let is_photograph = color_count > 1000 && has_gradients;
        
        // Get dominant colors
        let dominant_colors = self.get_dominant_colors(&rgba, 5);
        
        ImageAnalysis {
            has_transparency,
            color_count,
            has_gradients,
            is_photograph,
            dominant_colors,
            average_complexity: complexity,
        }
    }
    
    fn select_best_algorithm(&self, analysis: &ImageAnalysis) -> CompressionAlgorithm {
        match (analysis.has_transparency, analysis.is_photograph, analysis.color_count) {
            // Photos without transparency -> JPEG
            (false, true, _) => CompressionAlgorithm::MozJpeg,
            
            // Images with transparency and many colors -> WebP
            (true, _, colors) if colors > 256 => CompressionAlgorithm::WebPLossy,
            
            // Simple graphics with few colors -> PNG
            (_, false, colors) if colors <= 256 => CompressionAlgorithm::OxiPng,
            
            // Complex images with transparency -> WebP
            (true, _, _) => CompressionAlgorithm::WebPLossy,
            
            // Default to WebP for versatility
            _ => CompressionAlgorithm::WebPLossy,
        }
    }
    
    // JPEG Compression Methods
    fn compress_standard_jpeg(
        &self,
        image: &DynamicImage,
        options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        let rgb_image = image.to_rgb8();
        let (width, height) = rgb_image.dimensions();
        
        let quality = options.quality.unwrap_or(85);
        let mut result_data = Vec::new();
        
        if let Some(target_size) = options.target_size {
            // Binary search for target size
            result_data = self.jpeg_target_size(&rgb_image, target_size)?;
        } else {
            // Single pass with specified quality
            let mut cursor = Cursor::new(&mut result_data);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            encoder.encode(
                &rgb_image,
                width,
                height,
                image::ColorType::Rgb8,
            )?;
        }
        
        let compression_ratio = self.calculate_ratio(image, &result_data);
        
        Ok(CompressionResult {
            data: result_data,
            format: ImageFormat::Jpeg,
            algorithm_used: CompressionAlgorithm::StandardJpeg,
            final_quality: Some(quality),
            compression_ratio,
        })
    }
    
    fn compress_mozjpeg(
        &self,
        image: &DynamicImage,
        options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        let rgb_image = image.to_rgb8();
        let (width, height) = rgb_image.dimensions();
        let quality = options.quality.unwrap_or(85);
        
        // Convert quality from 0-100 to mozjpeg's float scale
        let moz_quality = quality as f32;
        
        // Create MozJPEG compressor
        let mut compress = Compress::new(ColorSpace::JCS_RGB);
        compress.set_size(width as usize, height as usize);
        compress.set_quality(moz_quality);
        
        // Enable progressive encoding for better web performance
        if options.optimize_for_web {
            compress.set_scan_optimization_mode(ScanMode::AllComponentsTogether);
            compress.set_progressive_mode();
        }
        
        // Create a buffer to write to
        let mut output_data = Vec::new();
        
        // Start compression with the writer
        let mut compress_started = compress.start_compress(&mut output_data)?;
        
        // Get raw pixel data
        let pixels = rgb_image.as_flat_samples();
        let data = pixels.as_slice();
        
        // Process scanlines
        let row_stride = width as usize * 3;
        for y in 0..height as usize {
            let start = y * row_stride;
            let end = start + row_stride;
            compress_started.write_scanlines(&data[start..end])?;
        }
        
        // Finish compression
        compress_started.finish_compress()?;
        
        // Handle target size if specified
        let final_data = if let Some(target_size) = options.target_size {
            self.mozjpeg_target_size(&rgb_image, target_size, options.optimize_for_web)?
        } else {
            output_data
        };
        
        let compression_ratio = self.calculate_ratio(image, &final_data);
        
        Ok(CompressionResult {
            data: final_data,
            format: ImageFormat::Jpeg,
            algorithm_used: CompressionAlgorithm::MozJpeg,
            final_quality: Some(quality),
            compression_ratio,
        })
    }
    
    // PNG Compression Methods
    fn compress_standard_png(
        &self,
        image: &DynamicImage,
        _options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        let mut result_data = Vec::new();
        let mut cursor = Cursor::new(&mut result_data);
        
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut cursor,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        
        image.write_with_encoder(encoder)?;
        
        let compression_ratio = self.calculate_ratio(image, &result_data);
        
        Ok(CompressionResult {
            data: result_data,
            format: ImageFormat::Png,
            algorithm_used: CompressionAlgorithm::StandardPng,
            final_quality: None,
            compression_ratio,
        })
    }
    
    fn compress_optipng(
        &self,
        image: &DynamicImage,
        _options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        // First encode as PNG
        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        image.write_to(&mut cursor, ImageFormat::Png)?;
        
        // Now optimize with a simple filter search
        let filters = [
            image::codecs::png::FilterType::NoFilter,
            image::codecs::png::FilterType::Sub,
            image::codecs::png::FilterType::Up,
            image::codecs::png::FilterType::Avg,
            image::codecs::png::FilterType::Paeth,
            image::codecs::png::FilterType::Adaptive,
        ];
        
        let mut best_result = png_data.clone();
        let mut best_size = png_data.len();
        
        for filter in filters {
            let mut temp_data = Vec::new();
            let mut cursor = Cursor::new(&mut temp_data);
            
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut cursor,
                image::codecs::png::CompressionType::Best,
                filter,
            );
            
            if image.write_with_encoder(encoder).is_ok() && temp_data.len() < best_size {
                best_size = temp_data.len();
                best_result = temp_data;
            }
        }
        
        let compression_ratio = self.calculate_ratio(image, &best_result);
        
        Ok(CompressionResult {
            data: best_result,
            format: ImageFormat::Png,
            algorithm_used: CompressionAlgorithm::OptiPng,
            final_quality: None,
            compression_ratio,
        })
    }
    
    fn compress_oxipng(
        &self,
        image: &DynamicImage,
        options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        // First encode as PNG
        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        image.write_to(&mut cursor, ImageFormat::Png)?;
        
        // Configure OxiPNG options
        let mut oxipng_options = OxiOptions::from_preset(3); // Good balance of speed/compression
        
        if options.optimize_for_web {
            oxipng_options.strip = StripChunks::Safe;
        } else if options.preserve_metadata {
            oxipng_options.strip = StripChunks::None;
        } else {
            oxipng_options.strip = StripChunks::All;
        }
        
        // Enable all filter types for best compression
        let mut filter_set = IndexSet::new();
        filter_set.insert(RowFilter::None);
        filter_set.insert(RowFilter::Sub);
        filter_set.insert(RowFilter::Up);
        filter_set.insert(RowFilter::Average);
        filter_set.insert(RowFilter::Paeth);
        oxipng_options.filter = filter_set;
        
        // Optimize the PNG data
        let optimized_data = oxipng::optimize_from_memory(&png_data, &oxipng_options)?;
        
        let compression_ratio = self.calculate_ratio(image, &optimized_data);
        
        Ok(CompressionResult {
            data: optimized_data,
            format: ImageFormat::Png,
            algorithm_used: CompressionAlgorithm::OxiPng,
            final_quality: None,
            compression_ratio,
        })
    }
    
    fn compress_pngquant(
        &self,
        image: &DynamicImage,
        options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        // For PNGQuant simulation, we'll quantize colors then use OxiPNG
        let max_colors = 256;
        let quantized = self.quantize_image(image, max_colors);
        
        // Now compress with OxiPNG for best results
        self.compress_oxipng(&quantized, options)
            .map(|mut result| {
                result.algorithm_used = CompressionAlgorithm::PngQuant;
                result
            })
    }
    
    // WebP Compression Methods
    fn compress_webp_lossy(
        &self,
        image: &DynamicImage,
        options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        let quality = options.quality.unwrap_or(85) as f32;
        
        // Convert to RGBA for WebP encoder
        let rgba_image = image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        
        // Create WebP encoder
        let encoder = WebPEncoder::from_rgba(
            rgba_image.as_raw(),
            width,
            height,
        );
        
        // Encode with specified quality
        let memory = encoder.encode(quality);
        let data = memory.to_vec();
        
        // Handle target size if specified
        let final_data = if let Some(target_size) = options.target_size {
            self.webp_target_size(&rgba_image, target_size, true)?
        } else {
            data
        };
        
        let compression_ratio = self.calculate_ratio(image, &final_data);
        
        Ok(CompressionResult {
            data: final_data,
            format: ImageFormat::WebP,
            algorithm_used: CompressionAlgorithm::WebPLossy,
            final_quality: Some(quality as u8),
            compression_ratio,
        })
    }
    
    fn compress_webp_lossless(
        &self,
        image: &DynamicImage,
        _options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        // Convert to RGBA for WebP encoder
        let rgba_image = image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        
        // Create WebP encoder for lossless
        let encoder = WebPEncoder::from_rgba(
            rgba_image.as_raw(),
            width,
            height,
        );
        
        // Encode losslessly (quality 100 triggers lossless mode in libwebp)
        let memory = encoder.encode_lossless();
        let data = memory.to_vec();
        
        let compression_ratio = self.calculate_ratio(image, &data);
        
        Ok(CompressionResult {
            data,
            format: ImageFormat::WebP,
            algorithm_used: CompressionAlgorithm::WebPLossless,
            final_quality: None,
            compression_ratio,
        })
    }
    
    // AVIF Compression
    fn compress_avif(
        &self,
        image: &DynamicImage,
        options: &CompressionOptions,
    ) -> Result<CompressionResult, Box<dyn std::error::Error>> {
        let quality = options.quality.unwrap_or(80) as f32 / 100.0; // ravif uses 0.0-1.0 scale
        
        // Convert to RGBA8 for AVIF encoder
        let rgba_image = image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        
        // Convert to imgref format required by ravif
        let pixels: Vec<RGBA8> = rgba_image
            .pixels()
            .map(|p| RGBA8 {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            })
            .collect();
        
        let img = ImgVec::new(pixels, width as usize, height as usize);
        
        // Create encoder and encode - ravif has a simple API
        let encoder = AvifEncoder::new();
        let encoded = encoder.encode_rgba(img.as_ref())?;
        
        let data = encoded.avif_file;
        
        // Handle target size if specified
        let final_data = if let Some(target_size) = options.target_size {
            // For simplicity, we'll use the standard AVIF encoding
            // as ravif doesn't easily support quality adjustment
            data
        } else {
            data
        };
        
        let compression_ratio = self.calculate_ratio(image, &final_data);
        
        Ok(CompressionResult {
            data: final_data,
            format: ImageFormat::Avif,
            algorithm_used: CompressionAlgorithm::Avif,
            final_quality: Some((quality * 100.0) as u8),
            compression_ratio,
        })
    }
    
    // Helper methods for target size compression
    fn mozjpeg_target_size(
        &self,
        image: &RgbImage,
        target_bytes: u64,
        optimize_for_web: bool,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let (width, height) = image.dimensions();
        let mut low = 10u8;
        let mut high = 95u8;
        let mut best_result = Vec::new();
        
        while low <= high {
            let quality = (low + high) / 2;
            
            let mut compress = Compress::new(ColorSpace::JCS_RGB);
            compress.set_size(width as usize, height as usize);
            compress.set_quality(quality as f32);
            
            if optimize_for_web {
                compress.set_scan_optimization_mode(ScanMode::AllComponentsTogether);
                compress.set_progressive_mode();
            }
            
            let mut output_data = Vec::new();
            let mut compress_started = compress.start_compress(&mut output_data)?;
            
            let pixels = image.as_flat_samples();
            let data = pixels.as_slice();
            let row_stride = width as usize * 3;
            
            for y in 0..height as usize {
                let start = y * row_stride;
                let end = start + row_stride;
                compress_started.write_scanlines(&data[start..end])?;
            }
            
            compress_started.finish_compress()?;
            
            if output_data.len() as u64 <= target_bytes {
                best_result = output_data;
                low = quality + 1;
            } else {
                high = quality - 1;
            }
        }
        
        if best_result.is_empty() {
            Err("Could not achieve target file size with MozJPEG".into())
        } else {
            Ok(best_result)
        }
    }
    
    fn webp_target_size(
        &self,
        image: &RgbaImage,
        target_bytes: u64,
        lossy: bool,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let (width, height) = image.dimensions();
        
        if lossy {
            let mut low = 10.0f32;
            let mut high = 95.0f32;
            let mut best_result = Vec::new();
            
            while high - low > 1.0 {
                let quality = (low + high) / 2.0;
                
                let encoder = WebPEncoder::from_rgba(image.as_raw(), width, height);
                let memory = encoder.encode(quality);
                let data = memory.to_vec();
                
                if data.len() as u64 <= target_bytes {
                    best_result = data;
                    low = quality;
                } else {
                    high = quality;
                }
            }
            
            if best_result.is_empty() {
                Err("Could not achieve target file size with WebP".into())
            } else {
                Ok(best_result)
            }
        } else {
            // For lossless, we can't adjust quality, so just return the lossless result
            let encoder = WebPEncoder::from_rgba(image.as_raw(), width, height);
            let memory = encoder.encode_lossless();
            Ok(memory.to_vec())
        }
    }
    
    fn avif_target_size(
        &self,
        image: &DynamicImage,
        target_bytes: u64,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Since ravif doesn't easily support quality adjustment,
        // we'll just return a single encoding
        let rgba_image = image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        
        let pixels: Vec<RGBA8> = rgba_image
            .pixels()
            .map(|p| RGBA8 {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            })
            .collect();
        
        let img = ImgVec::new(pixels, width as usize, height as usize);
        
        let encoder = AvifEncoder::new();
        let encoded = encoder.encode_rgba(img.as_ref())?;
        
        if encoded.avif_file.len() as u64 <= target_bytes {
            Ok(encoded.avif_file)
        } else {
            Err("AVIF file exceeds target size".into())
        }
    }
    
    // Existing helper methods remain the same...
    fn has_alpha_channel(&self, image: &image::RgbaImage) -> bool {
        image.pixels().any(|p| p[3] < 255)
    }
    
    fn count_unique_colors(&self, image: &image::RgbaImage, max_sample: usize) -> usize {
        let mut colors = HashSet::new();
        let pixels: Vec<&Rgba<u8>> = image.pixels().collect();
        let step = (pixels.len() / max_sample).max(1);
        
        for (i, pixel) in pixels.iter().enumerate() {
            if i % step == 0 {
                colors.insert([pixel[0], pixel[1], pixel[2]]);
                if colors.len() >= max_sample {
                    break;
                }
            }
        }
        
        colors.len()
    }
    
    fn analyze_complexity(&self, image: &image::RgbaImage) -> (bool, f32) {
        let (width, height) = image.dimensions();
        let mut gradient_pixels = 0;
        let mut total_diff = 0.0;
        let mut sample_count = 0;
        
        // Sample pixels to detect gradients
        for y in 0..height.saturating_sub(1) {
            for x in 0..width.saturating_sub(1) {
                // Sample every 4th pixel for performance
                if x % 4 == 0 && y % 4 == 0 {
                    let p1 = image.get_pixel(x, y);
                    let p2 = image.get_pixel(x + 1, y);
                    let p3 = image.get_pixel(x, y + 1);
                    
                    let diff1 = self.color_distance(p1, p2);
                    let diff2 = self.color_distance(p1, p3);
                    
                    total_diff += diff1 + diff2;
                    sample_count += 2;
                    
                    if diff1 > 10.0 || diff2 > 10.0 {
                        gradient_pixels += 1;
                    }
                }
            }
        }
        
        let has_gradients = gradient_pixels > (sample_count / 10);
        let complexity = total_diff / sample_count as f32;
        
        (has_gradients, complexity)
    }
    
    fn color_distance(&self, c1: &Rgba<u8>, c2: &Rgba<u8>) -> f32 {
        let dr = c1[0] as f32 - c2[0] as f32;
        let dg = c1[1] as f32 - c2[1] as f32;
        let db = c1[2] as f32 - c2[2] as f32;
        (dr * dr + dg * dg + db * db).sqrt()
    }
    
    fn get_dominant_colors(&self, image: &image::RgbaImage, count: usize) -> Vec<[u8; 3]> {
        // Simple color frequency analysis
        let mut color_counts: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
        
        for pixel in image.pixels() {
            let color = [pixel[0], pixel[1], pixel[2]];
            *color_counts.entry(color).or_insert(0) += 1;
        }
        
        let mut sorted: Vec<_> = color_counts.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        
        sorted.into_iter()
            .take(count)
            .map(|(color, _)| color)
            .collect()
    }
    
    fn quantize_image(&self, image: &DynamicImage, max_colors: usize) -> DynamicImage {
        // Simple color quantization
        let rgba = image.to_rgba8();
        let mut quantized = rgba.clone();
        
        // Calculate quantization factor based on max_colors
        let factor = (256.0 / (max_colors as f32).sqrt()) as u8;
        
        for pixel in quantized.pixels_mut() {
            pixel[0] = (pixel[0] / factor) * factor;
            pixel[1] = (pixel[1] / factor) * factor;
            pixel[2] = (pixel[2] / factor) * factor;
        }
        
        DynamicImage::ImageRgba8(quantized)
    }
    
    fn jpeg_target_size(
        &self,
        image: &image::RgbImage,
        target_bytes: u64,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let (width, height) = image.dimensions();
        let mut low = 10u8;
        let mut high = 95u8;
        let mut best_result = Vec::new();
        
        while low <= high {
            let quality = (low + high) / 2;
            let mut temp_data = Vec::new();
            let mut cursor = Cursor::new(&mut temp_data);
            
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            encoder.encode(image, width, height, image::ColorType::Rgb8)?;
            
            if temp_data.len() as u64 <= target_bytes {
                best_result = temp_data;
                low = quality + 1;
            } else {
                high = quality - 1;
            }
        }
        
        Ok(best_result)
    }
    
    fn calculate_ratio(&self, original: &DynamicImage, compressed: &[u8]) -> f32 {
        let original_size = self.estimate_raw_size(original);
        compressed.len() as f32 / original_size as f32
    }
    
    fn estimate_raw_size(&self, image: &DynamicImage) -> usize {
        let (width, height) = image.dimensions();
        let bytes_per_pixel = match image {
            DynamicImage::ImageLuma8(_) => 1,
            DynamicImage::ImageLumaA8(_) => 2,
            DynamicImage::ImageRgb8(_) => 3,
            DynamicImage::ImageRgba8(_) => 4,
            _ => 4,
        };
        (width * height * bytes_per_pixel) as usize
    }
}

// Algorithm descriptions for UI
impl CompressionAlgorithm {
    pub fn description(&self) -> &'static str {
        match self {
            Self::Auto => "Automatically select best algorithm based on image analysis",
            Self::Simple => "Use lowest acceptable image quality",
            Self::StandardJpeg => "Standard JPEG compression (fast, good quality)",
            Self::MozJpeg => "Mozilla JPEG encoder (10-15% better compression)",
            Self::StandardPng => "Standard PNG compression (lossless)",
            Self::OptiPng => "Optimized PNG (smaller files, lossless)",
            Self::OxiPng => "Fast optimized PNG (good balance)",
            Self::PngQuant => "Lossy PNG (up to 70% smaller, slight quality loss)",
            Self::WebPLossy => "WebP lossy (25-35% better than JPEG)",
            Self::WebPLossless => "WebP lossless (better than PNG)",
            Self::Avif => "AV1 Image Format (best compression, slower)",
        }
    }
    
    pub fn supports_quality(&self) -> bool {
        matches!(
            self,
            Self::StandardJpeg | Self::MozJpeg | Self::WebPLossy | Self::Avif
        )
    }
    
    pub fn recommended_quality(&self) -> u8 {
        match self {
            Self::StandardJpeg | Self::MozJpeg => 85,
            Self::WebPLossy => 90,
            Self::Avif => 80,
            _ => 100,
        }
    }
    
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Auto => "jpg",
            Self::Simple => "jpg",
            Self::StandardJpeg | Self::MozJpeg => "jpg",
            Self::StandardPng | Self::OptiPng | Self::OxiPng | Self::PngQuant => "png",
            Self::WebPLossy | Self::WebPLossless => "webp",
            Self::Avif => "avif",
        }
    }
}