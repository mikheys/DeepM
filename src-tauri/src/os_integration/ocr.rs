//! Screenshot / image OCR via a bundled Tesseract, with automatic script
//! detection (OSD) and on-demand language packs.
//!
//! - `osd.traineddata` (bundled) detects the dominant script of an image; we map
//!   it to a Tesseract language and add it to `-l`, so a Chinese screenshot is
//!   read with `chi_sim`, a Russian one with `rus`, etc.
//! - Language data lives in one writable dir (`%LOCALAPPDATA%/DeepM/tessdata`),
//!   seeded from the bundle (eng/rus/osd). Extra languages are downloaded there
//!   on demand (by lib.rs), so the installer stays small.
//!
//! Windows-only; non-Windows targets get stubs at the bottom.

use anyhow::{anyhow, Result};
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

// ── Resource directory (set once at startup) ──────────────────────────────────

#[cfg(target_os = "windows")]
static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg(target_os = "windows")]
pub fn set_resource_dir(dir: PathBuf) {
    let _ = RESOURCE_DIR.set(dir);
}
#[cfg(target_os = "windows")]
fn resource_dir() -> Option<PathBuf> {
    RESOURCE_DIR.get().cloned()
}
#[cfg(target_os = "windows")]
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

// ── Preprocessing ─────────────────────────────────────────────────────────────

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

/// Tesseract codes we offer + display names. eng/rus/osd are bundled; the rest
/// download on demand.
pub const SUPPORTED_LANGS: &[(&str, &str)] = &[
    ("eng", "English"),
    ("rus", "Русский"),
    ("chi_sim", "中文 (简体)"),
    ("chi_tra", "中文 (繁體)"),
    ("jpn", "日本語"),
    ("kor", "한국어"),
    ("deu", "Deutsch"),
    ("fra", "Français"),
    ("spa", "Español"),
    ("ita", "Italiano"),
    ("por", "Português"),
    ("ukr", "Українська"),
    ("pol", "Polski"),
    ("tur", "Türkçe"),
    ("ara", "العربية"),
    ("ell", "Ελληνικά"),
];

#[cfg(target_os = "windows")]
pub fn engine_status() -> bool {
    tesseract::available()
}

/// Writable tessdata dir (download target / language list source).
#[cfg(target_os = "windows")]
pub fn tessdata_user_dir() -> PathBuf {
    tesseract::data_dir()
}

/// Installed language codes (excluding "osd").
#[cfg(target_os = "windows")]
pub fn installed_langs() -> Vec<String> {
    tesseract::installed_langs()
}

#[cfg(target_os = "windows")]
pub fn is_lang_installed(code: &str) -> bool {
    tesseract::is_installed(code)
}

/// Removes a downloaded language (bundled eng/rus/osd can't be removed).
#[cfg(target_os = "windows")]
pub fn remove_lang(code: &str) -> bool {
    tesseract::remove_lang(code)
}

/// Detects the dominant script of the clipboard / file image → a language code
/// (e.g. "chi_sim"), or None if undetermined.
#[cfg(target_os = "windows")]
pub fn detect_clipboard_script() -> Option<String> {
    tesseract::detect_script(preprocess(clipboard_image().ok()?))
}
#[cfg(target_os = "windows")]
pub fn detect_file_script(path: &str) -> Option<String> {
    tesseract::detect_script(preprocess(image::open(path).ok()?))
}

/// OCR the clipboard / a file with the given `+`-joined Tesseract languages.
#[cfg(target_os = "windows")]
pub fn recognize_clipboard(lang_arg: &str) -> Result<String> {
    tesseract::recognize(preprocess(clipboard_image()?), lang_arg, 6)
}
#[cfg(target_os = "windows")]
pub fn recognize_file(path: &str, lang_arg: &str) -> Result<String> {
    let img = image::open(path).map_err(|e| anyhow!("open image: {e}"))?;
    tesseract::recognize(preprocess(img), lang_arg, 6)
}

