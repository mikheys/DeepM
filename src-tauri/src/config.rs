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
            translate_replace: "Ctrl+Shift+T".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_source_lang: String,
    pub default_target_lang: String,
    pub use_gpu: bool,
    pub model_size: String,
    pub quantization: String,
    pub model_path: String,
    pub glossary: Vec<GlossaryEntry>,
    pub hotkeys: HotkeyConfig,
    pub show_floating_button: bool,
    pub autostart: bool,
    pub start_in_tray: bool,
    pub triple_copy_interval_ms: u64,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_schema_version() -> u32 {
    1
}

fn default_locale() -> String {
    "en".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        let model_path = default_model_path();
        Self {
            default_source_lang: "auto".to_string(),
            default_target_lang: "en".to_string(),
            use_gpu: true,
            model_size: "1.8B".to_string(),
            quantization: "Q4_K_M".to_string(),
            model_path,
            glossary: Vec::new(),
            hotkeys: HotkeyConfig::default(),
            show_floating_button: true,
            autostart: false,
            start_in_tray: false,
            triple_copy_interval_ms: 500,
            locale: "en".to_string(),
            schema_version: 1,
        }
    }
}

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
    // Future schema migrations go here.
    settings.schema_version = 1;
}
