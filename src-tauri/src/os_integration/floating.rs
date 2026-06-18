use anyhow::Result;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const FLOATING_WINDOW_LABEL: &str = "floating";

// The window is always the full popup size; compact/expanded is CSS-only.
const WINDOW_W: u32 = 300;
const WINDOW_H: u32 = 218; // 52 (btn) + 6 (gap) + 160 (card)

// ── Windows-specific helpers ──────────────────────────────────────────────────
//
// We avoid adding a windows-sys dependency by declaring the two functions we
// need inline.  DwmSetWindowAttribute disables Windows 11's automatic corner
// rounding so our CSS border-radius is the only thing shaping the window.
// ShowWindow(SW_SHOWNA) shows the window without stealing keyboard focus —
// plain win.show() calls SW_SHOW which can disturb console selections.

#[cfg(target_os = "windows")]
mod win32 {
    use ::core::ffi::c_void;

    // HWND in the `windows` crate is #[repr(transparent)] struct HWND(*mut c_void).
    // Our FFI functions match that ABI by using *mut c_void directly.

    // dwmapi.dll
    #[link(name = "dwmapi")]
    extern "system" {
        pub fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            dw_attribute: u32,
            pv_attribute: *const c_void,
            cb_attribute: u32,
        ) -> i32;
    }

    // user32.dll
    #[link(name = "user32")]
    extern "system" {
        pub fn ShowWindow(hwnd: *mut c_void, n_cmd_show: i32) -> i32;
    }

    /// DWMWA_WINDOW_CORNER_PREFERENCE = 33
    pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    /// DWMWCP_DONOTROUND = 1 (disable automatic Windows 11 corner rounding)
    pub const DWMWCP_DONOTROUND: i32 = 1;
    /// SW_SHOWNA = 8 (show without activating)
    pub const SW_SHOWNA: i32 = 8;
}

/// Creates the floating translate button window (hidden at startup).
/// Called once during app setup.
pub fn create_floating_window(app: &AppHandle) -> Result<()> {
    if app.get_webview_window(FLOATING_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let win = WebviewWindowBuilder::new(
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
    // WS_EX_NOACTIVATE — never steal focus when shown or clicked.
    .focused(false)
    .build()
    .map_err(|e| anyhow::anyhow!("floating window build error: {e}"))?;

    // Disable Windows 11 automatic corner rounding.  Without this, the system
    // composites a rounded-rect shape over a white/grey WebView2 background,
    // producing the visible grey corners the user reported.
    // With DWMWCP_DONOTROUND the window has sharp OS-level corners; our CSS
    // border-radius on .fb-btn-wrap and .fb-card provides the visual rounding.
    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                let pref: i32 = win32::DWMWCP_DONOTROUND;
                win32::DwmSetWindowAttribute(
                    hwnd.0,
                    win32::DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const i32 as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
            }
        }
    }

    Ok(())
}

/// Shows the floating button near screen coordinates (x, y).
pub fn show_floating(app: &AppHandle, x: f64, y: f64) -> Result<()> {
    let win = match app.get_webview_window(FLOATING_WINDOW_LABEL) {
        Some(w) => w,
        None => return Ok(()),
    };

    // Offset so the button appears just above/right of the cursor.
    let px = (x + 12.0) as i32;
    let py = (y - 60.0) as i32;

    win.set_position(tauri::PhysicalPosition::new(px, py))
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // On Windows: ShowWindow(SW_SHOWNA) shows without activating the window,
    // so the user's text selection in console/terminal apps is preserved.
    // On other platforms: use the normal show() path.
    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = win.hwnd() {
            unsafe { win32::ShowWindow(hwnd.0, win32::SW_SHOWNA); }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        win.show().map_err(|e| anyhow::anyhow!("{e}"))?;
    }

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
