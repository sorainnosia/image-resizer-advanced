// Advanced Image Resizer with Beautiful UI
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod compression;
mod simple;

use compression::{CompressionAlgorithm, CompressionOptions, SmartCompressor};
use iced::widget::{button, column, container, pick_list, progress_bar, row, scrollable, text, text_input, checkbox, slider, Space, radio, horizontal_rule, vertical_rule};
use iced::{executor, Application, Command, Element, Length, Settings, Theme, Font, Color, Background};
use iced::theme;
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

const LIGHT_FONT: Font = Font {
    family: Family::SansSerif,
    weight: Weight::Light,
    stretch: iced::font::Stretch::Normal,
    monospaced: false,
};

// Custom theme colors
const PRIMARY_COLOR: Color = Color::from_rgb(0.2, 0.5, 0.9);
const SECONDARY_COLOR: Color = Color::from_rgb(0.9, 0.95, 1.0);
const SUCCESS_COLOR: Color = Color::from_rgb(0.2, 0.7, 0.3);
const ERROR_COLOR: Color = Color::from_rgb(0.9, 0.2, 0.2);
const BACKGROUND_COLOR: Color = Color::from_rgb(0.97, 0.97, 0.98);
const CARD_COLOR: Color = Color::WHITE;

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
            size: (580, 680),
            min_size: Some((560, 680)),
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
    auto_scale: bool,
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
    AutoScaleToggled(bool),
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
        app.quality_slider = 85;
        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("Image Resizer Pro")
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
                if mode == CompressionMode::Simple {
                    self.compression_algorithm = CompressionAlgorithm::Simple;
                    self.quality_slider = 85;
                }
            }
            Message::AlgorithmSelected(algorithm) => {
                self.compression_algorithm = algorithm;
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
                    let auto_scale = self.auto_scale;
                    
                    if algorithm == CompressionAlgorithm::Simple {
                        return Command::perform(
                            simple::process_images(
                                path,
                                target_size,
                                dimensions,
                                maintain_ratio,
                                auto_scale,
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
                self.status_message = format!("Processed {} images successfully!", self.results.len());
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
        // Header section with gradient background
        let header = container(
            column![
                text("Image Resizer Pro")
                    .size(18)
                    .font(HEADING_FONT)
                    .style(Color::WHITE),
                text("Compress and resize your images with style")
                    .size(14)
                    .font(LIGHT_FONT)
                    .style(Color::from_rgba(1.0, 1.0, 1.0, 0.8)),
            ].spacing(4)
        )
        .width(Length::Fill)
        .padding([18, 26])
        .style(theme::Container::Custom(Box::new(GradientContainer)));

        // File selection card
        let file_selection_card = container(
            column![
                row![
                    icon_text("", "Select Images", 14, 14),
                    Space::with_width(Length::Fill),
                ].spacing(8),
                
                Space::with_height(12),
                
                row![
                    styled_button("Select File", Message::SelectFile, ButtonStyle::Primary),
                    styled_button("Select Folder", Message::SelectFolder, ButtonStyle::Secondary),
                ].spacing(8),
                
                Space::with_height(12),
                
                if let Some(path) = &self.selected_path {
                    let display_path = path.display().to_string();
                    let truncated = if display_path.len() > 50 {
                        format!("...{}", &display_path[display_path.len()-47..])
                    } else {
                        display_path
                    };
                    container(
                        text(format!("{}", truncated))
                            .size(13)
                            .font(BODY_FONT)
                            .style(Color::from_rgb(0.4, 0.4, 0.5))
                    )
                    .width(Length::Fill)
                    .padding([8, 12])
                    .style(theme::Container::Custom(Box::new(SubtleContainer)))
                } else {
                    container(
                        text("No files selected yet")
                            .size(13)
                            .font(LIGHT_FONT)
                            .style(Color::from_rgb(0.6, 0.6, 0.7))
                    )
                    .width(Length::Fill)
                    .padding([8, 12])
                    .style(theme::Container::Custom(Box::new(SubtleContainer)))
                }
            ].spacing(0)
        )
        .width(Length::Fill)
        .padding(8)
        .style(theme::Container::Custom(Box::new(CardContainer)));

        // Compression mode selection with visual tabs
        let mode_selection_card = container(
            column![
                icon_text("", "Compression Mode", 14, 14),
                
                Space::with_height(12),
                
                row![
                    mode_button("Simple", "Fast & Easy", CompressionMode::Simple, self.compression_mode),
                    Space::with_width(12),
                    mode_button("Advanced", "Full Control", CompressionMode::Advanced, self.compression_mode),
                ].spacing(0),
            ].spacing(0)
        )
        .width(Length::Fill)
        .padding(8)
        .style(theme::Container::Custom(Box::new(CardContainer)));

        // Compression settings card
        let compression_settings = match self.compression_mode {
            CompressionMode::Simple => {
                container(
                    column![
                        icon_text("", "Simple Settings", 14, 14),
                        Space::with_height(12),
                        styled_checkbox(
                            "Auto Scale (resize to meet target size)",
                            self.auto_scale,
                            Message::AutoScaleToggled
                        ),
                    ].spacing(0)
                )
                .width(Length::Fill)
                .padding(8)
                .style(theme::Container::Custom(Box::new(CardContainer)))
            }
            CompressionMode::Advanced => {
                container(
                    column![
                        icon_text("", "Advanced Settings", 14, 14),
                        
                        Space::with_height(12),
                        
                        row![
                            text("Algorithm")
                                .size(14)
                                .font(BODY_FONT)
                                .style(Color::from_rgb(0.3, 0.3, 0.4))
                                .width(100),
                            pick_list(
                                &[
                                    CompressionAlgorithm::Auto,
                                    CompressionAlgorithm::Simple,
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
                            .width(Length::Fill)
                            .padding([8, 12])
                            .text_size(14),
                        ].spacing(12).align_items(iced::Alignment::Center),
                        
                        if self.compression_algorithm.supports_quality() {
                            column![
                                Space::with_height(16),
                                row![
                                    text("Quality")
                                        .size(14)
                                        .font(BODY_FONT)
                                        .style(Color::from_rgb(0.3, 0.3, 0.4))
                                        .width(100),
                                    slider(10..=100, self.quality_slider, Message::QualityChanged)
                                        .width(Length::Fill),
                                    container(
                                        text(format!("{}%", self.quality_slider))
                                            .size(14)
                                            .font(HEADING_FONT)
                                            .style(PRIMARY_COLOR)
                                    )
                                    .width(50)
                                    .center_x(),
                                ].spacing(12).align_items(iced::Alignment::Center),
                            ].spacing(0)
                        } else {
                            column![]
                        },
                        
                        Space::with_height(12),
                        
                        styled_checkbox("Optimize for web", self.optimize_for_web, Message::OptimizeForWebToggled),
                    ].spacing(0)
                )
                .width(Length::Fill)
                .padding(8)
                .style(theme::Container::Custom(Box::new(CardContainer)))
            }
        };

        // Size parameters card
        let parameters_card = container(
            column![
                icon_text("", "Size Parameters", 14, 14),
                
                Space::with_height(16),
                
                row![
                    text("Target Size")
                        .size(14)
                        .font(BODY_FONT)
                        .style(Color::from_rgb(0.3, 0.3, 0.4))
                        .width(100),
                    text_input("Optional (KB)", &self.target_size)
                        .on_input(Message::TargetSizeChanged)
                        .width(Length::Fill)
                        .padding([8, 12])
                        .size(14),
                ].spacing(12).align_items(iced::Alignment::Center),
                
                Space::with_height(12),
                
                row![
                    text("Dimensions")
                        .size(14)
                        .font(BODY_FONT)
                        .style(Color::from_rgb(0.3, 0.3, 0.4))
                        .width(100),
                    text_input("Width", &self.width)
                        .on_input(Message::WidthChanged)
                        .width(Length::Fixed(80.0))
                        .padding([8, 12])
                        .size(14),
                    text("×")
                        .size(16)
                        .font(BODY_FONT)
                        .style(Color::from_rgb(0.5, 0.5, 0.6)),
                    text_input("Height", &self.height)
                        .on_input(Message::HeightChanged)
                        .width(Length::Fixed(80.0))
                        .padding([8, 12])
                        .size(14),
                    text("px")
                        .size(14)
                        .font(BODY_FONT)
                        .style(Color::from_rgb(0.5, 0.5, 0.6)),
                ].spacing(8).align_items(iced::Alignment::Center),
                
                Space::with_height(12),
                
                styled_checkbox("Maintain aspect ratio", self.maintain_ratio, Message::MaintainRatioToggled),
            ].spacing(0)
        )
        .width(Length::Fill)
        .padding(8)
        .style(theme::Container::Custom(Box::new(CardContainer)));

        // Process button and progress
        let process_section = column![
            if self.is_processing {
                styled_button("Processing...", Message::Process, ButtonStyle::Disabled)
            } else if self.selected_path.is_some() && 
                     (!self.target_size.is_empty() || !self.width.is_empty() || !self.height.is_empty()) {
                styled_button("Process Images", Message::Process, ButtonStyle::Action)
            } else {
                styled_button("Process Images", Message::Process, ButtonStyle::Disabled)
            },
            
            if self.is_processing || self.progress > 0.0 {
                column![
                    Space::with_height(16),
                    container(
                        progress_bar(0.0..=1.0, self.progress)
                            .height(Length::Fixed(8.0))
                    )
                    .style(theme::Container::Custom(Box::new(ProgressContainer))),
                    Space::with_height(8),
                    text(&self.status_message)
                        .size(13)
                        .font(BODY_FONT)
                        .style(SUCCESS_COLOR),
                ].spacing(0)
            } else {
                column![]
            }
        ].spacing(0);

        // Results section
        let results_section = if !self.results.is_empty() {
            let results_list: Vec<Element<Message>> = self.results.iter().map(|result| {
                let (icon, color) = if result.success {
                    ("", SUCCESS_COLOR)
                } else {
                    ("", ERROR_COLOR)
                };
                
                container(
                    row![
                      
                        text(&result.filename)
                            .size(13)
                            .font(BODY_FONT)
                            .style(Color::from_rgb(0.2, 0.2, 0.3))
                            .width(Length::Fill),
                        if result.success {
                            text(format!("{} → {} KB", 
                                result.original_size / 1024, 
                                result.new_size / 1024
                            ))
                            .size(13)
                            .font(BODY_FONT)
                            .style(Color::from_rgb(0.4, 0.4, 0.5))
                        } else {
                            text(&result.message)
                                .size(13)
                                .font(BODY_FONT)
                                .style(ERROR_COLOR)
                        }
                    ].spacing(12).align_items(iced::Alignment::Center)
                )
                .padding([8, 12])
                .style(theme::Container::Custom(Box::new(ResultItemContainer {
                    success: result.success,
                })))
                .into()
            }).collect();

            container(
                column![
                    icon_text("", "Results", 14, 14),
                    Space::with_height(16),
                    container(
                        scrollable(
                            column(results_list).spacing(4)
                        ).height(Length::Fixed(150.0))
                    )
                    .style(theme::Container::Custom(Box::new(SubtleContainer)))
                    .padding(4),
                    Space::with_height(16),
                    row![
                        styled_button("Open Output", Message::OpenOutputFolder, ButtonStyle::Secondary),
                        styled_button("Clear", Message::ClearResults, ButtonStyle::Subtle),
                    ].spacing(12)
                ].spacing(0)
            )
            .width(Length::Fill)
            .padding(8)
            .style(theme::Container::Custom(Box::new(CardContainer)))
        } else {
            container(column![])
        };

        // Main layout with scrollable content
        let content = scrollable(
            column![
                header,
                container(
                    column![
                        file_selection_card,
                        mode_selection_card,
                        compression_settings,
                        parameters_card,
                        container(process_section)
                            .width(Length::Fill)
                            .padding([0, 20]),
                        results_section,
                        Space::with_height(20),
                    ].spacing(8)
                )
                .max_width(680)
                .center_x()
                .padding([20, 16, 0, 16])
            ].spacing(0)
        );

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::Container::Custom(Box::new(BackgroundContainer)))
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
    }
}

// Helper UI functions
fn icon_text(icon: &str, label: &str, icon_size: u16, text_size: u16) -> Element<'static, Message> {
    row![
        text(icon).size(icon_size),
        text(label)
            .size(text_size)
            .font(HEADING_FONT)
            .style(Color::from_rgb(0.2, 0.2, 0.3)),
    ].spacing(8).into()
}

fn styled_button(label: &str, on_press: Message, style: ButtonStyle) -> Element<'static, Message> {
    let btn = button(
        text(label)
            .size(14)
            .font(if matches!(style, ButtonStyle::Action) { HEADING_FONT } else { BODY_FONT })
            .horizontal_alignment(iced::alignment::Horizontal::Center)
    )
    .padding([10, 20]);
    
    match style {
        ButtonStyle::Primary => btn.on_press(on_press).style(theme::Button::Primary),
        ButtonStyle::Secondary => btn.on_press(on_press).style(theme::Button::Secondary),
        ButtonStyle::Action => btn.on_press(on_press).style(theme::Button::Positive),
        ButtonStyle::Subtle => btn.on_press(on_press).style(theme::Button::Text),
        ButtonStyle::Disabled => btn.style(theme::Button::Secondary),
    }.into()
}

fn mode_button(title: &str, subtitle: &str, mode: CompressionMode, current: CompressionMode) -> Element<'static, Message> {
    let is_selected = mode == current;
    
    button(
        column![
            text(title)
                .size(15)
                .font(HEADING_FONT)
                .style(if is_selected { PRIMARY_COLOR } else { Color::from_rgb(0.4, 0.4, 0.5) }),
            text(subtitle)
                .size(12)
                .font(LIGHT_FONT)
                .style(if is_selected { PRIMARY_COLOR } else { Color::from_rgb(0.6, 0.6, 0.7) }),
        ].spacing(2).align_items(iced::Alignment::Center)
    )
    .on_press(Message::ModeChanged(mode))
    .padding([12, 24])
    .style(if is_selected {
        theme::Button::Primary
    } else {
        theme::Button::Secondary
    })
    .into()
}

