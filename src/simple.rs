use iced::widget::{button, column, container, progress_bar, row, scrollable, text, text_input, checkbox, Space};
use iced::{executor, Application, Command, Element, Length, Settings, Theme, Font};
use iced::font::{Family, Weight};
use image::{DynamicImage, ImageFormat};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::ProcessResult;
use crate::CompressionAlgorithm;

pub async fn process_images(
    path: PathBuf,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    maintain_ratio: bool,
	auto_scale: bool
) -> Vec<ProcessResult> {
    tokio::task::spawn_blocking(move || {
        let images = collect_images(&path).unwrap_or_default();
        let mut results = Vec::new();
        
        for image_path in images {
            let filename = image_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            
            let result = process_single_image(&image_path, target_size_kb, dimensions, maintain_ratio, auto_scale);
            
            results.push(ProcessResult {
                filename,
                original_size: result.original_size,
                new_size: result.new_size,
                success: result.success,
                message: result.message,
				algorithm_used: CompressionAlgorithm::Simple,
				compression_ratio: if result.original_size > 0 {
					result.new_size as f32 / result.original_size as f32
				} else {
					0.0
				},
            });
        }
        
        results
    }).await.unwrap_or_default()
}

// Image processing
pub struct InternalResult {
    pub original_size: u64,
    pub new_size: u64,
    pub success: bool,
    pub message: String,
}

fn collect_images(path: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut images = Vec::new();
    
    if path.is_file() && is_image_file(path) {
        images.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && is_image_file(path) {
                images.push(path.to_path_buf());
            }
        }
    }
    
    Ok(images)
}

fn is_image_file(path: &Path) -> bool {
    match path.extension() {
        Some(ext) => {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp")
        }
        None => false,
    }
}

pub fn process_single_image(
    input_path: &Path,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    maintain_ratio: bool,
	auto_scale: bool
) -> InternalResult {
    let original_size = match fs::metadata(input_path) {
        Ok(metadata) => metadata.len(),
        Err(e) => {
            return InternalResult {
                original_size: 0,
                new_size: 0,
                success: false,
                message: format!("Failed to read: {}", e),
            };
        }
    };
    
    let mut img = match image::open(input_path) {
        Ok(img) => img,
        Err(e) => {
            return InternalResult {
                original_size,
                new_size: 0,
                success: false,
                message: format!("Failed to open: {}", e),
            };
        }
    };
    
    if let Some((width, height)) = dimensions {
        img = if maintain_ratio {
            img.resize(width, height, image::imageops::FilterType::Lanczos3)
        } else {
            img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
        };
    }
    
    let output_dir = input_path.parent().unwrap_or(Path::new(".")).join("resized");
    if let Err(e) = fs::create_dir_all(&output_dir) {
        return InternalResult {
            original_size,
            new_size: 0,
            success: false,
            message: format!("Failed to create dir: {}", e),
        };
    }
    
    let output_path = output_dir.join(format!(
        "{}_resized.{}",
        input_path.file_stem().unwrap().to_string_lossy(),
        input_path.extension().unwrap_or_default().to_string_lossy()
    ));
    
    if target_size_kb.is_none() {
        match img.save(&output_path) {
            Ok(_) => {
                let new_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                InternalResult {
                    original_size,
                    new_size,
                    success: true,
                    message: String::new(),
                }
            }
            Err(e) => InternalResult {
                original_size,
                new_size: 0,
                success: false,
                message: format!("Save failed: {}", e),
            },
        }
    } else {
        match compress_to_size(img, target_size_kb.unwrap(), &output_path, auto_scale) {
            Ok(new_size) => InternalResult {
                original_size,
                new_size,
                success: true,
                message: String::new(),
            },
            Err(e) => InternalResult {
                original_size,
                new_size: 0,
                success: false,
                message: e.to_string(),
            },
        }
    }
}

fn compress_to_size(
    mut img: DynamicImage,
    target_kb: u64,
    output_path: &Path,
	auto_scale: bool
) -> Result<u64, Box<dyn std::error::Error>> {
    let target_bytes = target_kb * 1024;
    let format = ImageFormat::Jpeg;
    
    for quality in (20..=95).rev().step_by(5) {
        let buffer = save_to_buffer(&img, format, quality)?;
        
        if buffer.len() <= target_bytes as usize {
            fs::write(output_path, &buffer)?;
            return Ok(buffer.len() as u64);
        }
    }
    
	if auto_scale {
		let mut scale = 0.9;
		while scale > 0.5 {
			let new_width = (img.width() as f32 * scale) as u32;
			let new_height = (img.height() as f32 * scale) as u32;
			img = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
			
			let buffer = save_to_buffer(&img, format, 75)?;
			
			if buffer.len() <= target_bytes as usize {
				fs::write(output_path, &buffer)?;
				return Ok(buffer.len() as u64);
			}
			
			scale *= 0.9;
		}
	}
    
    Err("Could not achieve target file size".into())
}

fn save_to_buffer(
    img: &DynamicImage,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = Cursor::new(Vec::new());
    
    match format {
        ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
            img.write_with_encoder(encoder)?;
        }
        _ => {
            img.write_to(&mut buffer, format)?;
        }
    }
    
    Ok(buffer.into_inner())
}