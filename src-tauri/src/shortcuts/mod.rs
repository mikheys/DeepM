//! Hotkey configuration model. The actual registration (SetWindowsHookEx / rdev)
//! lives in os_integration and is wired up in Stage 2.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub triple_copy: String,
    pub translate_replace: String,
    pub triple_copy_interval_ms: u64,
    pub triple_copy_count: u32,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            triple_copy: "Ctrl+C+C+C".to_string(),
            translate_replace: "Ctrl+Alt+T".to_string(),
            triple_copy_interval_ms: 500,
            triple_copy_count: 3,
        }
    }
}

impl ShortcutConfig {
    pub fn from_settings(settings: &crate::config::AppSettings) -> Self {
        Self {
            triple_copy: settings.hotkeys.triple_copy.clone(),
            translate_replace: settings.hotkeys.translate_replace.clone(),
            triple_copy_interval_ms: settings.triple_copy_interval_ms,
            triple_copy_count: settings.triple_copy_count.clamp(2, 5),
        }
    }
}

/// Validates that two shortcuts don't conflict.
pub fn check_conflict(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}
