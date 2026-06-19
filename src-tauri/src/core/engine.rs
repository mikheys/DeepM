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

// ── Windows job object ────────────────────────────────────────────────────────
//
// We put every llama-server child into a job object configured with
// KILL_ON_JOB_CLOSE. The job handle is owned by the engine for the whole app
// lifetime; when DeepM exits — even on a hard crash — the OS closes the handle,
// the job closes, and every llama-server in it is terminated. This stops the
// orphaned servers that otherwise pile up in memory across restarts.
#[cfg(target_os = "windows")]
mod job {
    use core::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateJobObjectW(attrs: *const c_void, name: *const u16) -> *mut c_void;
        pub fn SetInformationJobObject(
            job: *mut c_void,
            class: i32,
            info: *const c_void,
            len: u32,
        ) -> i32;
        pub fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    }

    /// JobObjectExtendedLimitInformation
    pub const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
    pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

    #[repr(C)]
    #[derive(Default)]
    pub struct JobBasicLimitInformation {
        pub per_process_user_time_limit: i64,
        pub per_job_user_time_limit: i64,
        pub limit_flags: u32,
        pub minimum_working_set_size: usize,
        pub maximum_working_set_size: usize,
        pub active_process_limit: u32,
        pub affinity: usize,
        pub priority_class: u32,
        pub scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct IoCounters {
        pub read_operation_count: u64,
        pub write_operation_count: u64,
        pub other_operation_count: u64,
        pub read_transfer_count: u64,
        pub write_transfer_count: u64,
        pub other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct JobExtendedLimitInformation {
        pub basic_limit_information: JobBasicLimitInformation,
        pub io_info: IoCounters,
        pub process_memory_limit: usize,
        pub job_memory_limit: usize,
        pub peak_process_memory_used: usize,
        pub peak_job_memory_used: usize,
    }

    /// A job-object handle that is safe to move/share across threads.
    pub struct JobHandle(pub *mut c_void);
    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    /// Creates a job object that kills its processes when the handle closes.
    pub fn create_kill_on_close() -> Option<JobHandle> {
        unsafe {
            let h = CreateJobObjectW(core::ptr::null(), core::ptr::null());
            if h.is_null() {
                return None;
            }
            let mut info = JobExtendedLimitInformation::default();
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                &info as *const _ as *const c_void,
                core::mem::size_of::<JobExtendedLimitInformation>() as u32,
            );
            Some(JobHandle(h))
        }
    }

    /// Adds a process handle to the job.
    pub fn assign(job: &JobHandle, process: *mut c_void) {
        unsafe {
            AssignProcessToJobObject(job.0, process);
        }
    }
}

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
    /// Active model family ("HY-MT1.5" / "Hy-MT2") — selects the prompt set.
    pub version: String,
    /// standard | contextual | formatted | style | structured | delimiter
    pub mode: String,
    /// Free-text style for the "style" mode (Hy-MT2).
    pub style: Option<String>,
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
    use_gpu: std::sync::atomic::AtomicBool,
    #[cfg(target_os = "windows")]
    job: Option<job::JobHandle>,
}

impl TranslationEngine {
    pub fn new(use_gpu: bool) -> Self {
        Self {
            server_process: Arc::new(Mutex::new(None)),
            client: reqwest::Client::new(),
            model_path: Arc::new(Mutex::new(None)),
            use_gpu: std::sync::atomic::AtomicBool::new(use_gpu),
            #[cfg(target_os = "windows")]
            job: job::create_kill_on_close(),
        }
    }

