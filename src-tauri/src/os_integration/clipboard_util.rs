use anyhow::{anyhow, Result};
use std::time::Duration;

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

/// Simulates Ctrl+C (copy), waits briefly, reads clipboard.
/// Saves and restores the previous clipboard content so the user's data is not lost.
/// Returns the newly copied (selected) text.
pub fn copy_selection_to_clipboard() -> Result<String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    // Snapshot clipboard before we touch it
    let previous = read_clipboard().ok();

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("enigo init error: {e}"))?;

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

/// Simulates Ctrl+V (paste).
pub fn paste_from_clipboard() -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("enigo init error: {e}"))?;

    std::thread::sleep(Duration::from_millis(30));
    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;
    enigo.key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| anyhow!("enigo key error: {e}"))?;

    Ok(())
}
