use anyhow::{anyhow, Result};
use std::time::Duration;

/// Waits until the user physically releases every modifier key, then sends a
/// synthetic key-up for each as a safety net. The translate-replace hotkey
/// (Ctrl+Alt+T) is usually still held when the action fires; a synthetic key-up
/// alone does NOT clear a physically-held key (the hardware state persists), so
/// Ctrl+C/Ctrl+V become Alt+c etc. and type garbage (the "⌀¢") over the
/// selection. Polling GetAsyncKeyState until the keys are actually up is the
/// reliable fix.
#[cfg(target_os = "windows")]
fn prepare_for_synthetic_input(enigo: &mut enigo::Enigo) {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12; // Alt
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    let down = |vk: i32| unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };

    let start = std::time::Instant::now();
    while (down(VK_CONTROL) || down(VK_MENU) || down(VK_SHIFT) || down(VK_LWIN) || down(VK_RWIN))
        && start.elapsed() < Duration::from_millis(1500)
    {
        std::thread::sleep(Duration::from_millis(20));
    }

    // Safety net: also send synthetic key-ups, then let the input queue settle.
    use enigo::{Direction, Key, Keyboard};
    for k in [Key::Control, Key::Alt, Key::Shift, Key::Meta] {
        let _ = enigo.key(k, Direction::Release);
    }
    std::thread::sleep(Duration::from_millis(40));
}
#[cfg(not(target_os = "windows"))]
fn prepare_for_synthetic_input(_enigo: &mut enigo::Enigo) {}

/// Saves current clipboard text, returns it (or None if clipboard was not text).
pub fn save_clipboard() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Writes text to clipboard.
pub fn write_clipboard(text: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new()
        .map_err(|e| anyhow!("clipboard error: {e}"))?;
    cb.set_text(text).map_err(|e| anyhow!("clipboard write error: {e}"))
}

/// Reads current clipboard text.
pub fn read_clipboard() -> Result<String> {
    let mut cb = arboard::Clipboard::new()
        .map_err(|e| anyhow!("clipboard error: {e}"))?;
    cb.get_text().map_err(|e| anyhow!("clipboard read error: {e}"))
}

/// A snapshot of the clipboard (text OR image) so it can be restored verbatim
/// after our copy/paste actions — not just text.
pub enum ClipboardData {
    Text(String),
    Image(arboard::ImageData<'static>),
    Empty,
}

/// Snapshot the clipboard before we overwrite it.
pub fn snapshot_clipboard() -> ClipboardData {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(t) = cb.get_text() {
            if !t.is_empty() {
                return ClipboardData::Text(t);
            }
        }
        if let Ok(img) = cb.get_image() {
            return ClipboardData::Image(img);
        }
    }
    ClipboardData::Empty
}

/// Restore a previously taken snapshot.
pub fn restore_clipboard(data: ClipboardData) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        match data {
            ClipboardData::Text(t) => { let _ = cb.set_text(t); }
            ClipboardData::Image(img) => { let _ = cb.set_image(img); }
            ClipboardData::Empty => { let _ = cb.clear(); }
        }
    }
}

/// Simulates Ctrl+C (copy), waits briefly, reads clipboard.
/// Saves and restores the previous clipboard content so the user's data is not lost.
/// Returns the newly copied (selected) text.
pub fn copy_selection_to_clipboard() -> Result<String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    // Snapshot clipboard before we touch it
    let previous = read_clipboard().ok();

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("enigo init error: {e}"))?;

    prepare_for_synthetic_input(&mut enigo);
    std::thread::sleep(Duration::from_millis(50));

    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;
    enigo.key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;

    std::thread::sleep(Duration::from_millis(150));

    let selected = read_clipboard()?;

    // Restore original clipboard so the user's content is not lost
    if let Some(prev) = previous {
        let _ = write_clipboard(&prev);
    }

    Ok(selected)
}

/// Returns the currently selected text, or None if nothing is selected. Gates
/// the floating button, so it must NEVER disturb a non-text foreground app.
///
/// Two non-destructive-where-it-matters layers:
/// 1. UI Automation query of the focused element (no keystroke) — covers most
///    native + Chromium apps.
/// 2. If UIA finds nothing AND the foreground window has a text caret, fall back
///    to a clipboard copy (save/restore). The caret guard is what keeps canvas
///    apps safe: Photoshop's canvas has no caret, so the Ctrl+C never fires
///    there, while real edit fields (Explorer rename, etc.) do have one.
/// `text_cursor` = the pointer was an I-beam when the selection gesture ended
/// (captured by the hook). Together with a Win32 caret it tells us the context
/// is text, so the clipboard fallback is safe; on a canvas (brush cursor, no
/// caret) it stays false and no keystroke is sent.
pub fn get_selected_text(text_cursor: bool) -> Option<String> {
    if let Some(text) = super::uia::selection_via_uia() {
        return Some(text);
    }
    #[cfg(target_os = "windows")]
    if text_cursor || foreground_has_caret() {
        return copy_selection_nondestructive();
    }
    let _ = text_cursor;
    None
}

/// True if the foreground GUI thread currently shows a text caret — i.e. focus
/// is in an editable text field. (Win32 carets only; Chromium draws its own, so
/// Electron text fields rely on the UIA layer above.)
#[cfg(target_os = "windows")]
fn foreground_has_caret() -> bool {
    use std::ffi::c_void;
    #[repr(C)]
    struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
    #[repr(C)]
    struct GuiThreadInfo {
        cb_size: u32,
        flags: u32,
        hwnd_active: *mut c_void,
        hwnd_focus: *mut c_void,
        hwnd_capture: *mut c_void,
        hwnd_menu_owner: *mut c_void,
        hwnd_move_size: *mut c_void,
        hwnd_caret: *mut c_void,
        rc_caret: Rect,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetGUIThreadInfo(id_thread: u32, pgui: *mut GuiThreadInfo) -> i32;
    }
    unsafe {
        let mut gti: GuiThreadInfo = std::mem::zeroed();
        gti.cb_size = std::mem::size_of::<GuiThreadInfo>() as u32;
        GetGUIThreadInfo(0, &mut gti) != 0 && !gti.hwnd_caret.is_null()
    }
}

/// Clipboard-copy fallback (Ctrl+C with save/restore). Only called from
/// get_selected_text when a text caret is present, so it never touches canvas
/// apps. Returns None if nothing actually got selected.
#[cfg(target_os = "windows")]
fn copy_selection_nondestructive() -> Option<String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let snap = snapshot_clipboard();
    let prev_str = match &snap {
        ClipboardData::Text(t) => t.clone(),
        _ => String::new(),
    };

    let mut enigo = Enigo::new(&Settings::default()).ok()?;
    prepare_for_synthetic_input(&mut enigo);
    std::thread::sleep(Duration::from_millis(40));
    enigo.key(Key::Control, Direction::Press).ok()?;
    enigo.key(Key::Unicode('c'), Direction::Click).ok()?;
    enigo.key(Key::Control, Direction::Release).ok()?;
    std::thread::sleep(Duration::from_millis(120));

    let selected = read_clipboard().ok();
    restore_clipboard(snap); // put back exactly what was there (text or image)

    match selected {
        Some(s) if !s.trim().is_empty() && s != prev_str => Some(s),
        _ => None,
    }
}

/// Simulates Ctrl+V (paste).
pub fn paste_from_clipboard() -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("enigo init error: {e}"))?;

    prepare_for_synthetic_input(&mut enigo);
    std::thread::sleep(Duration::from_millis(30));
    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;
    enigo.key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;

    Ok(())
}
