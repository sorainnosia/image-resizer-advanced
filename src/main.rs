// Advanced Image Resizer with Simple/Advanced modes
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod compression;
mod simple;

use compression::{CompressionAlgorithm, CompressionOptions, SmartCompressor};
use iced::widget::{button, column, container, pick_list, progress_bar, row, scrollable, text, text_input, checkbox, slider, Space, radio};
use iced::{executor, Application, Command, Element, Length, Settings, Theme, Font};
use iced::font::{Family, Weight};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const HEADING_FONT: Font = Font {
    family: Family::SansSerif,
    weight: Weight::Bold,
    stretch: iced::font::Stretch::Normal,
    monospaced: false,
};

const BODY_FONT: Font = Font {
    family: Family::SansSerif,
    weight: Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    monospaced: false,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionMode {
    Simple,
    Advanced,
}

impl Default for CompressionMode {
    fn default() -> Self {
        Self::Simple
    }
}

pub fn main() -> iced::Result {
    ImageResizer::run(Settings {
        window: iced::window::Settings {
            size: (520, 650),
            min_size: Some((500, 600)),
            resizable: true,
            decorations: true,
            ..Default::default()
        },
        default_font: BODY_FONT,
        default_text_size: 14.0,
        ..Default::default()
    })
}

#[derive(Default)]
struct ImageResizer {
    selected_path: Option<PathBuf>,
    target_size: String,
    width: String,
    height: String,
    maintain_ratio: bool,
    compression_mode: CompressionMode,
    compression_algorithm: CompressionAlgorithm,
    quality_slider: u8,
    optimize_for_web: bool,
    auto_scale: bool,  // ADD THIS LINE	
    is_processing: bool,
    progress: f32,
    status_message: String,
    results: Vec<ProcessResult>,
}

#[derive(Debug, Clone)]
enum Message {
    SelectFile,
    SelectFolder,
    FileSelected(Option<PathBuf>),
    TargetSizeChanged(String),
    WidthChanged(String),
    HeightChanged(String),
    MaintainRatioToggled(bool),
    ModeChanged(CompressionMode),
    AlgorithmSelected(CompressionAlgorithm),
    QualityChanged(u8),
    OptimizeForWebToggled(bool),
    AutoScaleToggled(bool),  // ADD THIS LINE	
    Process,
    ProcessingComplete(Vec<ProcessResult>),
    OpenOutputFolder,
    ClearResults,
}

#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub filename: String,
    pub original_size: u64,
    pub new_size: u64,
    pub success: bool,
    pub message: String,
    pub algorithm_used: CompressionAlgorithm,
    pub compression_ratio: f32,
}

