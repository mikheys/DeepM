//! Screenshot / image OCR via a Tesseract CLI bundled inside the app (rus+eng).
//!
//! Fixed configuration (decided by benchmarking): tessdata "standard", PSM 6,
//! OEM 1 (LSTM), with resize+grayscale preprocessing. No engine choice, no
//! knobs — it's a translator, not an OCR workbench.
//!
//! Pipeline: image → preprocess → Tesseract (raw text). Text normalization
//! happens one level up (lib.rs). `ocr_test_all` is a hidden diagnostic that
//! sweeps the installed data sets × PSM.

use anyhow::{anyhow, Result};
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

// ── Resource directory (set once at startup) ──────────────────────────────────

#[cfg(target_os = "windows")]
static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Called once from setup() with Tauri's resolved resource dir, so the bundled
/// Tesseract can be found next to the installed exe.
#[cfg(target_os = "windows")]
pub fn set_resource_dir(dir: PathBuf) {
    let _ = RESOURCE_DIR.set(dir);
}

#[cfg(target_os = "windows")]
fn resource_dir() -> Option<PathBuf> {
    RESOURCE_DIR.get().cloned()
}

/// Directory the executable lives in. In a packaged build the bundled
/// `tesseract/` folder sits next to the exe (same as `engine/`), which is the
/// reliable production path — `resource_dir()` can point elsewhere.
#[cfg(target_os = "windows")]
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

// ── Preprocessing ─────────────────────────────────────────────────────────────

