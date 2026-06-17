use anyhow::Result;
use rdev::{listen, Event, EventType, Key, Button};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::AppHandle;
use tauri::Emitter;

const DRAG_THRESHOLD_PX: f64 = 12.0;

/// Internal state shared across the rdev callback (must be Send).
struct HookState {
    held_keys: HashSet<Key>,
    c_press_times: Vec<Instant>,
    interval_ms: u64,
    /// Last known cursor position (updated on every MouseMove).
    last_pos: (f64, f64),
    /// Cursor position when left button was pressed.
    mouse_down_pos: Option<(f64, f64)>,
}

impl HookState {
    fn new(interval_ms: u64) -> Self {
        Self {
            held_keys: HashSet::new(),
            c_press_times: Vec::new(),
            interval_ms,
            last_pos: (0.0, 0.0),
            mouse_down_pos: None,
        }
    }

    fn ctrl_held(&self) -> bool {
        self.held_keys.contains(&Key::ControlLeft)
            || self.held_keys.contains(&Key::ControlRight)
    }

    fn shift_held(&self) -> bool {
        self.held_keys.contains(&Key::ShiftLeft)
            || self.held_keys.contains(&Key::ShiftRight)
    }

    /// Push a C press and check if triple occurred within interval.
    fn record_c_press(&mut self) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_millis(self.interval_ms);
        self.c_press_times.retain(|t| now.duration_since(*t) <= window);
        self.c_press_times.push(now);
        if self.c_press_times.len() >= 3 {
            self.c_press_times.clear();
            return true;
        }
        false
    }

    fn is_translate_replace_combo(&self, hotkey: &str) -> bool {
        let parts: HashSet<&str> = hotkey.split('+').map(str::trim).collect();
        let ctrl = parts.contains("Ctrl");
        let shift = parts.contains("Shift");
        let key_char = parts.iter()
            .find(|&&p| p != "Ctrl" && p != "Shift" && p != "Alt")
            .copied()
            .unwrap_or("");

        let ctrl_ok = !ctrl || self.ctrl_held();
        let shift_ok = !shift || self.shift_held();
        let key_ok = match key_char {
            "T" => self.held_keys.contains(&Key::KeyT),
            "U" => self.held_keys.contains(&Key::KeyU),
            "F1" => self.held_keys.contains(&Key::F1),
            _ => false,
        };
        ctrl_ok && shift_ok && key_ok
    }
}

/// Spawns the global keyboard/mouse hook on a dedicated OS thread.
/// Events are forwarded to the Tauri app via `app.emit()`.
///
/// `translate_replace_hotkey`: e.g. "Ctrl+Shift+T"
/// `triple_copy_interval_ms`: max ms between consecutive C presses
pub fn spawn_hook(
    app: AppHandle,
    translate_replace_hotkey: String,
    triple_copy_interval_ms: u64,
) {
    std::thread::Builder::new()
        .name("deepm-hook".into())
        .spawn(move || {
            let state = Arc::new(Mutex::new(HookState::new(triple_copy_interval_ms)));

            let callback = {
                let app = app.clone();
                let state = Arc::clone(&state);
                let tr_hotkey = translate_replace_hotkey.clone();

                move |event: Event| {
                    let mut st = match state.lock() {
                        Ok(s) => s,
                        Err(_) => return,
                    };

                    match event.event_type {
                        // ── Key tracking ───────────────────────────────────
                        EventType::KeyPress(key) => {
                            st.held_keys.insert(key.clone());

                            // Triple-C detection (Ctrl held + 3× C within interval)
                            if key == Key::KeyC && st.ctrl_held() {
                                if st.record_c_press() {
                                    let _ = app.emit("hotkey_triple_copy", ());
                                    log::debug!("triple-copy triggered");
                                }
                            }

                            // Translate-replace hotkey
                            if st.is_translate_replace_combo(&tr_hotkey) {
                                let _ = app.emit("hotkey_translate_replace", ());
                                log::debug!("translate-replace triggered");
                            }
                        }
                        EventType::KeyRelease(key) => {
                            st.held_keys.remove(&key);
                            // Clear triple-C history when Ctrl is released
                            if key == Key::ControlLeft || key == Key::ControlRight {
                                st.c_press_times.clear();
                            }
                        }

                        // ── Mouse tracking for floating button ─────────────
                        EventType::ButtonPress(Button::Left) => {
                            // Record position at press time (from last MouseMove)
                            st.mouse_down_pos = Some(st.last_pos);
                        }
                        EventType::ButtonRelease(Button::Left) => {
                            let (cx, cy) = st.last_pos;
                            let had_drag = st.mouse_down_pos.take().map_or(false, |(dx, dy)| {
                                let dist = ((cx - dx).powi(2) + (cy - dy).powi(2)).sqrt();
                                dist >= DRAG_THRESHOLD_PX
                            });
                            // Always emit so lib.rs can hide the button on plain clicks
                            let _ = app.emit("mouse_selection_released", serde_json::json!({
                                "had_drag": had_drag,
                            }));
                        }
                        EventType::MouseMove { x, y } => {
                            st.last_pos = (x, y);
                            let _ = app.emit("cursor_move", serde_json::json!({ "x": x, "y": y }));
                        }

                        _ => {}
                    }
                }
            };

            if let Err(e) = listen(callback) {
                log::error!("rdev::listen error: {e:?}. Hotkeys and floating button will be unavailable.");
            }
        })
        .expect("failed to spawn hook thread");
}