impl Application for ImageResizer {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let mut app = Self::default();
        app.quality_slider = 85; // Default quality
        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("Advanced Image Resizer")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SelectFile => {
                return Command::perform(select_file(), Message::FileSelected);
            }
            Message::SelectFolder => {
                return Command::perform(select_folder(), Message::FileSelected);
            }
			Message::AutoScaleToggled(value) => {
				self.auto_scale = value;
			}			
            Message::FileSelected(path) => {
                self.selected_path = path;
            }
            Message::TargetSizeChanged(value) => {
                self.target_size = value;
            }
            Message::WidthChanged(value) => {
                self.width = value;
            }
            Message::HeightChanged(value) => {
                self.height = value;
            }
            Message::MaintainRatioToggled(value) => {
                self.maintain_ratio = value;
            }
            Message::ModeChanged(mode) => {
                self.compression_mode = mode;
                // Reset to sensible defaults when switching modes
                if mode == CompressionMode::Simple {
                    self.compression_algorithm = CompressionAlgorithm::Simple;
                    self.quality_slider = 85;
                }
            }
            Message::AlgorithmSelected(algorithm) => {
                self.compression_algorithm = algorithm;
                // Update quality slider based on algorithm
                if algorithm.supports_quality() {
                    self.quality_slider = algorithm.recommended_quality();
                }
            }
            Message::QualityChanged(quality) => {
                self.quality_slider = quality;
            }
            Message::OptimizeForWebToggled(value) => {
                self.optimize_for_web = value;
            }
			Message::Process => {
				if let Some(path) = &self.selected_path {
					self.is_processing = true;
					self.progress = 0.0;
					self.results.clear();
					
					let path = path.clone();
					let target_size = self.target_size.parse::<u64>().ok();
					let dimensions = parse_dimensions(&self.width, &self.height);
					let maintain_ratio = self.maintain_ratio;
					let algorithm = self.compression_algorithm;
					let quality = self.quality_slider;
					let optimize_for_web = self.optimize_for_web;
					let auto_scale = self.auto_scale;  // ADD THIS LINE
		
					// Check if we should use simple processing
					if algorithm == CompressionAlgorithm::Simple {
						return Command::perform(
							simple::process_images(
								path,
								target_size,
								dimensions,
								maintain_ratio,
								auto_scale,  // ADD THIS PARAMETER
							),
							|results| Message::ProcessingComplete(
								results.into_iter().map(|r| ProcessResult {
									filename: r.filename,
									original_size: r.original_size,
									new_size: r.new_size,
									success: r.success,
									message: r.message,
									algorithm_used: CompressionAlgorithm::Simple,
									compression_ratio: if r.original_size > 0 {
										r.new_size as f32 / r.original_size as f32
									} else {
										0.0
									},
								}).collect()
							)
						);
					} else {
						return Command::perform(
							process_images_advanced(
								path,
								target_size,
								dimensions,
								maintain_ratio,
								algorithm,
								quality,
								optimize_for_web,
							),
							Message::ProcessingComplete
						);
					}
				}
			}
            Message::ProcessingComplete(results) => {
                self.is_processing = false;
                self.progress = 1.0;
                self.results = results;
                self.status_message = format!("Processed {} images", self.results.len());
            }
            Message::OpenOutputFolder => {
                if let Some(path) = &self.selected_path {
                    let output_dir = path.parent().unwrap_or(Path::new(".")).join("resized");
                    if output_dir.exists() {
                        let _ = open::that(output_dir);
                    }
                }
            }
            Message::ClearResults => {
                self.results.clear();
                self.progress = 0.0;
                self.status_message.clear();
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        // Title
        let title = text("Advanced Image Resizer")
            .size(22)
            .font(HEADING_FONT);

        // File selection
        let file_selection = column![
            text("Select Images")
                .size(16)
                .font(HEADING_FONT),
            row![
                button("Select File")
                    .on_press(Message::SelectFile)
                    .padding([6, 12]),
                button("Select Folder")
                    .on_press(Message::SelectFolder)
                    .padding([6, 12]),
            ].spacing(8),
            if let Some(path) = &self.selected_path {
                let display_path = path.display().to_string();
                let truncated = if display_path.len() > 60 {
                    format!("...{}", &display_path[display_path.len()-57..])
                } else {
                    display_path
                };
                text(format!("Selected: {}", truncated))
                    .size(12)
                    .font(BODY_FONT)
            } else {
                text("No file selected")
                    .size(12)
                    .font(BODY_FONT)
            }
        ].spacing(8);

        // Mode selection
        let mode_selection = column![
            text("Compression Mode")
                .size(16)
                .font(HEADING_FONT),
            row![
                radio(
                    "Simple",
                    CompressionMode::Simple,
                    Some(self.compression_mode),
                    Message::ModeChanged,
                ).size(13).spacing(8),
                Space::with_width(20),
                radio(
                    "Advanced",
                    CompressionMode::Advanced,
                    Some(self.compression_mode),
                    Message::ModeChanged,
                ).size(13).spacing(8),
            ].spacing(12),
        ].spacing(8);

        // Compression settings (varies by mode)
        let compression_settings = match self.compression_mode {
            CompressionMode::Simple => {
                // Simple mode - just quality slider
				column![
						checkbox("Auto Scale (resize to meet target size)", self.auto_scale, Message::AutoScaleToggled)
							.size(13)
							.spacing(8),
					].spacing(8)            }
            CompressionMode::Advanced => {
                // Advanced mode - full algorithm selection
                column![
                    text("Compression Settings")
                        .size(16)
                        .font(HEADING_FONT),
                    row![
                        text("Algorithm:")
                            .size(13)
                            .font(BODY_FONT)
                            .width(80),
                        pick_list(
							&[
								CompressionAlgorithm::Auto,
								CompressionAlgorithm::Simple, // Add this
								CompressionAlgorithm::StandardJpeg,
								CompressionAlgorithm::MozJpeg,
								CompressionAlgorithm::StandardPng,
								CompressionAlgorithm::OptiPng,
								CompressionAlgorithm::OxiPng,
								CompressionAlgorithm::PngQuant,
								CompressionAlgorithm::WebPLossy,
								CompressionAlgorithm::WebPLossless,
							][..],
							Some(self.compression_algorithm),
							Message::AlgorithmSelected,
						)
                    ].spacing(8),
                    
                    // Quality slider (only for lossy formats)
                    if self.compression_algorithm.supports_quality() {
                        column![
                            row![
                                text("Quality:")
                                    .size(13)
                                    .font(BODY_FONT)
                                    .width(80),
                                slider(10..=100, self.quality_slider, Message::QualityChanged)
                                    .width(Length::Fill),
                                text(format!("{}%", self.quality_slider))
                                    .size(13)
                                    .font(BODY_FONT)
                                    .width(40),
                            ].spacing(8),
                        ].spacing(4)
                    } else {
                        column![]
                    },
                    
                    checkbox("Optimize for web", self.optimize_for_web, Message::OptimizeForWebToggled)
                        .size(13)
                        .spacing(8),
                ].spacing(8)
            }
        };

        // Size parameters
        let parameters = column![
            text("Size Parameters")
                .size(16)
                .font(HEADING_FONT),
            row![
                text("Target KB:")
                    .size(13)
                    .font(BODY_FONT)
                    .width(80),
                text_input("Optional", &self.target_size)
                    .on_input(Message::TargetSizeChanged)
                    .width(Length::Fixed(120.0))
                    .padding(4)
                    .size(13),
            ].spacing(8),
            row![
                text("Dimensions:")
                    .size(13)
                    .font(BODY_FONT)
                    .width(80),
                text_input("W", &self.width)
                    .on_input(Message::WidthChanged)
                    .width(Length::Fixed(55.0))
                    .padding(4)
                    .size(13),
                text("×")
                    .size(13)
                    .font(BODY_FONT),
                text_input("H", &self.height)
                    .on_input(Message::HeightChanged)
                    .width(Length::Fixed(55.0))
                    .padding(4)
                    .size(13),
                text("px")
                    .size(13)
                    .font(BODY_FONT),
            ].spacing(6),
            checkbox("Maintain aspect ratio", self.maintain_ratio, Message::MaintainRatioToggled)
                .size(13)
                .spacing(8),
        ].spacing(8);

        // Process button
        let process_button = if self.is_processing {
            button("Processing...")
                .padding([8, 16])
        } else if self.selected_path.is_some() && 
                 (!self.target_size.is_empty() || !self.width.is_empty() || !self.height.is_empty()) {
            button("Process Images")
                .on_press(Message::Process)
                .padding([8, 16])
        } else {
            button("Process Images")
                .padding([8, 16])
        };

        // Progress
        let progress_section = if self.is_processing || self.progress > 0.0 {
            column![
                progress_bar(0.0..=1.0, self.progress)
                    .height(Length::Fixed(6.0)),
                text(&self.status_message)
                    .size(12)
                    .font(BODY_FONT),
            ].spacing(4)
        } else {
            column![]
        };

        // Results
        let results_section = if !self.results.is_empty() {
            let results_list: Vec<Element<Message>> = self.results.iter().map(|result| {
                let status = if result.success { "[OK]" } else { "[FAIL]" };
                
                row![
                    text(status)
                        .size(12)
                        .font(if result.success { BODY_FONT } else { HEADING_FONT })
                        .width(40),
                    text(&result.filename)
                        .size(12)
                        .font(BODY_FONT)
                        .width(Length::Fill),
                    if result.success {
                        text(format!("{} → {} KB ({})", 
                            result.original_size / 1024, 
                            result.new_size / 1024,
                            result.algorithm_used.file_extension()
                        ))
                        .size(12)
                        .font(BODY_FONT)
                    } else {
                        text(&result.message)
                            .size(12)
                            .font(BODY_FONT)
                    }
                ].spacing(8).into()
            }).collect();

            column![
                text("Results")
                    .size(16)
                    .font(HEADING_FONT),
                container(
                    scrollable(
                        column(results_list).spacing(3)
                    ).height(Length::Fixed(100.0))
                )
                .style(iced::theme::Container::Box)
                .padding(8),
                row![
                    button("Open Output")
                        .on_press(Message::OpenOutputFolder)
                        .padding([6, 12]),
                    button("Clear")
                        .on_press(Message::ClearResults)
                        .padding([6, 12]),
                ].spacing(8)
            ].spacing(8)
        } else {
            column![]
        };

        // Main layout
        let content = column![
            title,
            Space::with_height(12),
            file_selection,
            Space::with_height(12),
            mode_selection,
            Space::with_height(12),
            compression_settings,
            Space::with_height(12),
            parameters,
            Space::with_height(12),
            process_button,
            Space::with_height(8),
            progress_section,
            if !self.results.is_empty() {
                Space::with_height(12)
            } else {
                Space::with_height(0)
            },
            results_section,
        ]
        .padding(16);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            //.center_x()
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
    }
}

