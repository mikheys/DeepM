use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;

const HF_BASE_URL: &str = "https://huggingface.co";

/// Model families we offer for download. The newer Hy-MT2 supports more modes.
pub const VERSIONS: &[&str] = &["HY-MT1.5", "Hy-MT2"];
pub const SIZES: &[&str] = &["1.8B", "7B"];
pub const QUANTS: &[&str] = &["Q4_K_M", "Q6_K", "Q8_0"];

/// HuggingFace repo for a (version, size), e.g. "tencent/Hy-MT2-7B-GGUF".
fn repo_for(version: &str, size: &str) -> String {
    format!("tencent/{}-{}-GGUF", version, size)
}

/// Lower-cased token that uniquely identifies a version inside a filename.
/// "HY-MT1.5" => "mt1.5", "Hy-MT2" => "mt2" (the two never collide).
fn version_token(version: &str) -> String {
    let v = version.to_lowercase();
    if v.contains("mt2") { "mt2".into() } else { "mt1.5".into() }
}

/// Does a gguf file belong to (version, size, quant)? Case-insensitive — robust
/// to Tencent's inconsistent filename casing (Hy-MT2 vs HY-MT2).
fn file_matches(filename: &str, version: &str, size: &str, quant: &str) -> bool {
    let f = filename.to_lowercase();
    f.ends_with(".gguf")
        && f.contains(&version_token(version))
        && f.contains(&size.to_lowercase())
        && f.contains(&quant.to_lowercase())
}

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
    pub version: String,
    pub size: String,
    pub quantization: String,
}

/// Live download progress, queryable so the UI can resume showing it after the
/// model-manager tab is left and reopened (the download itself keeps running).
#[derive(Debug, Clone, Serialize)]
pub struct DownloadState {
    pub version: String,
    pub size: String,
    pub quantization: String,
    pub progress: f64,
    pub speed_mbps: f64,
}

pub struct ModelManager {
    pub status: Arc<Mutex<ModelStatus>>,
    pub current_spec: Arc<Mutex<Option<ModelSpec>>>,
    cancel_flag: Arc<Mutex<bool>>,
    download_state: Arc<StdMutex<Option<DownloadState>>>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ModelStatus::NotDownloaded)),
            current_spec: Arc::new(Mutex::new(None)),
            cancel_flag: Arc::new(Mutex::new(false)),
            download_state: Arc::new(StdMutex::new(None)),
        }
    }

    pub fn get_download_state(&self) -> Option<DownloadState> {
        self.download_state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_download_state(&self, version: &str, size: &str, quant: &str, progress: f64, speed_mbps: f64) {
        *self.download_state.lock().unwrap_or_else(|e| e.into_inner()) = Some(DownloadState {
            version: version.to_string(),
            size: size.to_string(),
            quantization: quant.to_string(),
            progress,
            speed_mbps,
        });
    }

    pub fn clear_download_state(&self) {
        *self.download_state.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    pub async fn get_status(&self) -> ModelStatus {
        self.status.lock().await.clone()
    }

    /// Finds the local gguf file for a (version, size, quant), if downloaded.
    pub fn local_file(&self, model_dir: &str, version: &str, size: &str, quant: &str) -> Option<PathBuf> {
        let rd = std::fs::read_dir(Path::new(model_dir)).ok()?;
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if file_matches(&name, version, size, quant) {
                return Some(entry.path());
            }
        }
        None
    }

    /// If the model file exists, mark it Ready and remember it as the current spec.
    pub async fn probe(&self, model_path: &str, version: &str, size: &str, quant: &str) -> bool {
        if let Some(path) = self.local_file(model_path, version, size, quant) {
            *self.status.lock().await =
                ModelStatus::Ready { path: path.to_string_lossy().to_string() };
            *self.current_spec.lock().await = Some(ModelSpec {
                version: version.to_string(),
                size: size.to_string(),
                quantization: quant.to_string(),
            });
            return true;
        }
        false
    }

    pub async fn cancel(&self) {
        *self.cancel_flag.lock().await = true;
    }

    /// (version, size, quant) for every downloaded gguf we recognise.
    pub fn list_downloaded(&self, model_dir: &str) -> Vec<(String, String, String)> {
        let mut result = Vec::new();
        for version in VERSIONS {
            for size in SIZES {
                for quant in QUANTS {
                    if self.local_file(model_dir, version, size, quant).is_some() {
                        result.push((version.to_string(), size.to_string(), quant.to_string()));
                    }
                }
            }
        }
        result
    }

    pub fn delete_model_file(&self, model_dir: &str, version: &str, size: &str, quant: &str) -> Result<()> {
        if let Some(path) = self.local_file(model_dir, version, size, quant) {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Resolve the EXACT remote gguf filename for a quant via the HF API — robust
    /// to inconsistent casing instead of guessing the name.
    async fn resolve_remote_filename(
        client: &reqwest::Client,
        repo: &str,
        size: &str,
        quant: &str,
    ) -> Result<String> {
        let url = format!("{HF_BASE_URL}/api/models/{repo}");
        let json: serde_json::Value = client
            .get(&url)
            .header("User-Agent", "DeepM")
            .send()
            .await?
            .json()
            .await?;
        let siblings = json
            .get("siblings")
            .and_then(|s| s.as_array())
            .ok_or_else(|| anyhow!("HuggingFace repo {repo} has no file list"))?;

        let q = quant.to_lowercase();
        let sz = size.to_lowercase();
        for s in siblings {
            if let Some(name) = s.get("rfilename").and_then(|r| r.as_str()) {
                let n = name.to_lowercase();
                if n.ends_with(".gguf") && n.contains(&q) && n.contains(&sz) {
                    return Ok(name.to_string());
                }
            }
        }
        Err(anyhow!("No {quant} GGUF found in {repo}"))
    }

    pub async fn download(
        &self,
        model_dir: &str,
        version: &str,
        size: &str,
        quant: &str,
        progress_cb: impl Fn(f64, f64) + Send + 'static,
    ) -> Result<PathBuf> {
        *self.cancel_flag.lock().await = false;

        let dir = PathBuf::from(model_dir);
        tokio::fs::create_dir_all(&dir).await?;

        let client = reqwest::Client::new();
        let repo = repo_for(version, size);
        let filename = Self::resolve_remote_filename(&client, &repo, size, quant).await?;
        let dest = dir.join(&filename);
        let url = format!("{HF_BASE_URL}/{repo}/resolve/main/{filename}");

        log::info!("Downloading {url} -> {}", dest.display());

        let existing_bytes = if dest.exists() {
            tokio::fs::metadata(&dest).await?.len()
        } else {
            0
        };

        let mut req = client.get(&url).header("User-Agent", "DeepM");
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
