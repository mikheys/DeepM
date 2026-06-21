use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tokio::sync::Mutex;
use std::sync::Mutex as StdMutex;

mod config;
mod core;
mod logging;
mod os_integration;
mod shortcuts;

use config::{AppSettings, load_settings, save_settings as persist_settings};
use core::{
    engine::{cuda_available, nvidia_gpu_present, TranslationEngine, TranslationRequest},
    history::{HistoryEntry, TranslationHistory},
    model_manager::{DownloadState, ModelManager, ModelSpec, ModelStatus},
};
use shortcuts::ShortcutConfig;

// ── App State ────────────────────────────────────────────────────────────────

pub(crate) struct AppState {
    settings: Mutex<AppSettings>,
    engine: Arc<TranslationEngine>,
    model_manager: Arc<ModelManager>,
    history: Mutex<TranslationHistory>,
    /// Whether the floating button feature is active.
    floating_enabled: Mutex<bool>,
    /// Last known cursor position from hook thread.
    last_cursor: Arc<StdMutex<(f64, f64)>>,
    /// True when the main window has keyboard focus (skip floating button).
    main_window_focused: Arc<AtomicBool>,
    /// Bumped on every mouse-selection event; used to debounce the floating
    /// button so a quick accidental select+deselect doesn't make it flash.
    selection_gen: Arc<AtomicU64>,
    /// Live hotkey config shared with the global hook thread; updated on save.
    hook_config: Arc<os_integration::SharedHookConfig>,
}

// ── Translation Commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn get_model_status(state: State<'_, AppState>) -> Result<ModelStatus, String> {
    let status = state.model_manager.get_status().await;
    // The stored status can be stale (it's only updated on load/error). If it
    // claims Ready, confirm the engine process is actually answering so the UI
    // status dot reflects the live engine, not the last-known state.
    if matches!(status, ModelStatus::Ready { .. }) && !state.engine.is_running().await {
        return Ok(ModelStatus::Error {
            message: "Engine not responding".to_string(),
        });
    }
    Ok(status)
}