impl std::fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto (Smart Selection)"),
            Self::Simple => write!(f, "Simple (Fast)"), // Add this
            Self::StandardJpeg => write!(f, "JPEG Standard"),
            Self::MozJpeg => write!(f, "JPEG (MozJPEG)"),
            Self::StandardPng => write!(f, "PNG Standard"),
            Self::OptiPng => write!(f, "PNG (OptiPNG)"),
            Self::OxiPng => write!(f, "PNG (OxiPNG)"),
            Self::PngQuant => write!(f, "PNG (PNGQuant Lossy)"),
            Self::WebPLossy => write!(f, "WebP Lossy"),
            Self::WebPLossless => write!(f, "WebP Lossless"),
            Self::Avif => write!(f, "AVIF"),
        }
    }
}

// Helper functions
async fn select_file() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("Images", &["jpg", "jpeg", "png", "gif", "bmp", "webp"])
        .pick_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}

async fn select_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}

fn parse_dimensions(width: &str, height: &str) -> Option<(u32, u32)> {
    match (width.parse::<u32>(), height.parse::<u32>()) {
        (Ok(w), Ok(h)) => Some((w, h)),
        _ => None,
    }
}

async fn process_images_advanced(
    path: PathBuf,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    maintain_ratio: bool,
    algorithm: CompressionAlgorithm,
    quality: u8,
    optimize_for_web: bool,
) -> Vec<ProcessResult> {
    tokio::task::spawn_blocking(move || {
        let compressor = SmartCompressor::new();
        let images = collect_images(&path).unwrap_or_default();
        let mut results = Vec::new();
        
        for image_path in images {
            let filename = image_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            
            let result = process_single_image_advanced(
                &image_path,
                target_size_kb,
                dimensions,
                maintain_ratio,
                algorithm,
                quality,
                optimize_for_web,
                &compressor,
            );
            
            results.push(ProcessResult {
                filename,
                original_size: result.original_size,
                new_size: result.new_size,
                success: result.success,
                message: result.message,
                algorithm_used: result.algorithm_used,
                compression_ratio: result.compression_ratio,
            });
        }
        
        results
    }).await.unwrap_or_default()
}

