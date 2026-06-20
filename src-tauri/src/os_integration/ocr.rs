//! Screenshot / image OCR with two selectable backends:
//! - "rapidocr"  : PP-OCRv5 via ONNX (oar-ocr). Default. Best local quality
//!                 for Russian; ships its default Cyrillic models bundled.
//! - "tesseract" : a Tesseract CLI bundled inside the app (rus+eng).
//!
//! Pipeline: image → preprocess → OCR (raw text). Text normalization happens
//! one level up (lib.rs) so Test Mode can show raw vs normalized.
//!
//! Everything here is Windows-only (Tesseract CLI + clipboard image); non-
//! Windows targets get stubs at the bottom.

use anyhow::{anyhow, Result};
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

// ── Preprocessing ─────────────────────────────────────────────────────────────

/// Image preprocessing applied before OCR. Exposed so the UI can A/B test which
/// works best for mixed RU/EN screenshots (grayscale can hurt colour-trained
/// PP-OCR models, so "resize" without grayscale is worth comparing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreprocessMode {
    /// Per-engine optimum: Tesseract → resize+grayscale, RapidOCR → resize.
    Auto,
    Original,
    Resize,
    Grayscale,
    ResizeGrayscale,
}

impl PreprocessMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "original" => PreprocessMode::Original,
            "resize" => PreprocessMode::Resize,
            "grayscale" => PreprocessMode::Grayscale,
            "resize_grayscale" => PreprocessMode::ResizeGrayscale,
            _ => PreprocessMode::Auto,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PreprocessMode::Auto => "auto",
            PreprocessMode::Original => "original",
            PreprocessMode::Resize => "resize",
            PreprocessMode::Grayscale => "grayscale",
            PreprocessMode::ResizeGrayscale => "resize+grayscale",
        }
    }
    /// Resolve "Auto" to the concrete mode each engine performs best with.
    pub fn resolve(self, engine: &str) -> PreprocessMode {
        match self {
            PreprocessMode::Auto => {
                if engine == "tesseract" {
                    PreprocessMode::ResizeGrayscale
                } else {
                    PreprocessMode::Resize
                }
            }
            m => m,
        }
    }
}

/// Upscales small images (OCR is tuned for ~300dpi) and/or grayscales them.
#[cfg(target_os = "windows")]
fn preprocess(img: image::DynamicImage, mode: PreprocessMode) -> image::DynamicImage {
    use image::GenericImageView;
    let do_resize = matches!(mode, PreprocessMode::Resize | PreprocessMode::ResizeGrayscale);
    let do_gray = matches!(mode, PreprocessMode::Grayscale | PreprocessMode::ResizeGrayscale);

    let img = if do_resize {
        let (w, h) = img.dimensions();
        let longest = w.max(h);
        let scale = if longest < 1000 { 3 } else if longest < 2200 { 2 } else { 1 };
        if scale > 1 {
            img.resize(w * scale, h * scale, image::imageops::FilterType::Lanczos3)
        } else {
            img
        }
    } else {
        img
    };

    if do_gray { img.grayscale() } else { img }
}

// ── Resource directory (set once at startup) ──────────────────────────────────

#[cfg(target_os = "windows")]
static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Called once from setup() with Tauri's resolved resource dir, so bundled
/// Tesseract / RapidOCR models can be found next to the installed exe.
#[cfg(target_os = "windows")]
pub fn set_resource_dir(dir: PathBuf) {
    let _ = RESOURCE_DIR.set(dir);
}