#[tauri::command]
async fn start_model_download(
    version: String,
    size: String,
    quantization: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let model_dir = {
        let s = state.settings.lock().await;
        s.model_path.clone()
    };

    let manager = Arc::clone(&state.model_manager);
    let engine = Arc::clone(&state.engine);
    let app_clone = app.clone();
    let progress_manager = Arc::clone(&state.model_manager);
    let dl_version = version.clone();
    let dl_size = size.clone();
    let dl_quant = quantization.clone();

    tokio::spawn(async move {
        let result = manager
            .download(
                &model_dir,
                &version,
                &size,
                &quantization,
                move |progress, speed_mbps| {
                    // Persist progress so reopening the tab resumes the display.
                    progress_manager.set_download_state(&dl_version, &dl_size, &dl_quant, progress, speed_mbps);
                    let _ = app_clone.emit(
                        "download_progress",
                        serde_json::json!({ "progress": progress, "speed_mbps": speed_mbps }),
                    );
                },
            )
            .await;

        manager.clear_download_state();

        match result {
            Ok(path) => {
                // Auto-load only when nothing is running yet (first / onboarding
                // model). If a model is already active, just mark this one as
                // downloaded — the user loads it explicitly via the Load button,
                // so an in-progress translation session isn't disrupted.
                if engine.is_running().await {
                    let _ = manager.probe(&model_dir, &version, &size, &quantization).await;
                    let _ = app.emit("model_downloaded", ());
                } else {
                    match engine.start(path).await {
                        Ok(()) => {
                            let _ = manager.probe(&model_dir, &version, &size, &quantization).await;
                            if let Ok(mut s) = config::load_settings() {
                                s.model_version = version.clone();
                                s.model_size = size.clone();
                                s.quantization = quantization.clone();
                                let _ = persist_settings(&s);
                            }
                            let _ = app.emit("model_ready", ());
                            os_integration::tray::update_tray_model_status(&app, "model ready");
                        }
                        Err(e) => {
                            log::error!("Engine start failed: {e}");
                            logging::error("engine", &e.to_string());
                            let _ = app.emit("model_error", e.to_string());
                            os_integration::tray::update_tray_model_status(&app, "engine error");
                        }
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("cancelled") {
                    log::info!("Download cancelled by user");
                    let _ = app.emit("download_cancelled", ());
                } else {
                    log::error!("Download failed: {e}");
                    logging::error("download", &e.to_string());
                    let _ = app.emit("model_error", e.to_string());
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn cancel_model_download(state: State<'_, AppState>) -> Result<(), String> {
    state.model_manager.cancel().await;
    Ok(())
}

/// Active model family ("HY-MT1.5" / "Hy-MT2"), detected from the loaded model
/// file path (works for external models too), falling back to settings.
async fn active_model_version(state: &AppState) -> String {
    if let ModelStatus::Ready { path } = state.model_manager.get_status().await {
        let p = path.to_lowercase();
        if p.contains("mt2") { return "Hy-MT2".to_string(); }
        if p.contains("mt1.5") { return "HY-MT1.5".to_string(); }
    }
    state.settings.lock().await.model_version.clone()
}

#[tauri::command]
async fn translate(
    source_text: String,
    source_lang: String,
    target_lang: String,
    context: Option<String>,
    glossary_entries: Option<Vec<serde_json::Value>>,
    mode: Option<String>,
    style: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let resolved_source_lang = if source_lang == "auto" {
        detect_language_internal(&source_text)
    } else {
        source_lang.clone()
    };

    let glossary: Vec<(String, String)> = glossary_entries
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let s = e.get("source")?.as_str()?.to_string();
            let t = e.get("target")?.as_str()?.to_string();
            Some((s, t))
        })
        .collect();

    let req = TranslationRequest {
        source_text: source_text.clone(),
        source_lang: resolved_source_lang.clone(),
        target_lang: target_lang.clone(),
        context,
        glossary,
        version: active_model_version(&state).await,
        mode: mode.unwrap_or_else(|| "standard".to_string()),
        style,
    };

    let result = state.engine.translate(req).await.map_err(|e| e.to_string())?;

    {
        let entry = HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            source_lang: resolved_source_lang.clone(),
            target_lang: target_lang.clone(),
            source_text,
            translated_text: result.translated_text.clone(),
        };
        state.history.lock().await.add(entry);
    }

    Ok(serde_json::json!({
        "translated_text": result.translated_text,
        "detected_lang": resolved_source_lang,
    }))
}

#[tauri::command]
async fn detect_language(text: String) -> Result<String, String> {
    Ok(detect_language_internal(&text))
}

fn detect_language_internal(text: &str) -> String {
    // The user works primarily in Russian and English. whatlang frequently
    // mislabels short Russian text as Ukrainian, or short Latin text as some
    // other language, which then drives the wrong translation direction.
    // So: classify by script first. Any Cyrillic => Russian, plain Latin =>
    // English. Only when a clearly different script dominates (CJK, Arabic,
    // Hebrew, Thai, …) do we defer to whatlang for an accurate label.
    let mut cyrillic = 0usize;
    let mut latin = 0usize;
    let mut other = 0usize;
    for ch in text.chars() {
        if ('\u{0400}'..='\u{052F}').contains(&ch) {
            cyrillic += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        } else if ch.is_alphabetic() {
            other += 1;
        }
    }

    let letters = cyrillic + latin + other;
    if letters == 0 {
        return "en".to_string();
    }

    // A non-Latin/non-Cyrillic script dominates → trust whatlang (zh/ja/ko/…).
    if other * 2 > letters {
        return whatlang::detect(text)
            .map(|i| lang_to_code(i.lang()))
            .unwrap_or_else(|| "en".to_string());
    }

    if cyrillic > 0 && cyrillic >= latin {
        "ru".to_string()
    } else {
        "en".to_string()
    }
}

/// Chooses the target language for "auto" mode given the detected source and
/// the user's preferred (priority) language. Foreign text goes INTO the
/// priority language; text already in the priority language goes into the
/// secondary one. Default priority "ru" → zh/en/… → ru, ru → en.
fn auto_target(src: &str, priority: &str) -> String {
    let secondary = if priority == "ru" { "en" } else { "ru" };
    if src == priority {
        secondary.to_string()
    } else {
        priority.to_string()
    }
}

fn lang_to_code(lang: whatlang::Lang) -> String {
    use whatlang::Lang::*;
    match lang {
        Cmn => "zh",
        Eng => "en",
        Fra => "fr",
        Deu => "de",
        Spa => "es",
        Por => "pt",
        Ita => "it",
        Nld => "nl",
        Pol => "pl",
        Ces => "cs",
        Rus => "ru",
        Ukr => "uk",
        Jpn => "ja",
        Kor => "ko",
        Ara => "ar",
        Heb => "he",
        Tur => "tr",
        Tha => "th",
        Vie => "vi",
        Ind => "id",
        Hin => "hi",
        Ben => "bn",
        _ => "en",
    }.to_string()
}

// ── Settings Commands ─────────────────────────────────────────────────────────

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().await.clone())
}

#[tauri::command]
async fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    persist_settings(&settings).map_err(|e| e.to_string())?;
    let new_floating = settings.show_floating_button;

    // Apply hotkey changes to the running hook immediately (no restart needed).
    state.hook_config.update(
        settings.hotkeys.translate_replace.clone(),
        settings.triple_copy_interval_ms,
        settings.triple_copy_count,
    );
    // GPU preference applies on the next engine (re)start.
    state.engine.set_use_gpu(settings.use_gpu);

    {
        let mut s = state.settings.lock().await;
        *s = settings;
    }
    // Sync runtime floating_enabled so Settings panel changes take effect immediately
    {
        let mut f = state.floating_enabled.lock().await;
        *f = new_floating;
    }
    if !new_floating {
        os_integration::hide_floating(&app);
    }
    os_integration::tray::rebuild_tray_menu(&app, new_floating);
    Ok(())
}

// ── History Commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    Ok(state.history.lock().await.all().to_vec())
}

#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history.lock().await.clear();
    Ok(())
}

// ── Stage 2: OS Integration Commands ─────────────────────────────────────────

/// Called by frontend when a triple-copy hotkey fires: reads clipboard,
/// opens main window, inserts text.
#[tauri::command]
async fn handle_triple_copy(app: AppHandle) -> Result<serde_json::Value, String> {
    // The clipboard already has the copied text (triple Ctrl+C is itself a copy action).
    // No need to restore: the user explicitly copied it.
    let text = os_integration::read_clipboard()
        .unwrap_or_default();

    // Bring main window to front
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }

    Ok(serde_json::json!({ "text": text }))
}

