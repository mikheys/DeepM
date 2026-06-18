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

    #[repr(C)]
    pub struct POINT {
        pub x: i32,
        pub y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    pub struct MONITORINFO {
        pub cb_size: u32,
        pub rc_monitor: RECT,
        pub rc_work: RECT,
        pub dw_flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn ShowWindow(hwnd: *mut c_void, n_cmd_show: i32) -> i32;
        pub fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        pub fn GetWindowLongPtrW(hwnd: *mut c_void, n_index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
        pub fn GetCursorPos(lp_point: *mut POINT) -> i32;
        pub fn SetWindowPos(
            hwnd: *mut c_void,
            hwnd_insert_after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            u_flags: u32,
        ) -> i32;
        pub fn GetWindowRect(hwnd: *mut c_void, lp_rect: *mut RECT) -> i32;
        pub fn MonitorFromPoint(pt: POINT, flags: u32) -> *mut c_void;
        pub fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
        pub fn GetMonitorInfoW(hmonitor: *mut c_void, lpmi: *mut MONITORINFO) -> i32;
    }

    pub const MONITOR_DEFAULTTONEAREST: u32 = 2;

    /// Returns the work area (excludes the taskbar) of the monitor that contains
    /// the given monitor handle.
    pub unsafe fn work_area(hmonitor: *mut c_void) -> Option<RECT> {
        if hmonitor.is_null() {
            return None;
        }
        let mut mi: MONITORINFO = core::mem::zeroed();
        mi.cb_size = core::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmonitor, &mut mi) != 0 {
            Some(mi.rc_work)
        } else {
            None
        }
    }

    /// Clamps a window rect (x, y, w, h) so it stays fully inside `area`.
    pub fn clamp_to(x: i32, y: i32, w: i32, h: i32, area: &RECT) -> (i32, i32) {
        let mut nx = x;
        let mut ny = y;
        if nx + w > area.right {
            nx = area.right - w;
        }
        if ny + h > area.bottom {
            ny = area.bottom - h;
        }
        if nx < area.left {
            nx = area.left;
        }
        if ny < area.top {
            ny = area.top;
        }
        (nx, ny)
    }

    /// DWMWA_WINDOW_CORNER_PREFERENCE
    pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    /// DWMWCP_DONOTROUND (disable automatic Windows 11 corner rounding)
    pub const DWMWCP_DONOTROUND: i32 = 1;
    /// SW_HIDE = 0
    pub const SW_HIDE: i32 = 0;
    /// SW_SHOWNA = 8 (show without activating)
    pub const SW_SHOWNA: i32 = 8;

    /// HWND_TOPMOST = (HWND)-1
    pub fn hwnd_topmost() -> *mut c_void {
        -1isize as *mut c_void
    }
    /// SetWindowPos flags
    pub const SWP_NOSIZE: u32 = 0x0001;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
    pub const SWP_SHOWWINDOW: u32 = 0x0040;

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

/// Shows the floating button (collapsed) near the mouse cursor.
///
/// `x`/`y` (from the input hook) are used as a fallback only. On Windows we
/// position via raw GetCursorPos + SetWindowPos, which both operate in the same
/// physical virtual-desktop coordinate space — this is what makes the button
/// land on the CORRECT monitor in a multi-monitor setup. Going through Tauri's
/// set_position re-interprets the coordinates against the window's current
/// monitor DPI and lands it on the wrong screen.
pub fn show_floating(app: &AppHandle, x: f64, y: f64) -> Result<()> {
    let win = match app.get_webview_window(FLOATING_WINDOW_LABEL) {
        Some(w) => w,
        None => return Ok(()),
    };

    // Always start collapsed (resets a possibly-expanded window from last time).
    let _ = win.set_size(PhysicalSize::new(COLLAPSED_W, COLLAPSED_H));

    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                // Real cursor position in physical virtual-desktop coordinates.
                let mut pt = win32::POINT { x: 0, y: 0 };
                let (cx, cy) = if win32::GetCursorPos(&mut pt) != 0 {
                    (pt.x, pt.y)
                } else {
                    (x as i32, y as i32)
                };
                // Below-right of the cursor so it clears the selected text.
                let mut px = cx + 6;
                let mut py = cy + 16;

                // Keep the whole button inside the work area of the monitor the
                // cursor is on, so it never spills off the edge or onto another
                // screen.
                let hmon = win32::MonitorFromPoint(
                    win32::POINT { x: cx, y: cy },
                    win32::MONITOR_DEFAULTTONEAREST,
                );
                if let Some(area) = win32::work_area(hmon) {
                    let (nx, ny) =
                        win32::clamp_to(px, py, COLLAPSED_W as i32, COLLAPSED_H as i32, &area);
                    px = nx;
                    py = ny;
                }

                // Position + show topmost WITHOUT activating (keeps source focus).
                win32::SetWindowPos(
                    hwnd.0,
                    win32::hwnd_topmost(),
                    px,
                    py,
                    0,
                    0,
                    win32::SWP_NOSIZE | win32::SWP_NOACTIVATE | win32::SWP_SHOWWINDOW,
                );
            }
            return Ok(());
        }
    }

    // Non-Windows fallback.
    let px = (x + 6.0) as i32;
    let py = (y + 16.0) as i32;
    win.set_position(PhysicalPosition::new(px.max(0), py.max(0)))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    win.show().map_err(|e| anyhow::anyhow!("{e}"))?;
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
    let (w, h) = if expanded {
        (EXPANDED_W as i32, EXPANDED_H as i32)
    } else {
        (COLLAPSED_W as i32, COLLAPSED_H as i32)
    };

    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                // Current top-left, then clamp the NEW size to the monitor so an
                // expanding card never spills off the bottom/right or across a
                // monitor boundary.
                let mut rect = win32::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                let (mut x, mut y) = if win32::GetWindowRect(hwnd.0, &mut rect) != 0 {
                    (rect.left, rect.top)
                } else {
                    (0, 0)
                };
                let hmon = win32::MonitorFromWindow(hwnd.0, win32::MONITOR_DEFAULTTONEAREST);
                if let Some(area) = win32::work_area(hmon) {
                    let (nx, ny) = win32::clamp_to(x, y, w, h, &area);
                    x = nx;
                    y = ny;
                }
                win32::SetWindowPos(
                    hwnd.0,
                    win32::hwnd_topmost(),
                    x,
                    y,
                    w,
                    h,
                    win32::SWP_NOACTIVATE,
                );
            }
            return Ok(());
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        win.set_size(PhysicalSize::new(w as u32, h as u32))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
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