/// OCR a mixed-script image (e.g. a Chinese page with a Russian/English title).
/// Runs two passes — `primary` (the detected dominant script) and `secondary`
/// (the user's Latin/Cyrillic set) — then keeps, per line, whichever pass was
/// more confident. This avoids the cross-script mixing a single `chi_sim+rus`
/// pass produces while still recovering the minority-language lines.
#[cfg(target_os = "windows")]
pub fn recognize_clipboard_merged(primary: &str, secondary: &str) -> Result<String> {
    tesseract::recognize_merged(preprocess(clipboard_image()?), primary, secondary, 6)
}
#[cfg(target_os = "windows")]
pub fn recognize_file_merged(path: &str, primary: &str, secondary: &str) -> Result<String> {
    let img = image::open(path).map_err(|e| anyhow!("open image: {e}"))?;
    tesseract::recognize_merged(preprocess(img), primary, secondary, 6)
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

#[derive(serde::Serialize)]
pub struct OcrTestResult {
    pub engine: String,
    pub model: String,
    pub preprocess: String,
    pub ms: u128,
    pub text: String,
    pub error: Option<String>,
}

#[cfg(target_os = "windows")]
pub fn ocr_test_all(path: &str) -> Vec<OcrTestResult> {
    let langs = {
        let mut l = installed_langs();
        if l.is_empty() { l = vec!["eng".into()]; }
        l.join("+")
    };
    let mut out = Vec::new();
    for psm in [3u32, 6, 11] {
        let model = format!("Tesseract {langs} psm{psm}");
        let img = match image::open(path) {
            Ok(i) => i,
            Err(e) => {
                out.push(OcrTestResult { engine: "tesseract".into(), model, preprocess: "resize+grayscale".into(), ms: 0, text: String::new(), error: Some(format!("open image: {e}")) });
                continue;
            }
        };
        let started = std::time::Instant::now();
        let result = tesseract::recognize(preprocess(img), &langs, psm);
        let ms = started.elapsed().as_millis();
        let (text, error) = match result { Ok(t) => (t, None), Err(e) => (String::new(), Some(e.to_string())) };
        out.push(OcrTestResult { engine: "tesseract".into(), model, preprocess: "resize+grayscale".into(), ms, text, error });
    }
    out
}

// ── Tesseract CLI backend ─────────────────────────────────────────────────────
#[cfg(target_os = "windows")]
mod tesseract {
    use super::{exe_dir, resource_dir};
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Bundled (never removable) data, seeded into the writable dir.
    const BUNDLED: &[&str] = &["eng", "rus", "osd"];

    fn exe() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(PathBuf::from(d).join("tesseract").join("tesseract.exe"));
        }
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

    pub fn available() -> bool {
        exe().is_some()
    }

    /// The bundled tessdata-standard dir (source for seeding eng/rus/osd).
    fn bundled_tessdata() -> Option<PathBuf> {
        let mut cands: Vec<PathBuf> = Vec::new();
        #[cfg(debug_assertions)]
        if let Some(d) = option_env!("CARGO_MANIFEST_DIR") {
            cands.push(PathBuf::from(d).join("tesseract").join("tessdata-standard"));
        }
        if let Some(d) = exe_dir() {
            cands.push(d.join("tesseract").join("tessdata-standard"));
        }
        if let Some(r) = resource_dir() {
            cands.push(r.join("tesseract").join("tessdata-standard"));
        }
        cands.into_iter().find(|d| d.join("eng.traineddata").exists())
    }

    /// One writable tessdata dir holding bundled + downloaded languages, so a
    /// single --tessdata-dir covers everything. Seeded from the bundle.
    pub fn data_dir() -> PathBuf {
        let dir = dirs::data_local_dir()
            .unwrap_or_default()
            .join("DeepM")
            .join("tessdata");
        let _ = std::fs::create_dir_all(&dir);
        if let Some(bundle) = bundled_tessdata() {
            for code in BUNDLED {
                let dst = dir.join(format!("{code}.traineddata"));
                let src = bundle.join(format!("{code}.traineddata"));
                if !dst.exists() && src.exists() {
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
        dir
    }

    pub fn installed_langs() -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(data_dir()) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(code) = name.strip_suffix(".traineddata") {
                    if code != "osd" {
                        out.push(code.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }

    pub fn is_installed(code: &str) -> bool {
        data_dir()
            .join(format!("{code}.traineddata"))
            .metadata()
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    }

    pub fn remove_lang(code: &str) -> bool {
        if BUNDLED.contains(&code) {
            return false;
        }
        std::fs::remove_file(data_dir().join(format!("{code}.traineddata"))).is_ok()
    }

    /// Map a Tesseract OSD script name to a language code we can OCR with.
    fn script_to_lang(script: &str) -> Option<&'static str> {
        Some(match script {
            "Han" => "chi_sim",
            "Japanese" => "jpn",
            "Korean" | "Hangul" => "kor",
            "Cyrillic" => "rus",
            "Latin" => "eng",
            "Arabic" => "ara",
            "Greek" => "ell",
            _ => return None,
        })
    }

    /// Run OSD (--psm 0) to detect the dominant script → language code.
    pub fn detect_script(img: image::DynamicImage) -> Option<String> {
        let exe = exe()?;
        let dir = data_dir();

        // OSD needs osd.traineddata in the tessdata dir. If it's missing (e.g.
        // it was never staged into the build) detection silently fails and any
        // non-Latin/Cyrillic text (Chinese, Japanese, …) gets OCR'd with the
        // wrong language. Make that situation diagnosable instead of silent.
        if !dir.join("osd.traineddata").exists() {
            super::dbg_log("OSD SKIP: osd.traineddata missing in tessdata dir (auto language detection disabled)");
            return None;
        }

        let tmp = std::env::temp_dir().join(format!("deepm_osd_{}.png", std::process::id()));
        img.save(&tmp).ok()?;
        let out = Command::new(&exe)
            .arg(&tmp)
            .arg("stdout")
            .args(["--psm", "0"])
            .args(["--tessdata-dir", &dir.to_string_lossy()])
            .no_window()
            .output();
        let _ = std::fs::remove_file(&tmp);
        let out = out.ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let script = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("Script: ").map(|s| s.trim().to_string()));
        match script {
            Some(s) => {
                let lang = script_to_lang(&s).map(String::from);
                super::dbg_log(&format!("OSD: script={s} -> lang={lang:?}"));
                lang
            }
            None => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                super::dbg_log(&format!("OSD: no script line (stderr={})", stderr.trim()));
                None
            }
        }
    }

    pub fn recognize(img: image::DynamicImage, lang_arg: &str, psm: u32) -> Result<String> {
        let exe = exe().ok_or_else(|| anyhow!("tesseract_not_installed"))?;
        let dir = data_dir();
        let lang = if lang_arg.trim().is_empty() { "eng" } else { lang_arg };

        let tmp = std::env::temp_dir().join(format!("deepm_ocr_{}.png", std::process::id()));
        img.save(&tmp).map_err(|e| anyhow!("save temp: {e}"))?;

        let psm = if (3..=13).contains(&psm) { psm } else { 6 };
        let psm_s = psm.to_string();
        let mut cmd = Command::new(&exe);
        cmd.arg(&tmp).arg("stdout").args(["-l", lang, "--oem", "1", "--psm", &psm_s]);
        cmd.args(["-c", "preserve_interword_spaces=1"]);
        cmd.args(["-c", "load_system_dawg=0"]);
        cmd.args(["-c", "load_freq_dawg=0"]);
        cmd.args(["--tessdata-dir", &dir.to_string_lossy()]);
        let output = cmd.no_window().output();
        let _ = std::fs::remove_file(&tmp);

        let output = output.map_err(|e| {
            super::dbg_log(&format!("OCR FAIL: spawn (exe={}): {e}", exe.display()));
            anyhow!("tesseract run: {e}")
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            super::dbg_log(&format!(
                "OCR FAIL: exit {:?} lang={lang} tessdata={} stderr={}",
                output.status.code(),
                dir.display(),
                stderr.trim()
            ));
            return Err(anyhow!("tesseract error: {}", stderr.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// One recognized text line with its mean word confidence (0..100).
    struct TsvLine {
        conf: f32,
        text: String,
    }

    /// Runs Tesseract with TSV output and groups words into lines, each with a
    /// mean confidence. Used by the two-pass merge.
    fn run_tsv(exe: &PathBuf, dir: &PathBuf, tmp: &PathBuf, lang: &str, psm: u32) -> Result<Vec<TsvLine>> {
        let psm_s = psm.to_string();
        let mut cmd = Command::new(exe);
        cmd.arg(tmp).arg("stdout").args(["-l", lang, "--oem", "1", "--psm", &psm_s]);
        cmd.args(["-c", "preserve_interword_spaces=1"]);
        cmd.args(["-c", "load_system_dawg=0"]);
        cmd.args(["-c", "load_freq_dawg=0"]);
        cmd.args(["--tessdata-dir", &dir.to_string_lossy()]);
        cmd.arg("tsv"); // config name → emit TSV to stdout
        let output = cmd.no_window().output().map_err(|e| anyhow!("tesseract tsv run: {e}"))?;
        if !output.status.success() {
            return Err(anyhow!("tesseract tsv exit {:?}", output.status.code()));
        }
        Ok(parse_tsv(&String::from_utf8_lossy(&output.stdout)))
    }

    /// Parses Tesseract TSV (columns: level page block par line word left top
    /// width height conf text) into per-line text + mean confidence, preserving
    /// reading order.
    fn parse_tsv(tsv: &str) -> Vec<TsvLine> {
        let mut lines: Vec<TsvLine> = Vec::new();
        let mut cur_key: Option<(u32, u32, u32)> = None;
        let mut words: Vec<String> = Vec::new();
        let mut confs: Vec<f32> = Vec::new();

        let flush = |lines: &mut Vec<TsvLine>, words: &mut Vec<String>, confs: &mut Vec<f32>| {
            if !words.is_empty() {
                let text = words.join(" ");
                let conf = if confs.is_empty() {
                    0.0
                } else {
                    confs.iter().sum::<f32>() / confs.len() as f32
                };
                if !text.trim().is_empty() {
                    lines.push(TsvLine { conf, text });
                }
            }
            words.clear();
            confs.clear();
        };

        for (i, row) in tsv.lines().enumerate() {
            if i == 0 {
                continue; // header
            }
            let c: Vec<&str> = row.split('\t').collect();
            if c.len() < 12 {
                continue;
            }
            let level: u32 = c[0].parse().unwrap_or(0);
            if level != 5 {
                continue; // only word rows carry text + confidence
            }
            let key = (
                c[2].parse().unwrap_or(0),
                c[3].parse().unwrap_or(0),
                c[4].parse().unwrap_or(0),
            );
            if cur_key != Some(key) {
                flush(&mut lines, &mut words, &mut confs);
                cur_key = Some(key);
            }
            let conf: f32 = c[10].parse().unwrap_or(-1.0);
            let word = c[11].trim();
            if !word.is_empty() {
                words.push(word.to_string());
                if conf >= 0.0 {
                    confs.push(conf);
                }
            }
        }
        flush(&mut lines, &mut words, &mut confs);
        lines
    }

    /// Two-pass merge: OCR with `primary` and `secondary`, then keep the more
    /// confident pass per line. Falls back to a plain `primary` pass if the two
    /// passes don't segment into the same number of lines (so we never emit a
    /// scrambled interleaving).
    pub fn recognize_merged(
        img: image::DynamicImage,
        primary: &str,
        secondary: &str,
        psm: u32,
    ) -> Result<String> {
        let exe = exe().ok_or_else(|| anyhow!("tesseract_not_installed"))?;
        let dir = data_dir();
        let psm = if (3..=13).contains(&psm) { psm } else { 6 };

        let tmp = std::env::temp_dir().join(format!("deepm_ocr_m_{}.png", std::process::id()));
        img.save(&tmp).map_err(|e| anyhow!("save temp: {e}"))?;

        let a = run_tsv(&exe, &dir, &tmp, primary, psm);
        let b = run_tsv(&exe, &dir, &tmp, secondary, psm);
        let _ = std::fs::remove_file(&tmp);

        let (a, b) = match (a, b) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                super::dbg_log("OCR merge: a TSV pass failed, falling back to primary");
                return recognize(img, primary, psm);
            }
        };

        if !a.is_empty() && a.len() == b.len() {
            // Bias toward the primary (detected) script: only take the secondary
            // line when it's clearly more confident.
            const MARGIN: f32 = 5.0;
            let mut out = String::new();
            let mut took_secondary = 0;
            for (la, lb) in a.iter().zip(b.iter()) {
                let pick = if lb.conf > la.conf + MARGIN {
                    took_secondary += 1;
                    &lb.text
                } else {
                    &la.text
                };
                out.push_str(pick);
                out.push('\n');
            }
            super::dbg_log(&format!(
                "OCR merge: {} lines, {took_secondary} from secondary ({secondary})",
                a.len()
            ));
            Ok(out.trim_end().to_string())
        } else {
            super::dbg_log(&format!(
                "OCR merge: line count mismatch (primary {}, secondary {}) → primary only",
                a.len(),
                b.len()
            ));
            recognize(img, primary, psm)
        }
    }

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

/// Appends a line to %LOCALAPPDATA%/DeepM/ocr-debug.log (visible in release).
#[cfg(target_os = "windows")]
fn dbg_log(msg: &str) {
    if let Some(d) = dirs::data_local_dir() {
        let dir = d.join("DeepM");
        let _ = std::fs::create_dir_all(&dir);
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("ocr-debug.log")) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

// ── Non-Windows stubs ─────────────────────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
pub fn engine_status() -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn tessdata_user_dir() -> std::path::PathBuf { std::path::PathBuf::new() }
#[cfg(not(target_os = "windows"))]
pub fn installed_langs() -> Vec<String> { Vec::new() }
#[cfg(not(target_os = "windows"))]
pub fn is_lang_installed(_code: &str) -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn remove_lang(_code: &str) -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn detect_clipboard_script() -> Option<String> { None }
#[cfg(not(target_os = "windows"))]
pub fn detect_file_script(_path: &str) -> Option<String> { None }
#[cfg(not(target_os = "windows"))]
pub fn recognize_clipboard(_lang_arg: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_file(_path: &str, _lang_arg: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_clipboard_merged(_p: &str, _s: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_file_merged(_path: &str, _p: &str, _s: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
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