struct InternalResult {
    original_size: u64,
    new_size: u64,
    success: bool,
    message: String,
    algorithm_used: CompressionAlgorithm,
    compression_ratio: f32,
}

fn process_single_image_advanced(
    input_path: &Path,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    maintain_ratio: bool,
    algorithm: CompressionAlgorithm,
    quality: u8,
    optimize_for_web: bool,
    compressor: &SmartCompressor,
) -> InternalResult {
    let original_size = match fs::metadata(input_path) {
        Ok(metadata) => metadata.len(),
        Err(e) => {
            return InternalResult {
                original_size: 0,
                new_size: 0,
                success: false,
                message: format!("Failed to read: {}", e),
                algorithm_used: algorithm,
                compression_ratio: 0.0,
            };
        }
    };
    
    // For Simple algorithm, use the simple processing logic
    if algorithm == CompressionAlgorithm::Simple {
		let auto_scale = false; // Default to false for direct API usage		
        let result = simple::process_single_image(
            input_path,
            target_size_kb,
            dimensions,
            maintain_ratio,
			auto_scale,
        );
        
        return InternalResult {
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
        };
    }
    
    // Rest of the advanced processing code...
    let mut img = match image::open(input_path) {
        Ok(img) => img,
        Err(e) => {
            return InternalResult {
                original_size,
                new_size: 0,
                success: false,
                message: format!("Failed to open: {}", e),
                algorithm_used: algorithm,
                compression_ratio: 0.0,
            };
        }
    };
    
    // Apply dimension resize if specified
    if let Some((width, height)) = dimensions {
        img = if maintain_ratio {
            img.resize(width, height, image::imageops::FilterType::Lanczos3)
        } else {
            img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
        };
    }
    
    // Create compression options
    let options = CompressionOptions {
        algorithm,
        quality: Some(quality),
        target_size: target_size_kb.map(|kb| kb * 1024),
        preserve_metadata: false,
        optimize_for_web,
    };
    
    // Compress using advanced algorithm
    let compression_result = match compressor.compress(&img, options) {
        Ok(result) => result,
        Err(e) => {
            return InternalResult {
                original_size,
                new_size: 0,
                success: false,
                message: format!("Compression failed: {}", e),
                algorithm_used: algorithm,
                compression_ratio: 0.0,
            };
        }
    };
    
    // Create output directory
    let output_dir = input_path.parent().unwrap_or(Path::new(".")).join("resized");
    if let Err(e) = fs::create_dir_all(&output_dir) {
        return InternalResult {
            original_size,
            new_size: 0,
            success: false,
            message: format!("Failed to create dir: {}", e),
            algorithm_used: algorithm,
            compression_ratio: 0.0,
        };
    }
    
    // Determine output filename with appropriate extension
    let output_path = output_dir.join(format!(
        "{}_resized.{}",
        input_path.file_stem().unwrap().to_string_lossy(),
        compression_result.algorithm_used.file_extension()
    ));
    
    // Save compressed image
    if let Err(e) = fs::write(&output_path, &compression_result.data) {
        return InternalResult {
            original_size,
            new_size: 0,
            success: false,
            message: format!("Save failed: {}", e),
            algorithm_used: algorithm,
            compression_ratio: 0.0,
        };
    }
    
    InternalResult {
        original_size,
        new_size: compression_result.data.len() as u64,
        success: true,
        message: String::new(),
        algorithm_used: compression_result.algorithm_used,
        compression_ratio: compression_result.compression_ratio,
    }
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
            matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "avif")
        }
        None => false,
    }
}