/// Translate-and-replace: copy selection → translate → paste back,
/// then restore original clipboard.
#[tauri::command]
async fn translate_and_replace(
    source_lang: String,
    target_lang: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // Respect the per-app exclusion list — do nothing in excluded apps.
    {
        let exclusions = state.settings.lock().await.floating_exclusions.clone();
        if os_integration::foreground_is_excluded(&exclusions) {
            return Ok(());
        }
    }

    // Notify UI that operation started (visual feedback)
    let _ = app.emit("translate_replace_started", ());

    // Snapshot the clipboard (text or image) so we can restore it afterwards.
    let saved_clipboard = os_integration::snapshot_clipboard();

    let source_text = tokio::task::spawn_blocking(
        os_integration::copy_selection_to_clipboard
    )
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    if source_text.trim().is_empty() {
        let _ = app.emit("translate_replace_done", serde_json::json!({ "ok": false, "reason": "no selection" }));
        return Ok(());
    }

    // Get glossary from settings
    let glossary: Vec<(String, String)> = {
        let s = state.settings.lock().await;
        let pair = format!("{source_lang}->{target_lang}");
        s.glossary.iter()
            .filter(|e| e.lang_pair == pair || e.lang_pair == "auto")
            .map(|e| (e.source.clone(), e.target.clone()))
            .collect()
    };

    let resolved_source = if source_lang == "auto" {
        detect_language_internal(&source_text)
    } else {
        source_lang.clone()
    };
    let resolved_target = if target_lang == "auto" {
        let priority = state.settings.lock().await.auto_target_priority.clone();
        auto_target(&resolved_source, &priority)
    } else {
        target_lang.clone()
    };

    let req = TranslationRequest {
        source_text: source_text.clone(),
        source_lang: resolved_source,
        target_lang: resolved_target,
        context: None,
        glossary,
        version: active_model_version(&state).await,
        mode: "standard".to_string(),
        style: None,
    };

    let result = match state.engine.translate(req).await {
        Ok(r) => r,
        Err(e) => {
            let _ = app.emit("translate_replace_done", serde_json::json!({ "ok": false, "reason": e.to_string() }));
            return Err(e.to_string());
        }
    };

    let translation = result.translated_text.clone();

    // Write translation to clipboard and paste
    os_integration::write_clipboard(&translation).map_err(|e| e.to_string())?;

    tokio::task::spawn_blocking(os_integration::paste_from_clipboard)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    // Restore the original clipboard after a delay (user asked to replace, not copy).
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
    os_integration::restore_clipboard(saved_clipboard);

    let _ = app.emit("translate_replace_done", serde_json::json!({ "ok": true }));
    Ok(())
}

/// Quick translate for the floating button: translates given text and returns result.
#[tauri::command]
async fn quick_translate(
    source_text: String,
    source_lang: String,
    target_lang: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let req = TranslationRequest {
        source_text: source_text.clone(),
        source_lang: if source_lang == "auto" {
            detect_language_internal(&source_text)
        } else {
            source_lang
        },
        target_lang,
        context: None,
        glossary: vec![],
        version: active_model_version(&state).await,
        mode: "standard".to_string(),
        style: None,
    };
    let result = state.engine.translate(req).await.map_err(|e| e.to_string())?;
    Ok(result.translated_text)
}

/// Copies the user's current selection (Ctrl+C) and translates it. Called when
/// the floating button is clicked — i.e. only when the user actually commits to
/// translating, so the selection-wiping copy never happens on mere selection.
#[tauri::command]
async fn translate_selection(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    // Explicit user action (button click) — allow the Ctrl+C capture fallback.
    let text = tokio::task::spawn_blocking(os_integration::capture_selection)
        .await
        .map_err(|e| e.to_string())?;

    // Only translate what Ctrl+C actually copied. We must NOT fall back to the
    // existing clipboard contents — doing so would "translate the last copied
    // text" when nothing is selected (e.g. clicking the button after an empty
    // double-click), which is exactly the confusing behaviour to avoid.
    let text = match text {
        Some(t) if !t.trim().is_empty() => t,
        _ => return Err("Нет выделенного текста для перевода".into()),
    };

    let src = detect_language_internal(&text);
    let priority = state.settings.lock().await.auto_target_priority.clone();
    let tgt = auto_target(&src, &priority);

    let req = TranslationRequest {
        source_text: text.clone(),
        source_lang: src.clone(),
        target_lang: tgt.clone(),
        context: None,
        glossary: vec![],
        version: active_model_version(&state).await,
        mode: "standard".to_string(),
        style: None,
    };
    let result = state.engine.translate(req).await.map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "translated_text": result.translated_text,
        "source_lang": src,
        "target_lang": tgt,
    }))
}

/// Toggle whether the floating button feature is active.
#[tauri::command]
async fn set_floating_enabled(
    enabled: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let mut f = state.floating_enabled.lock().await;
    *f = enabled;
    if !enabled {
        os_integration::hide_floating(&app);
    }
    Ok(())
}

/// Called from mouse_selection_released event handler in frontend:
/// shows or hides the floating button based on current clipboard content.
#[tauri::command]
async fn check_and_show_floating(
    x: f64,
    y: f64,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let enabled = *state.floating_enabled.lock().await;
    if !enabled {
        return Ok(());
    }

    // Try to read selected text from clipboard (non-destructive: we only read, don't write)
    // A small delay first so the OS has time to process the mouse release
    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

    let text = match os_integration::read_clipboard() {
        Ok(t) if !t.trim().is_empty() => t,
        _ => {
            os_integration::hide_floating(&app);
            return Ok(());
        }
    };

    os_integration::show_floating(&app, x, y).map_err(|e| e.to_string())?;

    // Send the selected text to the floating window
    let _ = app.emit_to("floating", "floating_text", serde_json::json!({ "text": text }));

    Ok(())
}

#[tauri::command]
async fn hide_floating_button(app: AppHandle) -> Result<(), String> {
    os_integration::hide_floating(&app);
    Ok(())
}

