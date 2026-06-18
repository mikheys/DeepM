use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use super::prompts::build_prompt;
use super::languages::hy_mt_language_name;

const LLAMA_SERVER_PORT: u16 = 28473;
const SERVER_STARTUP_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

pub struct TranslationRequest {
    pub source_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub context: Option<String>,
    pub glossary: Vec<(String, String)>,
    pub formatted: bool,
}

pub struct TranslationResult {
    pub translated_text: String,
    pub detected_lang: Option<String>,
}

pub struct LlamaServerProcess {
    child: Child,
}

impl Drop for LlamaServerProcess {
    fn drop(&mut self) {
        // Kill AND reap the process so its TCP port (28473) is released before a
        // new server is started during a restart. Without wait(), the old process
        // may still hold the port when the new one tries to bind it.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct TranslationEngine {
    server_process: Arc<Mutex<Option<LlamaServerProcess>>>,
    client: reqwest::Client,
    model_path: Arc<Mutex<Option<PathBuf>>>,
    use_gpu: bool,
}

impl TranslationEngine {
    pub fn new(use_gpu: bool) -> Self {
        Self {
            server_process: Arc::new(Mutex::new(None)),
            client: reqwest::Client::new(),
            model_path: Arc::new(Mutex::new(None)),
            use_gpu,
        }
    }

    pub async fn start(&self, model_path: PathBuf) -> Result<()> {
        let llama_server = llama_server_binary_path()?;

        log::info!("Starting llama-server: {}", llama_server.display());
        log::info!("Model: {}", model_path.display());

        // Spawn the server inside a scoped lock, then RELEASE the lock before
        // waiting for readiness. wait_for_server_ready() locks server_process
        // itself (to poll the child for early exit); holding the lock across that
        // call would deadlock (tokio::Mutex is not reentrant).
        let stderr_ref = {
            let mut proc = self.server_process.lock().await;

            // Kill+reap any existing server (Drop waits on it, freeing the port).
            *proc = None;

            let mut cmd = Command::new(&llama_server);
            cmd.arg("--model").arg(&model_path)
               .arg("--port").arg(LLAMA_SERVER_PORT.to_string())
               .arg("--ctx-size").arg("4096")
               .arg("--threads").arg(num_cpus().to_string())
               .stdout(Stdio::null())
               .stderr(Stdio::piped());

            if self.use_gpu {
                cmd.arg("--n-gpu-layers").arg("99");
            }

            let mut child = cmd.spawn()
                .map_err(|e| anyhow!(
                    "Failed to launch llama-server.exe: {e}.\n\
                    Make sure llama-server.exe AND all companion DLLs (cublas64_12.dll, \
                    cudart64_12.dll, ggml-cuda.dll, etc.) are in src-tauri\\binaries\\."
                ))?;

            // Capture stderr on a sync thread (std::sync::Mutex — safe from async context too)
            let stderr_output: Arc<StdMutex<String>> = Arc::new(StdMutex::new(String::new()));
            if let Some(stderr) = child.stderr.take() {
                let buf = Arc::clone(&stderr_output);
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader};
                    let reader = BufReader::new(stderr);
                    let mut out = String::new();
                    for line in reader.lines().flatten() {
                        log::debug!("[llama-server] {line}");
                        out.push_str(&line);
                        out.push('\n');
                        if out.len() > 4096 { break; }
                    }
                    if let Ok(mut g) = buf.lock() { *g = out; }
                });
            }

