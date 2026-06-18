use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use sha2::Digest;

use crate::config::default_model_path;

/// HuggingFace repo IDs for each model variant.
const HF_REPO_1_8B: &str = "tencent/HY-MT1.5-1.8B-GGUF";
const HF_REPO_7B: &str = "tencent/HY-MT1.5-7B-GGUF";
const HF_BASE_URL: &str = "https://huggingface.co";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelStatus {
    NotDownloaded,
    Downloading { progress: f64, speed_mbps: f64 },
    Ready { path: String },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub size: String,
    pub quantization: String,
}

impl ModelSpec {
    pub fn filename(&self) -> String {
        // Exact filenames as they appear in tencent/HY-MT1.5-x.xB-GGUF on HuggingFace
        format!("HY-MT1.5-{}-{}.gguf", self.size, self.quantization)
    }

    pub fn hf_repo(&self) -> &'static str {
        if self.size == "7B" { HF_REPO_7B } else { HF_REPO_1_8B }
    }

    pub fn download_url(&self) -> String {
        format!(
            "{}/{}/resolve/main/{}",
            HF_BASE_URL,
            self.hf_repo(),
            self.filename()
        )
    }
}

pub struct ModelManager {
    pub status: Arc<Mutex<ModelStatus>>,
    pub current_spec: Arc<Mutex<Option<ModelSpec>>>,
    cancel_flag: Arc<Mutex<bool>>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ModelStatus::NotDownloaded)),
            current_spec: Arc::new(Mutex::new(None)),
            cancel_flag: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn get_status(&self) -> ModelStatus {
        self.status.lock().await.clone()
    }

    /// Check if a model file already exists for this spec and mark as ready if so.
    pub async fn probe(&self, model_path: &str, size: &str, quantization: &str) -> bool {
        let spec = ModelSpec {
            size: size.to_string(),
            quantization: quantization.to_string(),
        };
        let path = PathBuf::from(model_path).join(spec.filename());
        if path.exists() {
            let mut status = self.status.lock().await;
            *status = ModelStatus::Ready { path: path.to_string_lossy().to_string() };
            let mut cs = self.current_spec.lock().await;
            *cs = Some(spec);
            return true;
        }
        false
    }

    pub fn model_path_for(&self, model_dir: &str, spec: &ModelSpec) -> PathBuf {
        PathBuf::from(model_dir).join(spec.filename())
    }

    pub async fn cancel(&self) {
        let mut flag = self.cancel_flag.lock().await;
        *flag = true;
    }

    /// Returns list of (size, quant) pairs for all downloaded GGUF files.
    pub fn list_downloaded(&self, model_dir: &str) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for size in &["1.8B", "7B"] {
            for quant in &["Q4_K_M", "Q6_K", "Q8_0"] {
                let spec = ModelSpec { size: size.to_string(), quantization: quant.to_string() };
                if std::path::Path::new(model_dir).join(spec.filename()).exists() {
                    result.push((size.to_string(), quant.to_string()));
                }
            }
        }
        result
    }

    /// Deletes a model file from disk.
    pub fn delete_model_file(&self, model_dir: &str, size: &str, quantization: &str) -> Result<()> {
        let spec = ModelSpec { size: size.to_string(), quantization: quantization.to_string() };
        let path = std::path::PathBuf::from(model_dir).join(spec.filename());
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub async fn download(
        &self,
        model_dir: &str,
        size: &str,
        quantization: &str,
        progress_cb: impl Fn(f64, f64) + Send + 'static,
    ) -> Result<PathBuf> {
        // Reset cancel flag
        {
            let mut flag = self.cancel_flag.lock().await;
            *flag = false;
        }

        let spec = ModelSpec {
            size: size.to_string(),
            quantization: quantization.to_string(),
        };

        let dir = PathBuf::from(model_dir);
        tokio::fs::create_dir_all(&dir).await?;

        let dest = dir.join(spec.filename());
        let url = spec.download_url();

        log::info!("Downloading model from {url} to {}", dest.display());

        let client = reqwest::Client::new();

        // Support resume: check existing partial download
        let existing_bytes = if dest.exists() {
            tokio::fs::metadata(&dest).await?.len()
        } else {
            0
        };

        let mut req = client.get(&url);
        if existing_bytes > 0 {
            req = req.header("Range", format!("bytes={}-", existing_bytes));
        }

        let response = req.send().await?;

        let total_size = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
            + existing_bytes;

        let status_code = response.status();
        if !status_code.is_success() && status_code.as_u16() != 206 {
            return Err(anyhow!("HTTP error {status_code} downloading model"));
        }

        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(existing_bytes > 0)
            .write(true)
            .open(&dest)
            .await?;

        let mut downloaded = existing_bytes;
        let mut stream = response.bytes_stream();
        let start = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            // Check cancel
            if *self.cancel_flag.lock().await {
                return Err(anyhow!("Download cancelled"));
            }

            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if total_size > 0 {
                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                let elapsed = start.elapsed().as_secs_f64();
                let speed_mbps = if elapsed > 0.0 {
                    (downloaded - existing_bytes) as f64 / elapsed / 1_000_000.0
                } else {
                    0.0
                };
                progress_cb(progress, speed_mbps);
            }
        }

        file.flush().await?;
        Ok(dest)
    }
}