/// Replace the source app's current selection with the given text. The floating
/// window is WS_EX_NOACTIVATE, so the source app still holds the selection —
/// we write the text to the clipboard and paste it (Ctrl+V), then restore the
/// original clipboard. Called by the floating "Replace" button.
#[tauri::command]
async fn floating_replace(text: String) -> Result<(), String> {
    let saved = os_integration::snapshot_clipboard();
    os_integration::write_clipboard(&text).map_err(|e| e.to_string())?;
    tokio::task::spawn_blocking(|| {
        // Bring the source app back to the foreground before pasting, so Ctrl+V
        // lands in it and not in our floating webview.
        os_integration::focus_source_window();
        os_integration::paste_from_clipboard()
    })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
    os_integration::restore_clipboard(saved);
    Ok(())
}

/// Resize the floating window to show/hide the translation card.
#[tauri::command]
async fn set_floating_expanded(expanded: bool, app: AppHandle) -> Result<(), String> {
    os_integration::set_floating_expanded(&app, expanded).map_err(|e| e.to_string())
}

/// GPU capability of this machine, for the Settings GPU toggle:
/// - cuda_ready: GPU mode will actually work (backend + deps resolvable).
/// - nvidia_present: an NVIDIA driver is installed at all.
#[tauri::command]
async fn gpu_status() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "cuda_ready": cuda_available(),
        "nvidia_present": nvidia_gpu_present(),
    }))
}

// ── OCR (screenshot translation) ─────────────────────────────────────────────

/// Whether the bundled Tesseract is usable right now.
#[tauri::command]
async fn ocr_status() -> Result<bool, String> {
    tokio::task::spawn_blocking(os_integration::ocr::engine_status)
        .await
        .map_err(|e| e.to_string())
}

/// Downloads a Tesseract language pack into the writable tessdata dir (jsdelivr,
/// the small "fast" data). Returns true on success.
async fn ensure_lang(app: &AppHandle, code: &str) -> bool {
    if os_integration::ocr::is_lang_installed(code) {
        return true;
    }
    let _ = app.emit("ocr_lang_downloading", serde_json::json!({ "code": code }));
    let url = format!("https://cdn.jsdelivr.net/gh/tesseract-ocr/tessdata_fast@main/{code}.traineddata");
    let dest = os_integration::ocr::tessdata_user_dir().join(format!("{code}.traineddata"));
    let ok = match reqwest::get(&url).await {
        Ok(r) if r.status().is_success() => match r.bytes().await {
            Ok(b) if !b.is_empty() => std::fs::write(&dest, &b).is_ok(),
            _ => false,
        },
        _ => false,
    };
    let _ = app.emit("ocr_lang_downloaded", serde_json::json!({ "code": code, "ok": ok }));
    ok
}

/// Decides which Tesseract languages to OCR with.
///
/// Returns `(primary, Some(secondary))` when the page is a non-Latin/Cyrillic
/// script (Chinese, Japanese, …) AND the user also has Latin/Cyrillic languages
/// enabled: the caller then runs a two-pass merge so the Chinese body stays
/// clean while a Russian/English title is still recovered. A single
/// `chi_sim+rus` pass instead mixes scripts mid-line. Otherwise returns
/// `(lang_arg, None)` for an ordinary single pass.
async fn resolve_ocr_langs(
    app: &AppHandle,
    enabled: Vec<String>,
    auto: bool,
    detected: Option<String>,
) -> (String, Option<String>) {
    let installed = |c: &str| os_integration::ocr::is_lang_installed(c);

    if auto {
        if let Some(d) = detected {
            if ensure_lang(app, &d).await {
                if d != "eng" && d != "rus" {
                    // Non-Latin/Cyrillic dominant script. Secondary = the user's
                    // other enabled (installed) languages, for a confidence merge.
                    let secondary: Vec<String> = enabled
                        .into_iter()
                        .filter(|l| l != &d && installed(l))
                        .collect();
                    let secondary = if secondary.is_empty() {
                        None
                    } else {
                        Some(secondary.join("+"))
                    };
                    logging::info("ocr", &format!("OCR plan: primary '{d}', secondary {secondary:?}"));
                    return (d, secondary);
                }
                // Latin/Cyrillic detected: combine with the enabled set (these
                // coexist fine in one pass).
                let mut langs = vec![d];
                for l in enabled {
                    if !langs.contains(&l) {
                        langs.push(l);
                    }
                }
                langs.retain(|c| installed(c));
                if langs.is_empty() {
                    langs.push("eng".to_string());
                }
                let joined = langs.join("+");
                logging::info("ocr", &format!("OCR plan: '{joined}' (latin/cyrillic detected)"));
                return (joined, None);
            }
        }
    }

    let mut langs: Vec<String> = Vec::new();
    for l in enabled {
        if !langs.contains(&l) {
            langs.push(l);
        }
    }
    langs.retain(|c| installed(c));
    if langs.is_empty() {
        langs.push("eng".to_string());
    }
    let joined = langs.join("+");
    logging::info("ocr", &format!("OCR plan: '{joined}' (no detection)"));
    (joined, None)
}

async fn ocr_lang_opts(state: &AppState) -> (Vec<String>, bool) {
    let s = state.settings.lock().await;
    (s.ocr_languages.clone(), s.ocr_auto_lang)
}