            *proc = Some(LlamaServerProcess { child });
            stderr_output
            // proc lock guard dropped here
        };

        {
            let mut mp = self.model_path.lock().await;
            *mp = Some(model_path);
        }

        // Wait for server to become ready (lock is free now — no deadlock)
        self.wait_for_server_ready(stderr_ref).await?;
        Ok(())
    }

    async fn wait_for_server_ready(&self, stderr_buf: Arc<StdMutex<String>>) -> Result<()> {
        let url = format!("http://127.0.0.1:{}/health", LLAMA_SERVER_PORT);
        let deadline = std::time::Instant::now() + Duration::from_secs(SERVER_STARTUP_TIMEOUT_SECS);
        loop {
            if std::time::Instant::now() > deadline {
                let captured = stderr_buf.lock().map(|g| g.clone()).unwrap_or_default();
                let hint = if captured.is_empty() {
                    "Tip: make sure all companion DLLs (cublas64_12.dll, cudart64_12.dll, \
                     ggml-cuda.dll…) are in the same folder as llama-server.exe.".to_string()
                } else {
                    format!("llama-server output:\n{}", captured.trim())
                };
                return Err(anyhow!(
                    "llama-server did not respond within {SERVER_STARTUP_TIMEOUT_SECS}s.\n{hint}"
                ));
            }

            // Check if the child process exited early (crash/missing DLL)
            {
                let mut proc = self.server_process.lock().await;
                if let Some(p) = proc.as_mut() {
                    match p.child.try_wait() {
                        Ok(Some(status)) => {
                            let captured = stderr_buf.lock().map(|g| g.clone()).unwrap_or_default();
                            let detail = if captured.is_empty() {
                                format!("exit status: {status}")
                            } else {
                                format!("exit status: {status}\nOutput:\n{}", captured.trim())
                            };
                            return Err(anyhow!(
                                "llama-server crashed immediately ({detail}).\n\
                                Likely cause: missing CUDA DLLs next to llama-server.exe."
                            ));
                        }
                        Ok(None) => {} // still running
                        Err(_) => {}
                    }
                }
            }

            match self.client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                _ => sleep(Duration::from_millis(500)).await,
            }
        }
    }

    pub async fn is_running(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/health", LLAMA_SERVER_PORT);
        self.client.get(&url).send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub async fn translate(&self, req: TranslationRequest) -> Result<TranslationResult> {
        if !self.is_running().await {
            return Err(anyhow!("Translation engine is not running. Please ensure a model is downloaded and loaded."));
        }

        let glossary_refs: Vec<(&str, &str)> = req.glossary
            .iter()
            .map(|(s, t)| (s.as_str(), t.as_str()))
            .collect();

        let glossary_opt = if glossary_refs.is_empty() {
            None
        } else {
            Some(glossary_refs.as_slice())
        };

        let prompt = build_prompt(
            &req.source_text,
            &req.source_lang,
            &req.target_lang,
            glossary_opt,
            req.context.as_deref(),
            req.formatted,
        );

        let chat_req = ChatRequest {
            model: "local".to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                }
            ],
            temperature: 0.1,
            max_tokens: 2048,
            stream: false,
        };

        let url = format!("http://127.0.0.1:{}/v1/chat/completions", LLAMA_SERVER_PORT);
        let response = self.client
            .post(&url)
            .json(&chat_req)
            .timeout(Duration::from_secs(120))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("llama-server error {status}: {body}"));
        }

        let chat_resp: ChatResponse = response.json().await?;
        let translated = chat_resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content.trim().to_string())
            .ok_or_else(|| anyhow!("Empty response from model"))?;

        // Strip <target>...</target> wrapper if formatted mode returned it
        let translated = if translated.starts_with("<target>") && translated.ends_with("</target>") {
            translated[8..translated.len() - 9].to_string()
        } else {
            translated
        };

        Ok(TranslationResult {
            translated_text: translated,
            detected_lang: None,
        })
    }

    pub async fn stop(&self) {
        let mut proc = self.server_process.lock().await;
        *proc = None;
    }
}

fn llama_server_binary_path() -> Result<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // CARGO_MANIFEST_DIR is set during `cargo tauri dev` — use project binaries folder
    let project_binaries = option_env!("CARGO_MANIFEST_DIR")
        .map(|d| PathBuf::from(d).join("binaries").join("llama-server.exe"));

    let mut candidates: Vec<PathBuf> = vec![
        exe_dir.join("llama-server.exe"),
        exe_dir.join("resources").join("llama-server.exe"),
        PathBuf::from("llama-server.exe"),
    ];

    if let Some(p) = project_binaries {
        candidates.insert(0, p);
    }

    for candidate in &candidates {
        if candidate.exists() {
            log::info!("Found llama-server at: {}", candidate.display());
            return Ok(candidate.clone());
        }
    }

    // Fall back to PATH lookup
    log::warn!("llama-server not found locally; falling back to PATH");
    Ok(PathBuf::from("llama-server"))
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8)
}
