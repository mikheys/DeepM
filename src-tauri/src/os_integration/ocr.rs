//! Screenshot / image OCR with a selectable backend:
//! - "windows"  : built-in Windows.Media.Ocr (offline, zero bundle)
//! - "tesseract": the Tesseract CLI if installed (offline, more tuning)
//! - "rapidocr" : reserved (ONNX pipeline) — not wired up yet
//!
//! All backends share the same preprocessing (upscale + grayscale), which is
//! the single biggest accuracy win on low-res screenshots.

use anyhow::{anyhow, Result};

/// Upscales small images (OCR is tuned for ~300dpi) and grayscales them.
#[cfg(target_os = "windows")]
fn preprocess(img: image::DynamicImage) -> image::DynamicImage {
    use image::GenericImageView;
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    let scale = if longest < 1000 { 3 } else if longest < 2200 { 2 } else { 1 };
    if scale > 1 {
        img.resize(w * scale, h * scale, image::imageops::FilterType::Lanczos3)
            .grayscale()
    } else {
        img.grayscale()
    }
}

#[cfg(target_os = "windows")]
fn run_engine(engine: &str, prepared: image::DynamicImage) -> Result<String> {
    match engine {
        "tesseract" => tesseract::recognize(prepared),
        "rapidocr" => Err(anyhow!("rapidocr_unavailable")),
        _ => win::recognize(prepared),
    }
}

/// True if the given OCR backend is usable right now.
#[cfg(target_os = "windows")]
pub fn engine_status(engine: &str) -> bool {
    match engine {
        "tesseract" => tesseract::available(),
        "rapidocr" => false,
        _ => win::available(),
    }
}

/// OCR a screenshot already on the clipboard.
#[cfg(target_os = "windows")]
pub fn recognize_clipboard(engine: &str) -> Result<String> {
    let img = arboard::Clipboard::new()
        .map_err(|e| anyhow!("clipboard: {e}"))?
        .get_image()
        .map_err(|_| anyhow!("no_image"))?;
    let buf = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
        .ok_or_else(|| anyhow!("bad clipboard image"))?;
    run_engine(engine, preprocess(image::DynamicImage::ImageRgba8(buf)))
}

/// OCR an image file from disk.
#[cfg(target_os = "windows")]
pub fn recognize_file(engine: &str, path: &str) -> Result<String> {
    let img = image::open(path).map_err(|e| anyhow!("open image: {e}"))?;
    run_engine(engine, preprocess(img))
}

// ── Windows.Media.Ocr backend ─────────────────────────────────────────────────
#[cfg(target_os = "windows")]
mod win {
    use anyhow::{anyhow, Result};

    pub fn available() -> bool {
        use windows::Media::Ocr::OcrEngine;
        OcrEngine::AvailableRecognizerLanguages()
            .and_then(|l| l.Size())
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    /// Prefer a Russian recognizer (reads Cyrillic AND Latin) over the default
    /// Latin-only one, so mixed RU/EN text comes out right.
    fn make_engine() -> Result<windows::Media::Ocr::OcrEngine> {
        use windows::core::HSTRING;
        use windows::Globalization::Language;
        use windows::Media::Ocr::OcrEngine;
        for tag in ["ru", "ru-RU"] {
            if let Ok(lang) = Language::CreateLanguage(&HSTRING::from(tag)) {
                if OcrEngine::IsLanguageSupported(&lang).unwrap_or(false) {
                    if let Ok(eng) = OcrEngine::TryCreateFromLanguage(&lang) {
                        return Ok(eng);
                    }
                }
            }
        }
        OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| anyhow!("no_ocr_language"))
    }

    pub fn recognize(img: image::DynamicImage) -> Result<String> {
        use windows::Graphics::Imaging::BitmapDecoder;
        use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

        let mut png: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| anyhow!("encode: {e}"))?;

        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
        writer.WriteBytes(&png)?;
        writer.StoreAsync()?.get()?;
        writer.FlushAsync()?.get()?;
        stream.Seek(0)?;

        let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
        let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;
        let engine = make_engine()?;
        let result = engine.RecognizeAsync(&bitmap)?.get()?;
        Ok(result.Text()?.to_string())
    }
}

// ── Tesseract CLI backend ─────────────────────────────────────────────────────
#[cfg(target_os = "windows")]
mod tesseract {
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Locate tesseract.exe (PATH or the default UB-Mannheim install dir).
    fn exe() -> Option<PathBuf> {
        // PATH
        if Command::new("tesseract").arg("--version").creation_flags2().output().is_ok() {
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

    /// Languages to pass: prefer rus+eng, falling back to whatever is installed.
    fn langs(exe: &PathBuf) -> String {
        let out = Command::new(exe)
            .arg("--list-langs")
            .creation_flags2()
            .output();
        let installed: Vec<String> = out
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        let has = |l: &str| installed.iter().any(|x| x == l);
        match (has("rus"), has("eng")) {
            (true, true) => "rus+eng".into(),
            (true, false) => "rus".into(),
            (false, true) => "eng".into(),
            _ => "eng".into(),
        }
    }

    pub fn recognize(img: image::DynamicImage) -> Result<String> {
        let exe = exe().ok_or_else(|| anyhow!("tesseract_not_installed"))?;

        let tmp = std::env::temp_dir().join(format!("deepm_ocr_{}.png", std::process::id()));
        img.save(&tmp).map_err(|e| anyhow!("save temp: {e}"))?;

        let langs = langs(&exe);
        let output = Command::new(&exe)
            .arg(&tmp)
            .arg("stdout")
            .args(["-l", &langs, "--psm", "6"])
            .creation_flags2()
            .output();
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
        fn creation_flags2(&mut self) -> &mut Self;
    }
    impl NoWindow for Command {
        fn creation_flags2(&mut self) -> &mut Self {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW)
        }
    }
}

// ── Non-Windows stubs ─────────────────────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
pub fn engine_status(_engine: &str) -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn recognize_clipboard(_engine: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_file(_engine: &str, _path: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