/// OCR the image currently on the clipboard (a screenshot) → normalized text.
#[tauri::command]
async fn ocr_from_clipboard(state: State<'_, AppState>, app: AppHandle) -> Result<String, String> {
    let (enabled, auto) = ocr_lang_opts(&state).await;
    let detected = if auto {
        tokio::task::spawn_blocking(os_integration::ocr::detect_clipboard_script)
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let (primary, secondary) = resolve_ocr_langs(&app, enabled, auto, detected).await;
    let text = tokio::task::spawn_blocking(move || match secondary {
        Some(sec) => os_integration::ocr::recognize_clipboard_merged(&primary, &sec),
        None => os_integration::ocr::recognize_clipboard(&primary),
    })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(core::ocr_normalize::normalize_ocr_text(&text))
}

/// OCR an image file from disk → normalized text.
#[tauri::command]
async fn ocr_from_file(path: String, state: State<'_, AppState>, app: AppHandle) -> Result<String, String> {
    let (enabled, auto) = ocr_lang_opts(&state).await;
    let detected = if auto {
        let p = path.clone();
        tokio::task::spawn_blocking(move || os_integration::ocr::detect_file_script(&p))
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let (primary, secondary) = resolve_ocr_langs(&app, enabled, auto, detected).await;
    let text = tokio::task::spawn_blocking(move || match secondary {
        Some(sec) => os_integration::ocr::recognize_file_merged(&path, &primary, &sec),
        None => os_integration::ocr::recognize_file(&path, &primary),
    })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(core::ocr_normalize::normalize_ocr_text(&text))
}

/// Status of every offered OCR language: code, display name, installed, enabled.
#[tauri::command]
async fn ocr_langs_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let enabled = state.settings.lock().await.ocr_languages.clone();
    let rows: Vec<serde_json::Value> = os_integration::ocr::SUPPORTED_LANGS
        .iter()
        .map(|(code, name)| {
            serde_json::json!({
                "code": code,
                "name": name,
                "installed": os_integration::ocr::is_lang_installed(code),
                "enabled": enabled.iter().any(|e| e == code),
                "bundled": *code == "eng" || *code == "rus",
            })
        })
        .collect();
    Ok(serde_json::json!(rows))
}

/// Download a language pack (Settings UI).
#[tauri::command]
async fn ocr_lang_download(code: String, app: AppHandle) -> Result<bool, String> {
    Ok(ensure_lang(&app, &code).await)
}

/// Remove a downloaded language pack (bundled eng/rus can't be removed).
#[tauri::command]
async fn ocr_lang_remove(code: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || os_integration::ocr::remove_lang(&code))
        .await
        .map_err(|e| e.to_string())?)
}

/// Hidden OCR diagnostic: sweep installed data sets x PSM on one image, with raw
/// + normalized text, timing and model per row.
#[tauri::command]
async fn ocr_test_all(path: String) -> Result<serde_json::Value, String> {
    let results = tokio::task::spawn_blocking(move || os_integration::ocr::ocr_test_all(&path))
        .await
        .map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = results
        .into_iter()
        .map(|r| {
            let normalized = core::ocr_normalize::normalize_ocr_text(&r.text);
            serde_json::json!({
                "engine": r.engine, "model": r.model, "preprocess": r.preprocess,
                "ms": r.ms, "text": r.text, "normalized": normalized, "error": r.error,
            })
        })
        .collect();
    Ok(serde_json::json!(rows))
}

/// Launches the built-in Windows region snipping tool (Win+Shift+S). The user
/// snips an area, the screenshot lands on the clipboard → then ocr_from_clipboard.
#[tauri::command]
async fn launch_snip() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Clear the clipboard first so polling can detect the NEW snip and not
        // re-OCR a stale image.
        let _ = os_integration::write_clipboard("");
        std::process::Command::new("explorer")
            .arg("ms-screenclip:")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Opens a URL in the user's default browser (used by the About page links).
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = url;
    }
    Ok(())
}

/// Records an error/info line from the frontend into the shared log file.
#[tauri::command]
fn log_event(level: String, source: String, message: String) {
    logging::log(&level, &source, &message);
}

/// Returns the tail of the log file for the "Report a problem" view.
#[tauri::command]
fn read_log() -> String {
    logging::tail(64 * 1024)
}

/// Opens the folder containing the log file in the system file manager.
#[tauri::command]
async fn open_log_folder() -> Result<(), String> {
    let dir = logging::log_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .arg(&dir)
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
    }
    Ok(())
}

/// Lists executable names of apps with a visible window, for the exclusion picker.
#[tauri::command]
async fn list_app_processes() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(os_integration::list_app_processes)
        .await
        .map_err(|e| e.to_string())
}

// ── Model file management commands ───────────────────────────────────────────

#[tauri::command]
async fn list_downloaded_models(state: State<'_, AppState>) -> Result<Vec<(String, String, String)>, String> {
    let model_dir = state.settings.lock().await.model_path.clone();
    Ok(state.model_manager.list_downloaded(&model_dir))
}

#[tauri::command]
async fn delete_model(
    version: String,
    size: String,
    quantization: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let model_dir = state.settings.lock().await.model_path.clone();
    state.model_manager.delete_model_file(&model_dir, &version, &size, &quantization).map_err(|e| e.to_string())?;
    // If the just-deleted model was active, reset status
    let is_active = state.model_manager.current_spec.lock().await
        .as_ref()
        .map_or(false, |s| s.version == version && s.size == size && s.quantization == quantization);
    if is_active {
        *state.model_manager.status.lock().await = ModelStatus::NotDownloaded;
        *state.model_manager.current_spec.lock().await = None;
        let _ = app.emit("model_removed", ());
    }
    Ok(())
}

/// Current download progress (if any) — lets the UI resume after a tab switch.
#[tauri::command]
async fn get_download_state(state: State<'_, AppState>) -> Result<Option<DownloadState>, String> {
    Ok(state.model_manager.get_download_state())
}

