use anyhow::Result;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const FLOATING_WINDOW_LABEL: &str = "floating";
const WINDOW_W: u32 = 52;
const WINDOW_H: u32 = 52;

/// Creates the floating translate button window (hidden at startup).
/// Called once during app setup.
pub fn create_floating_window(app: &AppHandle) -> Result<()> {
    if app.get_webview_window(FLOATING_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    WebviewWindowBuilder::new(
        app,
        FLOATING_WINDOW_LABEL,
        WebviewUrl::App("/?window=floating".into()),
    )
    .title("")
    .inner_size(WINDOW_W as f64, WINDOW_H as f64)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .transparent(true)
    .shadow(false)
    .build()
    .map_err(|e| anyhow::anyhow!("floating window build error: {e}"))?;

    Ok(())
}

/// Shows the floating button near screen coordinates (x, y).
/// Automatically clamps to keep the window on screen.
pub fn show_floating(app: &AppHandle, x: f64, y: f64) -> Result<()> {
    let win = match app.get_webview_window(FLOATING_WINDOW_LABEL) {
        Some(w) => w,
        None => return Ok(()),
    };

    // Offset slightly so the button appears just above/right of cursor
    let offset_x = 12.0;
    let offset_y = -60.0;

    let px = (x + offset_x) as i32;
    let py = (y + offset_y) as i32;

    win.set_position(tauri::PhysicalPosition::new(px, py))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    win.show().map_err(|e| anyhow::anyhow!("{e}"))?;
    // Intentionally NOT calling set_focus() — the floating window must never
    // steal focus from the application the user is working in.

    Ok(())
}

/// Hides the floating button window.
pub fn hide_floating(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(FLOATING_WINDOW_LABEL) {
        let _ = win.hide();
    }
}

/// Returns true if the floating window is currently visible.
pub fn is_floating_visible(app: &AppHandle) -> bool {
    app.get_webview_window(FLOATING_WINDOW_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}