fn styled_checkbox(label: &str, is_checked: bool, on_toggle: impl Fn(bool) -> Message + 'static) -> Element<'static, Message> {
    checkbox(label, is_checked, on_toggle)
        .size(14)
        .spacing(10)
        .text_size(14)
        .into()
}

#[derive(Clone, Copy)]
enum ButtonStyle {
    Primary,
    Secondary,
    Action,
    Subtle,
    Disabled,
}

// Custom container styles
struct BackgroundContainer;
impl container::StyleSheet for BackgroundContainer {
    type Style = Theme;
    
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(BACKGROUND_COLOR)),
            ..Default::default()
        }
    }
}

struct CardContainer;
impl container::StyleSheet for CardContainer {
    type Style = Theme;
    
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(CARD_COLOR)),
            border_radius: 12.0.into(),
            border_width: 1.0,
            border_color: Color::from_rgba(0.0, 0.0, 0.0, 0.05),
            ..Default::default()
        }
    }
}

struct GradientContainer;
impl container::StyleSheet for GradientContainer {
    type Style = Theme;
    
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(PRIMARY_COLOR)),
            ..Default::default()
        }
    }
}

struct SubtleContainer;
impl container::StyleSheet for SubtleContainer {
    type Style = Theme;
    
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(SECONDARY_COLOR)),
            border_radius: 8.0.into(),
            border_width: 1.0,
            border_color: Color::from_rgba(0.0, 0.0, 0.0, 0.05),
            ..Default::default()
        }
    }
}

