use anyhow::Result;
use tauri::{
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::AppState;

const TRAY_ID: &str = "deepm-tray";

pub fn setup_tray(app: &AppHandle, floating_enabled: bool) -> Result<()> {
    let menu = build_menu(app, floating_enabled)?;

    // The tray uses a dedicated transparent mark (reads well on the dark system
    // tray); the exe/window/installer use the solid app icon.
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/tray.png"))
        .ok()
        .or_else(|| app.default_window_icon().cloned())
        .expect("tray icon");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("DeepM — local translation")
        .on_menu_event({
            let app = app.clone();
            move |_tray, event: MenuEvent| {
                handle_menu_event(&app, &event.id.0);
            }
        })
        .on_tray_icon_event({
            let app = app.clone();
            move |_tray, event| {
                if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = event {
                    show_main_window(&app);
                }
            }
        })
        .build(app)
        .map_err(|e| anyhow::anyhow!("tray build error: {e}"))?;

    Ok(())
}

fn build_menu(app: &AppHandle, floating_enabled: bool) -> Result<Menu<tauri::Wry>> {
    let show_item = MenuItem::with_id(app, "show", "Open DeepM", true, None::<&str>)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let float_label = if floating_enabled { "Floating button: ON" } else { "Floating button: OFF" };
    let toggle_floating = MenuItem::with_id(app, "toggle_floating", float_label, true, None::<&str>)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let sep = PredefinedMenuItem::separator(app)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Menu::with_items(app, &[&show_item, &sep, &toggle_floating, &sep, &quit_item])
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main_window(app),
        "toggle_floating" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let new_val = {
                    let mut f = state.floating_enabled.lock().await;
                    *f = !*f;
                    *f
                };
                if !new_val {
                    crate::os_integration::hide_floating(&app);
                }
                // Persist the new value
                {
                    let mut s = state.settings.lock().await;
                    s.show_floating_button = new_val;
                    let _ = crate::config::save_settings(&*s);
                }
                rebuild_tray_menu(&app, new_val);
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        // `show()` alone does NOT restore a window that's minimized to the
        // taskbar, so unminimize first — otherwise a tray double-click does
        // nothing for a minimized window.
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Rebuilds the tray menu to reflect the current floating-button state.
pub fn rebuild_tray_menu(app: &AppHandle, floating_enabled: bool) {
    if let Ok(menu) = build_menu(app, floating_enabled) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Updates the tray tooltip with the current model status.
pub fn update_tray_model_status(app: &AppHandle, status: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(&format!("DeepM — {status}")));
    }
}

// Kept for call-site compatibility; functionality is now in rebuild_tray_menu.
pub fn update_tray_floating_label(_app: &AppHandle, _enabled: bool) {}