/// Loads a specific downloaded variant as the active model and (re)starts the
/// engine on it. This is what the per-variant "Load" button calls — it actually
/// switches the active model, unlike restart_engine which reloads the current one.
#[tauri::command]
async fn load_model(
    version: String,
    size: String,
    quantization: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let model_dir = state.settings.lock().await.model_path.clone();
    let path = state.model_manager
        .local_file(&model_dir, &version, &size, &quantization)
        .ok_or("Файл модели не найден — скачайте её сначала.")?;

    // Persist as the active model so it survives restarts.
    {
        let mut s = state.settings.lock().await;
        s.model_version = version.clone();
        s.model_size = size.clone();
        s.quantization = quantization.clone();
        let snapshot = s.clone();
        drop(s);
        let _ = persist_settings(&snapshot);
    }
    *state.model_manager.current_spec.lock().await = Some(ModelSpec {
        version: version.clone(),
        size: size.clone(),
        quantization: quantization.clone(),
    });
    *state.model_manager.status.lock().await =
        ModelStatus::Ready { path: path.to_string_lossy().to_string() };

    // Start the engine in the background; the UI waits for model_ready/error.
    let engine = Arc::clone(&state.engine);
    let app_c = app.clone();
    tokio::spawn(async move {
        match engine.start(path).await {
            Ok(()) => {
                let _ = app_c.emit("model_ready", ());
                os_integration::tray::update_tray_model_status(&app_c, "model ready");
            }
            Err(e) => {
                let _ = app_c.emit("model_error", e.to_string());
            }
        }
    });
    Ok(())
}

/// Loads an arbitrary .gguf model from disk (e.g. a finetune the user downloaded
/// elsewhere). The engine is started on it and it becomes the active model.
#[tauri::command]
async fn load_external_model(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let is_gguf = p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !p.exists() || !is_gguf {
        return Err("Укажите существующий файл .gguf".into());
    }

    // External model has no size/quant spec.
    *state.model_manager.current_spec.lock().await = None;
    *state.model_manager.status.lock().await =
        ModelStatus::Ready { path: path.clone() };

    let engine = Arc::clone(&state.engine);
    let app_c = app.clone();
    tokio::spawn(async move {
        match engine.start(p).await {
            Ok(()) => {
                let _ = app_c.emit("model_ready", ());
                os_integration::tray::update_tray_model_status(&app_c, "model ready");
            }
            Err(e) => {
                let _ = app_c.emit("model_error", e.to_string());
            }
        }
    });
    Ok(())
}

// ── Engine restart command ───────────────────────────────────────────────────

#[tauri::command]
async fn restart_engine(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let (model_dir, sv, ss, sq) = {
        let s = state.settings.lock().await;
        (s.model_path.clone(), s.model_version.clone(), s.model_size.clone(), s.quantization.clone())
    };

    let mm = &state.model_manager;
    // Settings model → currently-loaded spec → any downloaded model.
    let path = if let Some(p) = mm.local_file(&model_dir, &sv, &ss, &sq) {
        p
    } else {
        let current = mm.current_spec.lock().await.clone();
        let spec_path = current
            .and_then(|s| mm.local_file(&model_dir, &s.version, &s.size, &s.quantization));
        if let Some(p) = spec_path {
            p
        } else {
            match mm.list_downloaded(&model_dir).into_iter().next() {
                Some((v, s, q)) => mm
                    .local_file(&model_dir, &v, &s, &q)
                    .ok_or("Модель не найдена — скачайте модель в менеджере моделей.")?,
                None => return Err("Модель не найдена — скачайте модель в менеджере моделей.".into()),
            }
        }
    };

    // Spawn in background so the command returns immediately.
    // The frontend gets model_ready / model_error events when the engine is up.
    let engine = Arc::clone(&state.engine);
    let app_clone = app.clone();
    tokio::spawn(async move {
        match engine.start(path).await {
            Ok(()) => {
                let _ = app_clone.emit("model_ready", ());
                os_integration::tray::update_tray_model_status(&app_clone, "model ready");
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = app_clone.emit("model_error", &msg);
            }
        }
    });

    Ok(())
}

// ── Autostart Command ────────────────────────────────────────────────────────