struct ProgressContainer;
impl container::StyleSheet for ProgressContainer {
    type Style = Theme;
    
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            border_radius: 4.0.into(),
            ..Default::default()
        }
    }
}

struct ResultItemContainer {
    success: bool,
}
impl container::StyleSheet for ResultItemContainer {
    type Style = Theme;
    
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        let bg_color = if self.success {
            Color::from_rgba(0.2, 0.7, 0.3, 0.05)
        } else {
            Color::from_rgba(0.9, 0.2, 0.2, 0.05)
        };
        
        container::Appearance {
            background: Some(Background::Color(bg_color)),
            border_radius: 6.0.into(),
            ..Default::default()
        }
    }
}

// Rest of the implementation remains the same
impl std::fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto (Smart Selection)"),
            Self::Simple => write!(f, "Simple (Fast)"),
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
    
    if algorithm == CompressionAlgorithm::Simple {
        let auto_scale = false;
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
    
    if let Some((width, height)) = dimensions {
        img = if maintain_ratio {
            img.resize(width, height, image::imageops::FilterType::Lanczos3)
        } else {
            img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
        };
    }
    
    let options = CompressionOptions {
        algorithm,
        quality: Some(quality),
        target_size: target_size_kb.map(|kb| kb * 1024),
        preserve_metadata: false,
        optimize_for_web,
    };
    
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
    
    let output_path = output_dir.join(format!(
        "{}_resized.{}",
        input_path.file_stem().unwrap().to_string_lossy(),
        compression_result.algorithm_used.file_extension()
    ));
    
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