#[cfg(target_os = "windows")]
fn resource_dir() -> Option<PathBuf> {
    RESOURCE_DIR.get().cloned()
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_engine(
    engine: &str,
    img: image::DynamicImage,
    prep: PreprocessMode,
    tess_variant: &str,
) -> Result<String> {
    let prepared = preprocess(img, prep.resolve(engine));
    match engine {
        "tesseract" => tesseract::recognize(prepared, tess_variant),
        _ => {
            // "rapidocr" (default)
            #[cfg(feature = "rapidocr")]
            { rapidocr::recognize(prepared) }
            #[cfg(not(feature = "rapidocr"))]
            { let _ = prepared; Err(anyhow!("rapidocr_unavailable")) }
        }
    }
}

/// True if the given OCR backend is usable right now.
#[cfg(target_os = "windows")]
pub fn engine_status(engine: &str) -> bool {
    match engine {
        "tesseract" => tesseract::available(),
        _ => {
            #[cfg(feature = "rapidocr")]
            { rapidocr::available() }
            #[cfg(not(feature = "rapidocr"))]
            { false }
        }
    }
}

/// OCR a screenshot already on the clipboard.
#[cfg(target_os = "windows")]
pub fn recognize_clipboard(engine: &str, prep: PreprocessMode, tess_variant: &str) -> Result<String> {
    run_engine(engine, clipboard_image()?, prep, tess_variant)
}

/// OCR an image file from disk.
#[cfg(target_os = "windows")]
pub fn recognize_file(engine: &str, path: &str, prep: PreprocessMode, tess_variant: &str) -> Result<String> {
    let img = image::open(path).map_err(|e| anyhow!("open image: {e}"))?;
    run_engine(engine, img, prep, tess_variant)
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

// ── Test Mode ─────────────────────────────────────────────────────────────────

/// One engine's result for the OCR Test Mode comparison panel.
#[derive(serde::Serialize)]
pub struct OcrTestResult {
    pub engine: String,
    pub model: String,
    pub preprocess: String,
    pub ms: u128,
    pub text: String,
    pub error: Option<String>,
}

/// Runs both engines on one image file with the given preprocessing and returns
/// raw text + timing + model label for each. Normalization is added by lib.rs.
#[cfg(target_os = "windows")]
pub fn ocr_test(path: &str, prep: PreprocessMode, tess_variant: &str) -> Vec<OcrTestResult> {
    let mut out = Vec::new();
    for engine in ["rapidocr", "tesseract"] {
        let img = match image::open(path) {
            Ok(i) => i,
            Err(e) => {
                out.push(OcrTestResult {
                    engine: engine.into(),
                    model: String::new(),
                    preprocess: prep.resolve(engine).label().into(),
                    ms: 0,
                    text: String::new(),
                    error: Some(format!("open image: {e}")),
                });
                continue;
            }
        };
        let model = match engine {
            "tesseract" => format!("Tesseract rus+eng ({tess_variant})"),
            _ => {
                #[cfg(feature = "rapidocr")]
                { rapidocr::model_label() }
                #[cfg(not(feature = "rapidocr"))]
                { "RapidOCR (unavailable)".to_string() }
            }
        };
        let started = std::time::Instant::now();
        let result = run_engine(engine, img, prep, tess_variant);
        let ms = started.elapsed().as_millis();
        let (text, error) = match result {
            Ok(t) => (t, None),
            Err(e) => (String::new(), Some(e.to_string())),
        };
        out.push(OcrTestResult {
            engine: engine.into(),
            model,
            preprocess: prep.resolve(engine).label().into(),
            ms,
            text,
            error,
        });
    }
    out
}

// ── Tesseract CLI backend (bundled inside the app) ────────────────────────────
#[cfg(target_os = "windows")]
mod tesseract {
    use super::resource_dir;
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Locate tesseract.exe: bundled copy first (next to the installed exe under
    /// `tesseract/`), then a dev path, then PATH / Program Files as a fallback.
    fn exe() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        // DEV: prefer the freshly-staged source folder over target/<profile>,
        // which may hold stale placeholder copies of the resources.
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(PathBuf::from(d).join("tesseract").join("tesseract.exe"));
        }
        if let Some(r) = resource_dir() {
            candidates.push(r.join("tesseract").join("tesseract.exe"));
        }
        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }
        // Fallbacks: PATH, then the default UB-Mannheim install dir.
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

    /// Bundled tessdata dir for the chosen variant ("standard" | "fast"), if present.
    fn tessdata_dir(variant: &str) -> Option<PathBuf> {
        let sub = if variant == "fast" { "tessdata-fast" } else { "tessdata-standard" };
        let mut candidates: Vec<PathBuf> = Vec::new();
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(PathBuf::from(d).join("tesseract").join(sub));
        }
        if let Some(r) = resource_dir() {
            candidates.push(r.join("tesseract").join(sub));
        }
        // A non-empty traineddata file must be present (placeholder copies are 0 bytes).
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

    pub fn recognize(img: image::DynamicImage, variant: &str) -> Result<String> {
        let exe = exe().ok_or_else(|| anyhow!("tesseract_not_installed"))?;
        let tessdata = tessdata_dir(variant);

        let tmp = std::env::temp_dir().join(format!("deepm_ocr_{}.png", std::process::id()));
        img.save(&tmp).map_err(|e| anyhow!("save temp: {e}"))?;

        let langs = langs(&exe, tessdata.as_ref());
        let mut cmd = Command::new(&exe);
        cmd.arg(&tmp).arg("stdout").args(["-l", &langs, "--psm", "6"]);
        // Mixed RU/EN text (esp. Latin-Cyrillic hyphenated tokens like
        // "rec-модели") is mangled when Tesseract coerces a word toward one
        // language's dictionary. Turning the dictionaries off makes it read
        // character-by-character, which is what we want for technical text;
        // also keep inter-word spaces.
        cmd.args(["-c", "preserve_interword_spaces=1"]);
        cmd.args(["-c", "load_system_dawg=0"]);
        cmd.args(["-c", "load_freq_dawg=0"]);
        if let Some(d) = &tessdata {
            cmd.args(["--tessdata-dir", &d.to_string_lossy()]);
        }
        let output = cmd.no_window().output();
        let _ = std::fs::remove_file(&tmp);

        let output = output.map_err(|e| anyhow!("tesseract run: {e}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "tesseract error: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
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

// ── RapidOCR backend (PP-OCRv5 via ONNX) ──────────────────────────────────────
#[cfg(all(target_os = "windows", feature = "rapidocr"))]
mod rapidocr {
    use super::resource_dir;
    use anyhow::{anyhow, Result};
    use std::path::{Path, PathBuf};

    fn has_local(d: &Path) -> bool {
        let nonempty = |p: std::path::PathBuf| p.metadata().map(|m| m.len() > 0).unwrap_or(false);
        nonempty(d.join("det.onnx")) && nonempty(d.join("rec.onnx")) && nonempty(d.join("dict.txt"))
    }

    /// User override dir: drop a custom PP-OCR set here to replace the bundled one.
    fn user_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_default()
            .join("DeepM")
            .join("models")
            .join("rapidocr")
    }

    /// Bundled default models (PP-OCRv5 Cyrillic) shipped as a Tauri resource.
    fn bundled_dir() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        // DEV: prefer the freshly-staged source folder over target/<profile>.
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(PathBuf::from(d).join("rapidocr"));
        }
        if let Some(r) = resource_dir() {
            candidates.push(r.join("rapidocr"));
        }
        candidates.into_iter().find(|d| has_local(d))
    }

    /// Resolve the active model dir: user override wins, else bundled default.
    fn resolve() -> Option<(PathBuf, bool)> {
        let u = user_dir();
        if has_local(&u) {
            return Some((u, true));
        }
        bundled_dir().map(|d| (d, false))
    }

    pub fn available() -> bool {
        resolve().is_some()
    }

    pub fn model_label() -> String {
        match resolve() {
            Some((_, true)) => "RapidOCR (custom models)".to_string(),
            Some((_, false)) => "RapidOCR PP-OCRv5 cyrillic".to_string(),
            None => "RapidOCR (models missing)".to_string(),
        }
    }

    pub fn recognize(img: image::DynamicImage) -> Result<String> {
        use oar_ocr::prelude::*;

        let (dir, custom) = resolve().ok_or_else(|| {
            eprintln!("[RapidOCR] models MISSING (no bundled and no user models)");
            anyhow!("rapidocr_models_missing")
        })?;
        let det = dir.join("det.onnx").to_string_lossy().into_owned();
        let rec = dir.join("rec.onnx").to_string_lossy().into_owned();
        let dict = dir.join("dict.txt").to_string_lossy().into_owned();
        eprintln!(
            "[RapidOCR] {} models: {}",
            if custom { "CUSTOM" } else { "bundled PP-OCRv5 cyrillic" },
            dir.display()
        );

        let ocr = OAROCRBuilder::new(det, rec, dict).build().map_err(|e| {
            eprintln!("[RapidOCR] init FAILED: {e}");
            anyhow!("rapidocr init: {e}")
        })?;

        let tmp = std::env::temp_dir().join(format!("deepm_rocr_{}.png", std::process::id()));
        img.save(&tmp).map_err(|e| anyhow!("save temp: {e}"))?;
        let loaded = load_image(&tmp).map_err(|e| anyhow!("load: {e}"));
        let _ = std::fs::remove_file(&tmp);
        let loaded = loaded?;

        let results = ocr.predict(vec![loaded]).map_err(|e| {
            eprintln!("[RapidOCR] predict FAILED: {e}");
            anyhow!("rapidocr predict: {e}")
        })?;
        let mut lines: Vec<String> = Vec::new();
        if let Some(r) = results.get(0) {
            for region in &r.text_regions {
                if let Some((text, _conf)) = region.text_with_confidence() {
                    lines.push(text.to_string());
                }
            }
        }
        Ok(lines.join("\n"))
    }
}

// ── Non-Windows stubs ─────────────────────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
pub fn engine_status(_engine: &str) -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn recognize_clipboard(_engine: &str, _prep: PreprocessMode, _tess: &str) -> Result<String> {
    Err(anyhow!("OCR is Windows-only"))
}
#[cfg(not(target_os = "windows"))]
pub fn recognize_file(_engine: &str, _path: &str, _prep: PreprocessMode, _tess: &str) -> Result<String> {
    Err(anyhow!("OCR is Windows-only"))
}
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
pub fn ocr_test(_path: &str, _prep: PreprocessMode, _tess: &str) -> Vec<OcrTestResult> { Vec::new() }
