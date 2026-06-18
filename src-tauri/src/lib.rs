use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tokio::sync::Mutex;
use std::sync::Mutex as StdMutex;

mod config;
mod core;
mod os_integration;
mod shortcuts;

use config::{AppSettings, load_settings, save_settings as persist_settings};
use core::{
    engine::{TranslationEngine, TranslationRequest},
    history::{HistoryEntry, TranslationHistory},
    model_manager::{ModelManager, ModelStatus},
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
}

// ── Translation Commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn get_model_status(state: State<'_, AppState>) -> Result<ModelStatus, String> {
    Ok(state.model_manager.get_status().await)
}

#[tauri::command]
async fn start_model_download(
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

    tokio::spawn(async move {
        let result = manager
            .download(
                &model_dir,
                &size,
                &quantization,
                move |progress, speed_mbps| {
                    let _ = app_clone.emit(
                        "download_progress",
                        serde_json::json!({ "progress": progress, "speed_mbps": speed_mbps }),
                    );
                },
            )
            .await;

        match result {
            Ok(path) => {
                match engine.start(path).await {
                    Ok(()) => {
                        let _ = app.emit("model_ready", ());
                        os_integration::tray::update_tray_model_status(&app, "model ready");
                    }
                    Err(e) => {
                        log::error!("Engine start failed: {e}");
                        let _ = app.emit("model_error", e.to_string());
                        os_integration::tray::update_tray_model_status(&app, "engine error");
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("cancelled") {
                    log::info!("Download cancelled by user");
                } else {
                    log::error!("Download failed: {e}");
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

#[tauri::command]
async fn translate(
    source_text: String,
    source_lang: String,
    target_lang: String,
    context: Option<String>,
    glossary_entries: Option<Vec<serde_json::Value>>,
    formatted: Option<bool>,
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
        formatted: formatted.unwrap_or(false),
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
    use whatlang::Lang;
    whatlang::detect(text)
        .map(|i| lang_to_code(i.lang()))
        .unwrap_or_else(|| "en".to_string())
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
    // Notify UI that operation started (visual feedback)
    let _ = app.emit("translate_replace_started", ());

    // Run blocking clipboard/enigo work on a blocking thread so we don't block Tokio
    let saved_clipboard = os_integration::save_clipboard();

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

    let req = TranslationRequest {
        source_text: source_text.clone(),
        source_lang: if source_lang == "auto" {
            detect_language_internal(&source_text)
        } else {
            source_lang.clone()
        },
        target_lang: target_lang.clone(),
        context: None,
        glossary,
        formatted: false,
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

    // Restore original clipboard after a delay (user didn't ask to copy — they asked to replace)
    if let Some(original) = saved_clipboard {
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
        let _ = os_integration::write_clipboard(&original);
    }

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
        formatted: false,
    };
    let result = state.engine.translate(req).await.map_err(|e| e.to_string())?;
    Ok(result.translated_text)
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

// ── Model file management commands ───────────────────────────────────────────

#[tauri::command]
async fn list_downloaded_models(state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    let model_dir = state.settings.lock().await.model_path.clone();
    Ok(state.model_manager.list_downloaded(&model_dir))
}

#[tauri::command]
async fn delete_model(
    size: String,
    quantization: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let model_dir = state.settings.lock().await.model_path.clone();
    state.model_manager.delete_model_file(&model_dir, &size, &quantization).map_err(|e| e.to_string())?;
    // If the just-deleted model was active, reset status
    let is_active = state.model_manager.current_spec.lock().await
        .as_ref()
        .map_or(false, |s| s.size == size && s.quantization == quantization);
    if is_active {
        *state.model_manager.status.lock().await = ModelStatus::NotDownloaded;
        *state.model_manager.current_spec.lock().await = None;
        let _ = app.emit("model_removed", ());
    }
    Ok(())
}

// ── Engine restart command ───────────────────────────────────────────────────

#[tauri::command]
async fn restart_engine(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let settings = state.settings.lock().await;
    let model_dir = settings.model_path.clone();
    let size = settings.model_size.clone();
    let quant = settings.quantization.clone();
    drop(settings);

    let path = std::path::PathBuf::from(&model_dir)
        .join(format!("HY-MT1.5-{}-{}.gguf", size, quant));

    if !path.exists() {
        return Err("Model file not found — please download the model first.".into());
    }

    match state.engine.start(path).await {
        Ok(()) => {
            let _ = app.emit("model_ready", ());
            os_integration::tray::update_tray_model_status(&app, "model ready");
            Ok(())
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = app.emit("model_error", &msg);
            Err(msg)
        }
    }
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
    let model_size = settings.model_size.clone();
    let model_quant = settings.quantization.clone();
    let show_floating = settings.show_floating_button;
    let start_in_tray = settings.start_in_tray;
    let shortcut_cfg = ShortcutConfig::from_settings(&settings);

    let history_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeepM")
        .join("history.json");

    let engine = Arc::new(TranslationEngine::new(use_gpu));
    let model_manager = Arc::new(ModelManager::new());
    let cursor_pos: Arc<StdMutex<(f64, f64)>> = Arc::new(StdMutex::new((0.0, 0.0)));
    let main_focused = Arc::new(AtomicBool::new(false));

    let state = AppState {
        settings: Mutex::new(settings),
        engine: Arc::clone(&engine),
        model_manager: Arc::clone(&model_manager),
        history: Mutex::new(TranslationHistory::load(history_path)),
        floating_enabled: Mutex::new(show_floating),
        last_cursor: Arc::clone(&cursor_pos),
        main_window_focused: Arc::clone(&main_focused),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();

            // Tray
            if let Err(e) = os_integration::setup_tray(&handle, show_floating) {
                log::warn!("Tray setup failed: {e}");
            }

            // Floating button window
            if let Err(e) = os_integration::create_floating_window(&handle) {
                log::warn!("Floating window creation failed: {e}");
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
                let ms = model_size.clone();
                let mq = model_quant.clone();

                tauri::async_runtime::spawn(async move {
                    // Try saved model first; fall back to any downloaded model
                    let found = if manager.probe(&mp, &ms, &mq).await {
                        Some((ms.clone(), mq.clone()))
                    } else {
                        manager.list_downloaded(&mp).into_iter().next().and_then(|(s, q)| {
                            let path = PathBuf::from(&mp).join(format!("HY-MT1.5-{}-{}.gguf", s, q));
                            if path.exists() { Some((s, q)) } else { None }
                        })
                    };

                    if let Some((size, quant)) = found {
                        // Ensure status is marked Ready (covers the fallback path where
                        // probe() was not called for the found model).
                        manager.probe(&mp, &size, &quant).await;
                        let path = PathBuf::from(&mp).join(
                            format!("HY-MT1.5-{}-{}.gguf", size, quant)
                        );
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

            // Spawn global keyboard/mouse hook
            os_integration::spawn_hook(
                handle.clone(),
                shortcut_cfg.translate_replace.clone(),
                shortcut_cfg.triple_copy_interval_ms,
            );

            // Listen for hook events and dispatch to commands
            {
                let h = handle.clone();
                handle.listen("hotkey_triple_copy", move |_| {
                    let h = h.clone();
                    tauri::async_runtime::spawn(async move {
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
                    let click_x = payload.as_ref()
                        .and_then(|v| v["x"].as_f64())
                        .unwrap_or(0.0);
                    let click_y = payload.as_ref()
                        .and_then(|v| v["y"].as_f64())
                        .unwrap_or(0.0);

                    let h = h.clone();
                    tauri::async_runtime::spawn(async move {
                        if !has_selection {
                            // Check if the click landed inside the floating window.
                            // If yes, the user is interacting with the button — don't hide.
                            let on_floating = h.get_webview_window("floating").map_or(false, |fw| {
                                let visible = fw.is_visible().unwrap_or(false);
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

                        // Skip if user is working inside the main DeepM window
                        let is_focused = h.state::<AppState>()
                            .main_window_focused.load(Ordering::Relaxed);
                        if is_focused { return; }

                        let enabled = *h.state::<AppState>().floating_enabled.lock().await;
                        if !enabled { return; }

                        let cursor = h.state::<AppState>().last_cursor.clone();
                        let (x, y) = *cursor.lock().unwrap_or_else(|e| e.into_inner());

                        // Wait ~1 s: let the user finish their selection before Ctrl+C
                        tokio::time::sleep(tokio::time::Duration::from_millis(950)).await;

                        let copy_result = tokio::task::spawn_blocking(
                            os_integration::copy_selection_to_clipboard
                        ).await;

                        let text = match copy_result {
                            Ok(Ok(t)) if !t.trim().is_empty() => t,
                            _ => {
                                os_integration::hide_floating(&h);
                                return;
                            }
                        };

                        if let Err(e) = os_integration::show_floating(&h, x, y) {
                            log::debug!("show_floating failed: {e}");
                            return;
                        }
                        let _ = h.emit_to("floating", "floating_text",
                            serde_json::json!({ "text": text }));
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
            set_floating_enabled,
            check_and_show_floating,
            hide_floating_button,
            set_autostart,
            get_autostart,
            restart_engine,
            list_downloaded_models,
            delete_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DeepM");
}
