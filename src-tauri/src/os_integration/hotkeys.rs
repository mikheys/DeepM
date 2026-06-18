use anyhow::Result;
use rdev::{listen, Event, EventType, Key, Button};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri::Emitter;

const DRAG_THRESHOLD_PX: f64 = 8.0;

struct HookState {
    held_keys: HashSet<Key>,
    c_press_times: Vec<Instant>,
    interval_ms: u64,
    last_pos: (f64, f64),
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

    fn record_c_press(&mut self) -> bool {
        let now = Instant::now();
        let window = Duration::from_millis(self.interval_ms);
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
                        EventType::KeyPress(key) => {
                            st.held_keys.insert(key.clone());

                            if key == Key::KeyC && st.ctrl_held() {
                                if st.record_c_press() {
                                    let _ = app.emit("hotkey_triple_copy", ());
                                }
                            }

                            if st.is_translate_replace_combo(&tr_hotkey) {
                                let _ = app.emit("hotkey_translate_replace", ());
                            }

                            // Hide floating button when user types / deletes text
                            let is_modifier = matches!(key,
                                Key::ControlLeft | Key::ControlRight |
                                Key::ShiftLeft | Key::ShiftRight |
                                Key::Alt | Key::AltGr |
                                Key::MetaLeft | Key::MetaRight |
                                Key::CapsLock | Key::F1 | Key::F2 | Key::F3 |
                                Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 |
                                Key::F9 | Key::F10 | Key::F11 | Key::F12
                            );
                            if !is_modifier && !st.ctrl_held() {
                                let app_c = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    super::hide_floating(&app_c);
                                });
                            }
                        }
                        EventType::KeyRelease(key) => {
                            st.held_keys.remove(&key);
                            if key == Key::ControlLeft || key == Key::ControlRight {
                                st.c_press_times.clear();
                            }
                        }

                        EventType::ButtonPress(Button::Left) => {
                            st.mouse_down_pos = Some(st.last_pos);
                        }
                        EventType::ButtonRelease(Button::Left) => {
                            let (cx, cy) = st.last_pos;

                            // Only a drag (mouse moved past the threshold while held) counts
                            // as a text selection. We deliberately do NOT treat double/triple
                            // clicks as selections: that produced false positives (e.g. double-
                            // clicking a tray icon) and, crucially, confirming a selection would
                            // require a synthetic Ctrl+C — which wipes the selection in console /
                            // terminal apps. The actual copy now happens only when the user
                            // clicks the floating button (see translate_selection).
                            let has_selection = st.mouse_down_pos.take().map_or(false, |(dx, dy)| {
                                let dist = ((cx - dx).powi(2) + (cy - dy).powi(2)).sqrt();
                                dist >= DRAG_THRESHOLD_PX
                            });

                            let _ = app.emit("mouse_selection_released", serde_json::json!({
                                "has_selection": has_selection,
                                "x": cx,
                                "y": cy,
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
                log::error!("rdev::listen error: {e:?}");
            }
        })
        .expect("failed to spawn hook thread");
}
