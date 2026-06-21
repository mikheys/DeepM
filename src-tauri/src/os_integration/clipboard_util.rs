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

/// Selected text via UI Automation ONLY — never touches the clipboard. This
/// gates the *proactive* showing of the floating button, so merely selecting
/// text can never clobber the user's clipboard (the old Ctrl+C probe raced with
/// the user's own Ctrl+C and pasted stale content). Apps without UIA text just
/// won't auto-show the button — but the button still appears over an I-beam and
/// captures on click (see `capture_selection`).
pub fn get_selected_text() -> Option<String> {
    super::uia::selection_via_uia()
}

/// Captures the selection only when the user EXPLICITLY clicks the floating
/// button (a deliberate action, so no race with their own Ctrl+C): UIA first,
/// else a Ctrl+C copy with full clipboard save/restore.
pub fn capture_selection() -> Option<String> {
    if let Some(t) = super::uia::selection_via_uia() {
        return Some(t);
    }
    #[cfg(target_os = "windows")]
    { copy_selection_nondestructive() }
    #[cfg(not(target_os = "windows"))]
    { None }
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
