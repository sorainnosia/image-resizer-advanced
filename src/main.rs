// Fixed Iced GUI - Compact layout with custom fonts
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use iced::widget::{button, column, container, progress_bar, row, scrollable, text, text_input, checkbox, Space};
use iced::{executor, Application, Command, Element, Length, Settings, Theme, Font};
use iced::font::{Family, Weight};
use image::{DynamicImage, ImageFormat};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Custom fonts
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

pub fn main() -> iced::Result {
    ImageResizer::run(Settings {
        window: iced::window::Settings {
            size: (480, 550),
            min_size: Some((400, 500)),
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
    Process,
    ProcessingComplete(Vec<ProcessResult>),
    OpenOutputFolder,
    ClearResults,
}

#[derive(Debug, Clone)]
struct ProcessResult {
    filename: String,
    original_size: u64,
    new_size: u64,
    success: bool,
    message: String,
}

impl Application for ImageResizer {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (Self::default(), Command::none())
    }

    fn title(&self) -> String {
        String::from("Image Resizer")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SelectFile => {
                return Command::perform(select_file(), Message::FileSelected);
            }
            Message::SelectFolder => {
                return Command::perform(select_folder(), Message::FileSelected);
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
            Message::Process => {
                if let Some(path) = &self.selected_path {
                    self.is_processing = true;
                    self.progress = 0.0;
                    self.results.clear();
                    
                    let path = path.clone();
                    let target_size = self.target_size.parse::<u64>().ok();
                    let dimensions = parse_dimensions(&self.width, &self.height);
                    let maintain_ratio = self.maintain_ratio;
                    
                    return Command::perform(
                        process_images(path, target_size, dimensions, maintain_ratio),
                        Message::ProcessingComplete
                    );
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
        let title = text("Image Resizer")
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
                let truncated = if display_path.len() > 50 {
                    format!("...{}", &display_path[display_path.len()-47..])
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

        // Parameters
        let parameters = column![
            text("Parameters")
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
                text("Size:")
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
                        text(format!("{} → {} KB", 
                            result.original_size / 1024, 
                            result.new_size / 1024
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
                    ).height(Length::Fixed(120.0))
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
            .center_x()
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
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

async fn process_images(
    path: PathBuf,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    maintain_ratio: bool,
) -> Vec<ProcessResult> {
    tokio::task::spawn_blocking(move || {
        let images = collect_images(&path).unwrap_or_default();
        let mut results = Vec::new();
        
        for image_path in images {
            let filename = image_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            
            let result = process_single_image(&image_path, target_size_kb, dimensions, maintain_ratio);
            
            results.push(ProcessResult {
                filename,
                original_size: result.original_size,
                new_size: result.new_size,
                success: result.success,
                message: result.message,
            });
        }
        
        results
    }).await.unwrap_or_default()
}

// Image processing
struct InternalResult {
    original_size: u64,
    new_size: u64,
    success: bool,
    message: String,
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

fn process_single_image(
    input_path: &Path,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    maintain_ratio: bool,
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
        match compress_to_size(img, target_size_kb.unwrap(), &output_path) {
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