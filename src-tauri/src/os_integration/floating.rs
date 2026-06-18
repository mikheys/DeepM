use anyhow::Result;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const FLOATING_WINDOW_LABEL: &str = "floating";

// The window is sized tightly to its content so there is almost no transparent
// "dead" area around the button. CSS keeps the button anchored at the top-left
// with a small margin for its drop-shadow.
//
// Collapsed = just the round button (+ shadow margin).
// Expanded  = button row + translation card below it.
const COLLAPSED_W: u32 = 66;
const COLLAPSED_H: u32 = 66;
const EXPANDED_W: u32 = 320;
const EXPANDED_H: u32 = 300;

// ── Windows-specific helpers ──────────────────────────────────────────────────
//
// We declare the few Win32 functions we need inline to avoid pulling in a
// windows-sys dependency. The key one is SetWindowLongPtrW, used to add
// WS_EX_NOACTIVATE so the window NEVER takes keyboard focus — neither when it
// is shown nor when it is clicked. This is what keeps the user's text selection
// alive in the source app and lets them keep interacting with that app.

#[cfg(target_os = "windows")]
mod win32 {
    use ::core::ffi::c_void;

    // HWND in the `windows` crate is #[repr(transparent)] struct HWND(*mut c_void).

    #[link(name = "dwmapi")]
    extern "system" {
        pub fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            dw_attribute: u32,
            pv_attribute: *const c_void,
            cb_attribute: u32,
        ) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn ShowWindow(hwnd: *mut c_void, n_cmd_show: i32) -> i32;
        pub fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        pub fn GetWindowLongPtrW(hwnd: *mut c_void, n_index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
    }

    /// DWMWA_WINDOW_CORNER_PREFERENCE
    pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    /// DWMWCP_DONOTROUND (disable automatic Windows 11 corner rounding)
    pub const DWMWCP_DONOTROUND: i32 = 1;
    /// SW_HIDE = 0
    pub const SW_HIDE: i32 = 0;
    /// SW_SHOWNA = 8 (show without activating)
    pub const SW_SHOWNA: i32 = 8;

    /// GWL_EXSTYLE
    pub const GWL_EXSTYLE: i32 = -20;
    /// WS_EX_NOACTIVATE — window cannot be activated / never takes focus.
    pub const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    /// WS_EX_TOOLWINDOW — keep it out of Alt-Tab and the taskbar.
    pub const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    /// WS_EX_TOPMOST
    pub const WS_EX_TOPMOST: isize = 0x0000_0008;
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
    .inner_size(COLLAPSED_W as f64, COLLAPSED_H as f64)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .transparent(true)
    .shadow(false)
    .focused(false)
    .build()
    .map_err(|e| anyhow::anyhow!("floating window build error: {e}"))?;

    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = win.hwnd() {
            let h = hwnd.0;
            unsafe {
                // 1. Disable Windows 11 corner rounding (CSS does all the rounding).
                let pref: i32 = win32::DWMWCP_DONOTROUND;
                win32::DwmSetWindowAttribute(
                    h,
                    win32::DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const i32 as *const _,
                    std::mem::size_of::<i32>() as u32,
                );

                // 2. Force WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST so the
                //    window never steals focus and stays a click-only overlay.
                let cur = win32::GetWindowLongPtrW(h, win32::GWL_EXSTYLE);
                let new = cur
                    | win32::WS_EX_NOACTIVATE
                    | win32::WS_EX_TOOLWINDOW
                    | win32::WS_EX_TOPMOST;
                win32::SetWindowLongPtrW(h, win32::GWL_EXSTYLE, new);
            }
        }
    }

    Ok(())
}

/// Shows the floating button (collapsed) near screen coordinates (x, y).
pub fn show_floating(app: &AppHandle, x: f64, y: f64) -> Result<()> {
    let win = match app.get_webview_window(FLOATING_WINDOW_LABEL) {
        Some(w) => w,
        None => return Ok(()),
    };

    // Always start collapsed (resets a possibly-expanded window from last time).
    let _ = win.set_size(PhysicalSize::new(COLLAPSED_W, COLLAPSED_H));

    // Position the button BELOW-RIGHT of the cursor so it never overlaps the
    // selected text (the drag usually ends at the bottom-right of the selection).
    // The window has 12px transparent padding, so the visible button sits well
    // clear of the cursor/selection.
    let px = (x + 6.0) as i32;
    let py = (y + 16.0) as i32;
    win.set_position(PhysicalPosition::new(px.max(0), py.max(0)))
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Show WITHOUT activating, so the source app keeps focus and its selection.
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

/// Resizes the floating window between collapsed (button-only) and expanded
/// (button + translation card). The top-left corner stays fixed so the button
/// does not jump; the card grows downward/rightward.
pub fn set_floating_expanded(app: &AppHandle, expanded: bool) -> Result<()> {
    let win = match app.get_webview_window(FLOATING_WINDOW_LABEL) {
        Some(w) => w,
        None => return Ok(()),
    };
    let size = if expanded {
        PhysicalSize::new(EXPANDED_W, EXPANDED_H)
    } else {
        PhysicalSize::new(COLLAPSED_W, COLLAPSED_H)
    };
    win.set_size(size).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

/// Hides the floating button window.
///
/// On Windows we hide via the raw ShowWindow(SW_HIDE) to mirror the raw
/// SW_SHOWNA used to show it. Tauri's own win.hide() consults its cached
/// visibility flag, which is NOT updated by our raw show — so win.hide()
/// would no-op and the window would never disappear.
pub fn hide_floating(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(FLOATING_WINDOW_LABEL) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(hwnd) = win.hwnd() {
                unsafe { win32::ShowWindow(hwnd.0, win32::SW_HIDE); }
                return;
            }
        }
        let _ = win.hide();
    }
}

/// Returns true if the floating window is currently visible.
/// Uses the raw IsWindowVisible on Windows to stay consistent with the raw
/// show/hide path (Tauri's cached is_visible() can be out of sync).
pub fn is_floating_visible(app: &AppHandle) -> bool {
    if let Some(win) = app.get_webview_window(FLOATING_WINDOW_LABEL) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(hwnd) = win.hwnd() {
                return unsafe { win32::IsWindowVisible(hwnd.0) != 0 };
            }
        }
        return win.is_visible().unwrap_or(false);
    }
    false
}