#[tauri::command]
async fn set_autostart(
    enabled: bool,
    app: AppHandle,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|e| e.to_string())?;
    } else {
        autostart.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

// ── App Setup ─────────────────────────────────────────────────────────────────

pub fn run() {
    env_logger::init();

    let settings = load_settings().unwrap_or_default();
    let use_gpu = settings.use_gpu;
    let model_path_str = settings.model_path.clone();
    let model_version = settings.model_version.clone();
    let model_size = settings.model_size.clone();
    let model_quant = settings.quantization.clone();
    let show_floating = settings.show_floating_button;
    let start_in_tray = settings.start_in_tray;
    let global_hotkeys = settings.global_hotkeys;
    let shortcut_cfg = ShortcutConfig::from_settings(&settings);

    let history_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeepM")
        .join("history.json");

    let engine = Arc::new(TranslationEngine::new(use_gpu));
    let model_manager = Arc::new(ModelManager::new());
    let cursor_pos: Arc<StdMutex<(f64, f64)>> = Arc::new(StdMutex::new((0.0, 0.0)));
    let main_focused = Arc::new(AtomicBool::new(false));
    let hook_config = Arc::new(os_integration::SharedHookConfig::new(
        shortcut_cfg.translate_replace.clone(),
        shortcut_cfg.triple_copy_interval_ms,
        shortcut_cfg.triple_copy_count,
    ));

    let state = AppState {
        settings: Mutex::new(settings),
        engine: Arc::clone(&engine),
        model_manager: Arc::clone(&model_manager),
        history: Mutex::new(TranslationHistory::load(history_path)),
        floating_enabled: Mutex::new(show_floating),
        last_cursor: Arc::clone(&cursor_pos),
        main_window_focused: Arc::clone(&main_focused),
        selection_gen: Arc::new(AtomicU64::new(0)),
        hook_config: Arc::clone(&hook_config),
    };

    tauri::Builder::default()
        // single-instance MUST be the first plugin. A second launch just brings
        // the existing window to the front instead of starting another copy.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        // Remembers the main window's size & position across restarts (the
        // floating popup is positioned at the cursor, so it's excluded).
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["floating"])
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();

            logging::info("app", &format!("DeepM {} started", env!("CARGO_PKG_VERSION")));

            // Tell the OCR layer where bundled Tesseract / RapidOCR models live.
            #[cfg(target_os = "windows")]
            os_integration::ocr::set_resource_dir(
                handle.path().resource_dir().unwrap_or_default(),
            );

            // Tray
            if let Err(e) = os_integration::setup_tray(&handle, show_floating) {
                log::warn!("Tray setup failed: {e}");
            }

            // Floating button window — created only when the floating button is
            // enabled. (Gated separately from the hook so each can be isolated:
            // a transparent always-on-top layered window is itself a known cause
            // of GDI caret artifacts in classic apps like Notepad.)
            if show_floating {
                if let Err(e) = os_integration::create_floating_window(&handle) {
                    log::warn!("Floating window creation failed: {e}");
                }
            }

            // Intercept main window close: hide to tray instead of destroying.
            // Also track focus so the floating button doesn't appear when user selects
            // text inside the main DeepM window.
            {
                let h = handle.clone();
                let focused_flag = Arc::clone(&main_focused);
                if let Some(main_win) = handle.get_webview_window("main") {
                    main_win.on_window_event(move |event| {
                        match event {
                            tauri::WindowEvent::CloseRequested { api, .. } => {
                                api.prevent_close();
                                if let Some(w) = h.get_webview_window("main") {
                                    let _ = w.hide();
                                }
                            }
                            tauri::WindowEvent::Focused(is_focused) => {
                                focused_flag.store(*is_focused, Ordering::Relaxed);
                            }
                            _ => {}
                        }
                    });
                }
            }

            // Start-in-tray: hide main window if configured
            if start_in_tray {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            // Probe for existing model and start engine
            {
                let manager = Arc::clone(&model_manager);
                let eng = Arc::clone(&engine);
                let h = handle.clone();
                let mp = model_path_str.clone();
                let mv = model_version.clone();
                let ms = model_size.clone();
                let mq = model_quant.clone();

                tauri::async_runtime::spawn(async move {
                    // Try saved model first; fall back to any downloaded model
                    let found = if manager.probe(&mp, &mv, &ms, &mq).await {
                        Some((mv.clone(), ms.clone(), mq.clone()))
                    } else {
                        manager.list_downloaded(&mp).into_iter().next()
                    };

                    if let Some((version, size, quant)) = found {
                        // Ensure status is marked Ready (covers the fallback path where
                        // probe() was not called for the found model).
                        manager.probe(&mp, &version, &size, &quant).await;
                        let path = match manager.local_file(&mp, &version, &size, &quant) {
                            Some(p) => p,
                            None => return,
                        };
                        match eng.start(path).await {
                            Ok(()) => {
                                let _ = h.emit("model_ready", ());
                                os_integration::tray::update_tray_model_status(&h, "model ready");
                            }
                            Err(e) => {
                                log::warn!("Auto-start engine failed: {e}");
                                let _ = h.emit("model_error", e.to_string());
                            }
                        }
                    }
                });
            }

            // Spawn global keyboard/mouse hook (reads live config). Skipped when
            // the user disables global hotkeys — this also lets us isolate the
            // hook as the cause of input-side side effects (e.g. caret artifacts
            // in some GDI apps), since with it off no system-wide hook exists.
            if global_hotkeys {
                os_integration::spawn_hook(handle.clone(), Arc::clone(&hook_config));
            } else {
                logging::info("hook", "global hotkeys disabled by settings — hook not installed");
            }

            // Listen for hook events and dispatch to commands
            {
                let h = handle.clone();
                handle.listen("hotkey_triple_copy", move |_| {
                    let h = h.clone();
                    tauri::async_runtime::spawn(async move {
                        // Respect the per-app exclusion list.
                        let exclusions = h.state::<AppState>().settings.lock().await
                            .floating_exclusions.clone();
                        if os_integration::foreground_is_excluded(&exclusions) { return; }

                        // Slight delay to let the last Ctrl+C settle
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        let text = os_integration::read_clipboard().unwrap_or_default();
                        if !text.is_empty() {
                            let _ = h.emit("insert_text", serde_json::json!({ "text": text }));
                            if let Some(win) = h.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    });
                });
            }

            // cursor_move: update last known cursor position
            {
                let cursor = Arc::clone(&cursor_pos);
                handle.listen("cursor_move", move |event| {
                    if let Ok(pos) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                        let x = pos["x"].as_f64().unwrap_or(0.0);
                        let y = pos["y"].as_f64().unwrap_or(0.0);
                        *cursor.lock().unwrap_or_else(|e| e.into_inner()) = (x, y);
                    }
                });
            }

            // mouse_selection_released: decide whether to show/hide floating button.
            // Payload now contains: { has_selection: bool, x: f64, y: f64 }
            {
                let h = handle.clone();
                handle.listen("mouse_selection_released", move |event| {
                    let payload = serde_json::from_str::<serde_json::Value>(event.payload()).ok();
                    let has_selection = payload.as_ref()
                        .and_then(|v| v["has_selection"].as_bool())
                        .unwrap_or(false);
                    let text_cursor = payload.as_ref()
                        .and_then(|v| v["text_cursor"].as_bool())
                        .unwrap_or(false);
                    let click_x = payload.as_ref()
                        .and_then(|v| v["x"].as_f64())
                        .unwrap_or(0.0);
                    let click_y = payload.as_ref()
                        .and_then(|v| v["y"].as_f64())
                        .unwrap_or(0.0);

                    // Bump generation synchronously and in event order. Any older
                    // pending delayed-show will see a newer value and bail out.
                    let gen_arc = h.state::<AppState>().selection_gen.clone();
                    let my_gen = gen_arc.fetch_add(1, Ordering::SeqCst) + 1;

                    let h = h.clone();
                    tauri::async_runtime::spawn(async move {
                        if !has_selection {
                            // Did the click land inside the floating window (button or card)?
                            // If so, the user is interacting with it — React handles the click,
                            // we must not hide. The window is now sized tightly to its content,
                            // so this rect closely matches the visible button/card.
                            let on_floating = h.get_webview_window("floating").map_or(false, |fw| {
                                let visible = os_integration::is_floating_visible(&h);
                                if !visible { return false; }
                                match (fw.outer_position(), fw.outer_size()) {
                                    (Ok(pos), Ok(sz)) => {
                                        let fx = pos.x as f64;
                                        let fy = pos.y as f64;
                                        click_x >= fx && click_x <= fx + sz.width as f64
                                            && click_y >= fy && click_y <= fy + sz.height as f64
                                    }
                                    _ => false,
                                }
                            });

                            if !on_floating {
                                os_integration::hide_floating(&h);
                            }
                            return;
                        }

                        // If the selection was made INSIDE our own floating popup,
                        // do nothing — the user is selecting part of the translation
                        // to copy it. We must not re-trigger the button there.
                        let in_floating = h.get_webview_window("floating").map_or(false, |fw| {
                            if !os_integration::is_floating_visible(&h) { return false; }
                            match (fw.outer_position(), fw.outer_size()) {
                                (Ok(pos), Ok(sz)) => {
                                    let fx = pos.x as f64;
                                    let fy = pos.y as f64;
                                    click_x >= fx && click_x <= fx + sz.width as f64
                                        && click_y >= fy && click_y <= fy + sz.height as f64
                                }
                                _ => false,
                            }
                        });
                        if in_floating { return; }

                        // Skip if user is working inside the main DeepM window
                        let is_focused = h.state::<AppState>()
                            .main_window_focused.load(Ordering::Relaxed);
                        if is_focused { return; }

                        let enabled = *h.state::<AppState>().floating_enabled.lock().await;
                        if !enabled { return; }

                        // Skip apps the user excluded (e.g. terminals where copy works oddly).
                        let exclusions = h.state::<AppState>().settings.lock().await
                            .floating_exclusions.clone();
                        if os_integration::foreground_is_excluded(&exclusions) { return; }

                        let cursor = h.state::<AppState>().last_cursor.clone();
                        let (x, y) = *cursor.lock().unwrap_or_else(|e| e.into_inner());

                        // Debounce: wait a moment, then proceed only if no newer mouse
                        // event arrived meanwhile. Prevents flashing when the user makes
                        // a selection and immediately clicks it away.
                        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
                        if gen_arc.load(Ordering::SeqCst) != my_gen {
                            return;
                        }

                        // Verify there's a real text selection — WITHOUT touching the
                        // clipboard (UIA only). If UIA has nothing but the gesture was
                        // over text (I-beam cursor), show the button anyway and let the
                        // click capture the text via Ctrl+C — so merely selecting never
                        // clobbers the user's clipboard.
                        let uia = tokio::task::spawn_blocking(os_integration::get_selected_text)
                            .await
                            .ok()
                            .flatten();
                        let (text, capture) = match uia {
                            Some(t) if !t.trim().is_empty() => (t, false),
                            _ if text_cursor => (String::new(), true),
                            _ => return, // not a text selection — don't show the button
                        };
                        if gen_arc.load(Ordering::SeqCst) != my_gen {
                            return;
                        }

                        // Detect language + direction when we already have the text;
                        // otherwise defaults (the click path returns the real langs).
                        let (src, tgt) = if capture {
                            ("auto".to_string(), "auto".to_string())
                        } else {
                            let s = detect_language_internal(&text);
                            let priority = h.state::<AppState>().settings.lock().await
                                .auto_target_priority.clone();
                            let t = auto_target(&s, &priority);
                            (s, t)
                        };

                        // Remember the source app so the "Replace" button can
                        // paste the translation back into it.
                        os_integration::remember_source_window();

                        if let Err(e) = os_integration::show_floating(&h, x, y) {
                            log::debug!("show_floating failed: {e}");
                            return;
                        }
                        let _ = h.emit_to("floating", "floating_show", serde_json::json!({
                            "text": text,
                            "source_lang": src,
                            "target_lang": tgt,
                            "capture": capture,
                        }));
                    });
                });
            }

            // key_hide_floating: hide floating button when user types after selecting
            {
                let h = handle.clone();
                handle.listen("key_hide_floating", move |_| {
                    os_integration::hide_floating(&h);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Stage 1
            get_model_status,
            start_model_download,
            cancel_model_download,
            translate,
            detect_language,
            get_settings,
            save_settings,
            get_history,
            clear_history,
            // Stage 2
            handle_triple_copy,
            translate_and_replace,
            quick_translate,
            translate_selection,
            set_floating_enabled,
            check_and_show_floating,
            hide_floating_button,
            floating_replace,
            set_floating_expanded,
            list_app_processes,
            gpu_status,
            ocr_status,
            ocr_from_clipboard,
            ocr_from_file,
            ocr_test_all,
            ocr_langs_status,
            ocr_lang_download,
            ocr_lang_remove,
            launch_snip,
            open_url,
            log_event,
            read_log,
            open_log_folder,
            set_autostart,
            get_autostart,
            restart_engine,
            list_downloaded_models,
            delete_model,
            get_download_state,
            load_model,
            load_external_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DeepM");
}
