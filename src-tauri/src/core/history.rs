use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: u64,
    pub source_lang: String,
    pub target_lang: String,
    pub source_text: String,
    pub translated_text: String,
}

const MAX_HISTORY: usize = 500;

pub struct TranslationHistory {
    entries: Vec<HistoryEntry>,
    path: PathBuf,
}

impl TranslationHistory {
    pub fn load(path: PathBuf) -> Self {
        let entries = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self { entries, path }
    }

    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
        if self.entries.len() > MAX_HISTORY {
            self.entries.truncate(MAX_HISTORY);
        }
        let _ = self.persist();
    }

    pub fn all(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        let _ = self.persist();
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}
