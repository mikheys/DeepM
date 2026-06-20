//! On-screen / screenshot OCR using the built-in Windows OCR engine
//! (Windows.Media.Ocr) — offline, no third-party engine or bundle. Language
//! packs (e.g. Russian) may need to be installed via Windows Settings.

use anyhow::{anyhow, Result};

/// True if at least one OCR recognizer language is installed.
#[cfg(target_os = "windows")]
pub fn ocr_available() -> bool {
    use windows::Media::Ocr::OcrEngine;
    OcrEngine::AvailableRecognizerLanguages()
        .and_then(|langs| langs.Size())
        .map(|n| n > 0)
        .unwrap_or(false)
}

/// Builds an OCR engine, preferring a Cyrillic-capable recognizer (Russian)
/// when installed — it reads both Cyrillic AND Latin, so mixed RU/EN text comes
/// out correctly. A Latin-only (English) engine misreads Cyrillic as lookalikes.
#[cfg(target_os = "windows")]
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

/// Runs OCR on PNG-encoded image bytes.
#[cfg(target_os = "windows")]
pub fn recognize_png(png: &[u8]) -> Result<String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
    writer.WriteBytes(png)?;
    writer.StoreAsync()?.get()?;
    writer.FlushAsync()?.get()?;
    stream.Seek(0)?;

    let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
    let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;

    let engine = make_engine()?;
    let result = engine.RecognizeAsync(&bitmap)?.get()?;
    Ok(result.Text()?.to_string())
}

/// Upscales (Windows OCR is tuned for ~300dpi scans, screenshots are low-res),
/// grayscales, then OCRs. Upscaling is the single biggest accuracy win on
/// screenshots and small UI text.
#[cfg(target_os = "windows")]
fn ocr_dynimage(img: image::DynamicImage) -> Result<String> {
    use image::GenericImageView;
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    let scale = if longest < 1000 { 3 } else if longest < 2200 { 2 } else { 1 };

    let prepared = if scale > 1 {
        img.resize(w * scale, h * scale, image::imageops::FilterType::Lanczos3)
            .grayscale()
    } else {
        img.grayscale()
    };

    let mut png: Vec<u8> = Vec::new();
    prepared
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| anyhow!("encode: {e}"))?;
    recognize_png(&png)
}

/// Reads an image from the clipboard (a screenshot), runs OCR, returns text.
#[cfg(target_os = "windows")]
pub fn recognize_clipboard() -> Result<String> {
    let img = arboard::Clipboard::new()
        .map_err(|e| anyhow!("clipboard: {e}"))?
        .get_image()
        .map_err(|_| anyhow!("no_image"))?;

    let w = img.width as u32;
    let h = img.height as u32;
    let rgba = img.bytes.into_owned();
    let buf = image::RgbaImage::from_raw(w, h, rgba)
        .ok_or_else(|| anyhow!("bad clipboard image"))?;

    ocr_dynimage(image::DynamicImage::ImageRgba8(buf))
}

/// Reads an image file from disk, runs OCR, returns text.
#[cfg(target_os = "windows")]
pub fn recognize_file(path: &str) -> Result<String> {
    let dynimg = image::open(path).map_err(|e| anyhow!("open image: {e}"))?;
    ocr_dynimage(dynimg)
}

// ── Non-Windows stubs ─────────────────────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
pub fn ocr_available() -> bool { false }
#[cfg(not(target_os = "windows"))]
pub fn recognize_png(_png: &[u8]) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_clipboard() -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
#[cfg(not(target_os = "windows"))]
pub fn recognize_file(_path: &str) -> Result<String> { Err(anyhow!("OCR is Windows-only")) }
