use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub id: String,
    pub source: String,
    pub target: String,
    pub lang_pair: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub triple_copy: String,
    pub translate_replace: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            triple_copy: "Ctrl+C+C+C".to_string(),
            translate_replace: "Ctrl+Alt+T".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_source_lang: String,
    pub default_target_lang: String,
    pub use_gpu: bool,
    #[serde(default = "default_model_version")]
    pub model_version: String,
    pub model_size: String,
    pub quantization: String,
    pub model_path: String,
    pub glossary: Vec<GlossaryEntry>,
    pub hotkeys: HotkeyConfig,
    pub show_floating_button: bool,
    pub autostart: bool,
    pub start_in_tray: bool,
    pub triple_copy_interval_ms: u64,
    /// How many quick Ctrl+C presses trigger the "copy → open DeepM" action (2 or 3).
    #[serde(default = "default_triple_copy_count")]
    pub triple_copy_count: u32,
    /// Executable names (e.g. "mobaxterm.exe") where the floating button and
    /// global hotkeys are suppressed.
    #[serde(default)]
    pub floating_exclusions: Vec<String>,
    /// OCR backend: "rapidocr" (default) | "tesseract".
    #[serde(default = "default_ocr_engine")]
    pub ocr_engine: String,
    /// Image preprocessing for OCR: "original" | "resize" | "grayscale" | "resize_grayscale".
    #[serde(default = "default_ocr_preprocess")]
    pub ocr_preprocess: String,
    /// Tesseract language data set: "standard" | "fast".
    #[serde(default = "default_tesseract_data")]
    pub tesseract_data: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_schema_version() -> u32 {
    1
}

fn default_triple_copy_count() -> u32 {
    3
}

fn default_model_version() -> String {
    "HY-MT1.5".to_string()
}

fn default_ocr_engine() -> String {
    "rapidocr".to_string()
}

fn default_ocr_preprocess() -> String {
    "auto".to_string()
}

fn default_tesseract_data() -> String {
    "standard".to_string()
}

fn default_locale() -> String {
    "en".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        let model_path = default_model_path();
        Self {
            default_source_lang: "auto".to_string(),
            default_target_lang: "auto".to_string(),
            use_gpu: true,
            model_version: "HY-MT1.5".to_string(),
            model_size: "1.8B".to_string(),
            quantization: "Q4_K_M".to_string(),
            model_path,
            glossary: Vec::new(),
            hotkeys: HotkeyConfig::default(),
            show_floating_button: true,
            autostart: false,
            start_in_tray: false,
            triple_copy_interval_ms: 500,
            triple_copy_count: 3,
            floating_exclusions: Vec::new(),
            ocr_engine: "rapidocr".to_string(),
            ocr_preprocess: "auto".to_string(),
            tesseract_data: "standard".to_string(),
            locale: "en".to_string(),
            schema_version: CURRENT_SCHEMA,
        }
    }
}

const CURRENT_SCHEMA: u32 = 4;

pub fn default_model_path() -> String {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeepM")
        .join("models")
        .to_string_lossy()
        .to_string()
}

pub fn config_path() -> PathBuf {
    dirs::config_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeepM")
        .join("settings.toml")
}

pub fn load_settings() -> Result<AppSettings> {
    let path = config_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let mut settings: AppSettings = toml::from_str(&content)?;
    migrate(&mut settings);
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(settings)?;
    std::fs::write(&path, content)?;
    Ok(())
}

fn migrate(settings: &mut AppSettings) {
    // v3: the default translation direction is now fully automatic
    // ("Auto (EN↔RU)"). One-time adopt it for any pre-v3 config. Users who
    // deliberately pick + save a fixed target afterwards get schema_version 3
    // persisted, so this never overrides their choice again.
    if settings.schema_version < 3 {
        settings.default_source_lang = "auto".to_string();
        settings.default_target_lang = "auto".to_string();
    }
    // v4: Windows OCR was removed; RapidOCR is the default engine now.
    if settings.schema_version < 4 && settings.ocr_engine == "windows" {
        settings.ocr_engine = "rapidocr".to_string();
    }
    settings.schema_version = CURRENT_SCHEMA;
}