    /// Updates the GPU preference; applied on the next engine (re)start.
    pub fn set_use_gpu(&self, value: bool) {
        self.use_gpu.store(value, std::sync::atomic::Ordering::Relaxed);
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

            // Don't pop up a console window for the sidecar (Windows).
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            if self.use_gpu.load(std::sync::atomic::Ordering::Relaxed) {
                cmd.arg("--n-gpu-layers").arg("99");
            }

            let child = cmd.spawn()
                .map_err(|e| anyhow!(
                    "Failed to launch llama-server.exe: {e}.\n\
                    Make sure llama-server.exe AND all companion DLLs (cublas64_12.dll, \
                    cudart64_12.dll, ggml-cuda.dll, etc.) are in src-tauri\\binaries\\."
                ))?;

            // Put the server in the kill-on-close job so it dies with DeepM.
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::io::AsRawHandle;
                if let Some(j) = self.job.as_ref() {
                    job::assign(j, child.as_raw_handle() as *mut _);
                }
            }

            let mut child = child;

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
            &req.version,
            &req.source_text,
            &req.source_lang,
            &req.target_lang,
            glossary_opt,
            req.context.as_deref(),
            &req.mode,
            req.style.as_deref(),
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

    // In a packaged build the engine ships next to the exe under `engine/`
    // (bundled as a Tauri resource). These exe-relative paths are the ONLY ones
    // used in a release build.
    let mut candidates: Vec<PathBuf> = vec![
        exe_dir.join("engine").join("llama-server.exe"),
        exe_dir.join("resources").join("engine").join("llama-server.exe"),
        exe_dir.join("llama-server.exe"),
        exe_dir.join("resources").join("llama-server.exe"),
        PathBuf::from("llama-server.exe"),
    ];

    // DEV ONLY: CARGO_MANIFEST_DIR is baked in at compile time, so this must be
    // gated behind debug_assertions — otherwise a release built on this machine
    // would hardcode an absolute path to the dev GPU `binaries/` folder and use
    // it instead of the bundled CPU engine (breaking the CPU build's behaviour
    // and its GPU auto-detection on the build machine).
    #[cfg(debug_assertions)]
    if let Some(dir) = option_env!("CARGO_MANIFEST_DIR") {
        // Dev: try GPU binaries first, then the staged CPU engine folder.
        candidates.insert(0, PathBuf::from(dir).join("engine").join("llama-server.exe"));
        candidates.insert(0, PathBuf::from(dir).join("binaries").join("llama-server.exe"));
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

/// Directory the engine (llama-server + backend DLLs) lives in.
fn engine_dir() -> Option<PathBuf> {
    llama_server_binary_path()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// True if a DLL of the given name can be loaded via the OS search path
/// (app dir, System32, PATH). Used to detect a system-wide CUDA toolkit /
/// the NVIDIA driver without bundling those files.
#[cfg(target_os = "windows")]
fn can_load(name: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut core::ffi::c_void;
        fn FreeLibrary(h: *mut core::ffi::c_void) -> i32;
    }
    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let h = LoadLibraryW(wide.as_ptr());
        if h.is_null() {
            false
        } else {
            FreeLibrary(h);
            true
        }
    }
}
#[cfg(not(target_os = "windows"))]
fn can_load(_name: &str) -> bool {
    false
}

/// True if an NVIDIA driver is installed (nvcuda.dll loads). Tells us whether
/// the machine even has a CUDA-capable GPU, regardless of the GPU pack.
pub fn nvidia_gpu_present() -> bool {
    can_load("nvcuda.dll")
}

/// True if GPU mode would actually work: the llama.cpp CUDA backend
/// (ggml-cuda.dll) is present next to the engine AND its CUDA runtime deps are
/// resolvable — either bundled next to it (full pack) or available from a
/// system CUDA Toolkit on PATH (slim pack). ggml-cuda.dll itself is part of
/// llama.cpp and is never provided by a system CUDA install, so it must be
/// shipped; only cublas/cublasLt/cudart can come from the system.
pub fn cuda_available() -> bool {
    let dir = match engine_dir() {
        Some(d) => d,
        None => return false,
    };
    if !dir.join("ggml-cuda.dll").exists() {
        return false;
    }
    // Deps bundled locally (full pack) OR a system CUDA 12.x toolkit on PATH.
    dir.join("cublasLt64_12.dll").exists() || can_load("cudart64_12.dll")
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8)
}