/// Upscales small images (Tesseract goes blind on small text) and grayscales.
#[cfg(target_os = "windows")]
fn preprocess(img: image::DynamicImage) -> image::DynamicImage {
    use image::GenericImageView;
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    let scale = if longest < 1000 { 3 } else if longest < 2200 { 2 } else { 1 };
    let img = if scale > 1 {
        img.resize(w * scale, h * scale, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    img.grayscale()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// True if the bundled Tesseract is usable right now.
#[cfg(target_os = "windows")]
pub fn engine_status() -> bool {
    tesseract::available()
}

/// OCR a screenshot already on the clipboard.
#[cfg(target_os = "windows")]
pub fn recognize_clipboard() -> Result<String> {
    tesseract::recognize(preprocess(clipboard_image()?), "standard", 6)
}

/// OCR an image file from disk.
#[cfg(target_os = "windows")]
pub fn recognize_file(path: &str) -> Result<String> {
    let img = image::open(path).map_err(|e| anyhow!("open image: {e}"))?;
    tesseract::recognize(preprocess(img), "standard", 6)
}

#[cfg(target_os = "windows")]
fn clipboard_image() -> Result<image::DynamicImage> {
    let img = arboard::Clipboard::new()
        .map_err(|e| anyhow!("clipboard: {e}"))?
        .get_image()
        .map_err(|_| anyhow!("no_image"))?;
    let buf = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
        .ok_or_else(|| anyhow!("bad clipboard image"))?;
    Ok(image::DynamicImage::ImageRgba8(buf))
}

// ── Test Mode (hidden diagnostic) ─────────────────────────────────────────────

/// One row of the Test Mode comparison.
#[derive(serde::Serialize)]
pub struct OcrTestResult {
    pub engine: String,
    pub model: String,
    pub preprocess: String,
    pub ms: u128,
    pub text: String,
    pub error: Option<String>,
}

/// Sweeps every installed data set × PSM {3,6,11} on one image (diagnostic).
#[cfg(target_os = "windows")]
pub fn ocr_test_all(path: &str) -> Vec<OcrTestResult> {
    let mut out = Vec::new();
    for variant in ["standard", "fast", "best"] {
        if !tesseract::has_data(variant) {
            continue;
        }
        for psm in [3u32, 6, 11] {
            let model = format!("Tesseract {variant} psm{psm}");
            let img = match image::open(path) {
                Ok(i) => i,
                Err(e) => {
                    out.push(OcrTestResult {
                        engine: "tesseract".into(),
                        model,
                        preprocess: "resize+grayscale".into(),
                        ms: 0,
                        text: String::new(),
                        error: Some(format!("open image: {e}")),
                    });
                    continue;
                }
            };
            let started = std::time::Instant::now();
            let result = tesseract::recognize(preprocess(img), variant, psm);
            let ms = started.elapsed().as_millis();
            let (text, error) = match result {
                Ok(t) => (t, None),
                Err(e) => (String::new(), Some(e.to_string())),
            };
            out.push(OcrTestResult {
                engine: "tesseract".into(),
                model,
                preprocess: "resize+grayscale".into(),
                ms,
                text,
                error,
            });
        }
    }
    out
}

// ── Tesseract CLI backend (bundled inside the app) ────────────────────────────
#[cfg(target_os = "windows")]
mod tesseract {
    use super::{exe_dir, resource_dir};
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Appends a line to %LOCALAPPDATA%/DeepM/ocr-debug.log (visible in release,
    /// unlike eprintln). Helps diagnose path/DLL issues in installed builds.
    fn dbg_log(msg: &str) {
        if let Some(d) = dirs::data_local_dir() {
            let dir = d.join("DeepM");
            let _ = std::fs::create_dir_all(&dir);
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("ocr-debug.log"))
            {
                let _ = writeln!(f, "{msg}");
            }
        }
    }

    /// Locate tesseract.exe: bundled copy first (next to the installed exe under
    /// `tesseract/`), then a dev path, then PATH / Program Files as a fallback.
    fn exe() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(PathBuf::from(d).join("tesseract").join("tesseract.exe"));
        }
        // Production: bundled next to the exe (like engine/).
        if let Some(d) = exe_dir() {
            candidates.push(d.join("tesseract").join("tesseract.exe"));
        }
        if let Some(r) = resource_dir() {
            candidates.push(r.join("tesseract").join("tesseract.exe"));
        }
        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }
        if Command::new("tesseract").arg("--version").no_window().output().is_ok() {
            return Some(PathBuf::from("tesseract"));
        }
        for p in [
            r"C:\Program Files\Tesseract-OCR\tesseract.exe",
            r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe",
        ] {
            let pb = PathBuf::from(p);
            if pb.exists() {
                return Some(pb);
            }
        }
        None
    }

    /// True if the given data set ("standard"|"fast"|"best") is installed.
    pub fn has_data(variant: &str) -> bool {
        tessdata_dir(variant).is_some()
    }

    /// Bundled tessdata dir for the chosen variant, if present (non-empty files).
    fn tessdata_dir(variant: &str) -> Option<PathBuf> {
        let sub = match variant {
            "fast" => "tessdata-fast",
            "best" => "tessdata-best",
            _ => "tessdata-standard",
        };
        let mut candidates: Vec<PathBuf> = Vec::new();
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(PathBuf::from(d).join("tesseract").join(sub));
        }
        if let Some(d) = exe_dir() {
            candidates.push(d.join("tesseract").join(sub));
        }
        if let Some(r) = resource_dir() {
            candidates.push(r.join("tesseract").join(sub));
        }
        candidates.into_iter().find(|d| {
            d.join("eng.traineddata").metadata().map(|m| m.len() > 0).unwrap_or(false)
        })
    }

    pub fn available() -> bool {
        exe().is_some()
    }

    /// Languages to pass: prefer rus+eng, falling back to whatever is installed.
    fn langs(exe: &PathBuf, tessdata: Option<&PathBuf>) -> String {
        let mut cmd = Command::new(exe);
        cmd.arg("--list-langs");
        if let Some(d) = tessdata {
            cmd.args(["--tessdata-dir", &d.to_string_lossy()]);
        }
        let out = cmd.no_window().output();
        let installed: Vec<String> = out
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        let has = |l: &str| installed.iter().any(|x| x == l);
        match (has("rus"), has("eng")) {
            (true, true) => "rus+eng".into(),
            (true, false) => "rus".into(),
            (false, true) => "eng".into(),
            _ => "rus+eng".into(),
        }
    }

    /// Context line written to the debug log only when recognition fails.
    fn fail_context(variant: &str, psm: u32) -> String {
        format!(
            "--- OCR FAIL (variant={variant}, psm={psm}) exe_dir={:?} resource_dir={:?} ---",
            exe_dir().map(|d| d.display().to_string()),
            resource_dir().map(|d| d.display().to_string()),
        )
    }

    pub fn recognize(img: image::DynamicImage, variant: &str, psm: u32) -> Result<String> {
        let exe = match exe() {
            Some(e) => e,
            None => {
                dbg_log(&fail_context(variant, psm));
                dbg_log("tesseract.exe NOT FOUND in any candidate");
                return Err(anyhow!("tesseract_not_installed"));
            }
        };
        let tessdata = tessdata_dir(variant);

        let tmp = std::env::temp_dir().join(format!("deepm_ocr_{}.png", std::process::id()));
        img.save(&tmp).map_err(|e| anyhow!("save temp: {e}"))?;

        let langs = langs(&exe, tessdata.as_ref());
        let psm = if (3..=13).contains(&psm) { psm } else { 6 };
        let psm_s = psm.to_string();
        let mut cmd = Command::new(&exe);
        // --oem 1 = LSTM engine only. Dictionaries off so mixed RU/EN technical
        // tokens (rec-модели) read literally; keep inter-word spaces.
        cmd.arg(&tmp).arg("stdout").args(["-l", &langs, "--oem", "1", "--psm", &psm_s]);
        cmd.args(["-c", "preserve_interword_spaces=1"]);
        cmd.args(["-c", "load_system_dawg=0"]);
        cmd.args(["-c", "load_freq_dawg=0"]);
        if let Some(d) = &tessdata {
            cmd.args(["--tessdata-dir", &d.to_string_lossy()]);
        }
        let output = cmd.no_window().output();
        let _ = std::fs::remove_file(&tmp);

        let output = output.map_err(|e| {
            dbg_log(&fail_context(variant, psm));
            dbg_log(&format!("SPAWN FAILED (exe={}): {e}", exe.display()));
            anyhow!("tesseract run: {e}")
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            dbg_log(&fail_context(variant, psm));
            dbg_log(&format!(
                "EXIT FAIL code={:?} tessdata={:?} stderr={}",
                output.status.code(),
                tessdata.as_ref().map(|d| d.display().to_string()),
                stderr.trim()
            ));
            return Err(anyhow!("tesseract error: {}", stderr.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Small extension so the CLI calls don't flash a console window.
    trait NoWindow {
        fn no_window(&mut self) -> &mut Self;
    }
    impl NoWindow for Command {
        fn no_window(&mut self) -> &mut Self {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW)
        }
    }
}

// ── Non-Windows stubs ─────────────────────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
pub fn engine_status() -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn recognize_clipboard() -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_file(_path: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
#[derive(serde::Serialize)]
pub struct OcrTestResult {
    pub engine: String,
    pub model: String,
    pub preprocess: String,
    pub ms: u128,
    pub text: String,
    pub error: Option<String>,
}
#[cfg(not(target_os = "windows"))]
pub fn ocr_test_all(_path: &str) -> Vec<OcrTestResult> { Vec::new() }
