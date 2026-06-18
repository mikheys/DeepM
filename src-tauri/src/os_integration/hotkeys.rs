use anyhow::Result;
use rdev::{listen, Event, EventType, Key, Button};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri::Emitter;

const DRAG_THRESHOLD_PX: f64 = 8.0;
const MULTI_CLICK_MS: u64 = 400;
const MULTI_CLICK_RADIUS_PX: f64 = 6.0;

struct HookState {
    held_keys: HashSet<Key>,
    c_press_times: Vec<Instant>,
    interval_ms: u64,
    /// How many quick Ctrl+C presses trigger the copy→open action (>= 2).
    copy_count: usize,
    last_pos: (f64, f64),
    mouse_down_pos: Option<(f64, f64)>,
    /// Tracks consecutive quick clicks to detect double/triple-click word selection.
    last_click_time: Option<Instant>,
    last_click_pos: (f64, f64),
    /// True once the translate-replace combo has fired, until its keys are
    /// released. Prevents auto-repeat from firing it many times in a row.
    tr_fired: bool,
}

impl HookState {
    fn new(interval_ms: u64, copy_count: usize) -> Self {
        Self {
            held_keys: HashSet::new(),
            c_press_times: Vec::new(),
            interval_ms,
            copy_count: copy_count.max(2),
            last_pos: (0.0, 0.0),
            mouse_down_pos: None,
            last_click_time: None,
            last_click_pos: (0.0, 0.0),
            tr_fired: false,
        }
    }

    /// Returns true if this release is a double/triple-click at the same spot
    /// (i.e. word/line selection without dragging).
    fn detect_multi_click(&mut self, x: f64, y: f64) -> bool {
        let now = Instant::now();
        let is_multi = self.last_click_time.map_or(false, |t| {
            let time_ok = now.duration_since(t) < Duration::from_millis(MULTI_CLICK_MS);
            let (lx, ly) = self.last_click_pos;
            let dist = ((x - lx).powi(2) + (y - ly).powi(2)).sqrt();
            time_ok && dist < MULTI_CLICK_RADIUS_PX
        });
        self.last_click_time = Some(now);
        self.last_click_pos = (x, y);
        is_multi
    }

    fn ctrl_held(&self) -> bool {
        self.held_keys.contains(&Key::ControlLeft)
            || self.held_keys.contains(&Key::ControlRight)
    }

    fn shift_held(&self) -> bool {
        self.held_keys.contains(&Key::ShiftLeft)
            || self.held_keys.contains(&Key::ShiftRight)
    }

    fn alt_held(&self) -> bool {
        self.held_keys.contains(&Key::Alt) || self.held_keys.contains(&Key::AltGr)
    }

    fn record_c_press(&mut self) -> bool {
        let now = Instant::now();
        let window = Duration::from_millis(self.interval_ms);
        self.c_press_times.retain(|t| now.duration_since(*t) <= window);
        self.c_press_times.push(now);
        if self.c_press_times.len() >= self.copy_count {
            self.c_press_times.clear();
            return true;
        }
        false
    }

    /// Matches a hotkey string like "Ctrl+Shift+T" against the currently held
    /// keys. Supports A–Z, 0–9 and F1–F12 as the main key, with exact Ctrl /
    /// Shift / Alt modifiers.
    fn is_translate_replace_combo(&self, hotkey: &str) -> bool {
        let parts: Vec<&str> = hotkey
            .split('+')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            return false;
        }

        let need_ctrl = parts.iter().any(|p| p.eq_ignore_ascii_case("ctrl"));
        let need_shift = parts.iter().any(|p| p.eq_ignore_ascii_case("shift"));
        let need_alt = parts.iter().any(|p| p.eq_ignore_ascii_case("alt"));

        let main = parts.iter().find(|p| {
            !p.eq_ignore_ascii_case("ctrl")
                && !p.eq_ignore_ascii_case("shift")
                && !p.eq_ignore_ascii_case("alt")
        });

        let main_key = match main.and_then(|m| key_from_name(m)) {
            Some(k) => k,
            None => return false,
        };

        self.ctrl_held() == need_ctrl
            && self.shift_held() == need_shift
            && self.alt_held() == need_alt
            && self.held_keys.contains(&main_key)
    }
}

/// Maps a key name (e.g. "T", "5", "F3") to the corresponding rdev key.
fn key_from_name(name: &str) -> Option<Key> {
    use rdev::Key::*;
    let n = name.to_ascii_uppercase();
    Some(match n.as_str() {
        "A" => KeyA, "B" => KeyB, "C" => KeyC, "D" => KeyD, "E" => KeyE,
        "F" => KeyF, "G" => KeyG, "H" => KeyH, "I" => KeyI, "J" => KeyJ,
        "K" => KeyK, "L" => KeyL, "M" => KeyM, "N" => KeyN, "O" => KeyO,
        "P" => KeyP, "Q" => KeyQ, "R" => KeyR, "S" => KeyS, "T" => KeyT,
        "U" => KeyU, "V" => KeyV, "W" => KeyW, "X" => KeyX, "Y" => KeyY,
        "Z" => KeyZ,
        "0" => Num0, "1" => Num1, "2" => Num2, "3" => Num3, "4" => Num4,
        "5" => Num5, "6" => Num6, "7" => Num7, "8" => Num8, "9" => Num9,
        "F1" => F1, "F2" => F2, "F3" => F3, "F4" => F4, "F5" => F5, "F6" => F6,
        "F7" => F7, "F8" => F8, "F9" => F9, "F10" => F10, "F11" => F11, "F12" => F12,
        _ => return None,
    })
}

pub fn spawn_hook(
    app: AppHandle,
    translate_replace_hotkey: String,
    triple_copy_interval_ms: u64,
    triple_copy_count: u32,
) {
    std::thread::Builder::new()
        .name("deepm-hook".into())
        .spawn(move || {
            let state = Arc::new(Mutex::new(HookState::new(
                triple_copy_interval_ms,
                triple_copy_count as usize,
            )));

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

                            // Fire translate-replace once per chord. Without the
                            // tr_fired guard, key auto-repeat re-emits this every few
                            // milliseconds, causing the translation to be pasted
                            // multiple times.
                            if st.is_translate_replace_combo(&tr_hotkey) {
                                if !st.tr_fired {
                                    st.tr_fired = true;
                                    let _ = app.emit("hotkey_translate_replace", ());
                                }
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
                            // Re-arm the translate-replace combo once its keys are released.
                            if st.tr_fired && !st.is_translate_replace_combo(&tr_hotkey) {
                                st.tr_fired = false;
                            }
                        }

                        EventType::ButtonPress(Button::Left) => {
                            st.mouse_down_pos = Some(st.last_pos);
                        }
                        EventType::ButtonRelease(Button::Left) => {
                            let (cx, cy) = st.last_pos;

                            let had_drag = st.mouse_down_pos.take().map_or(false, |(dx, dy)| {
                                let dist = ((cx - dx).powi(2) + (cy - dy).powi(2)).sqrt();
                                dist >= DRAG_THRESHOLD_PX
                            });

                            // After a real drag, reset the click chain so the next single
                            // click isn't mistaken for a double-click.
                            if had_drag {
                                st.last_click_time = None;
                            }

                            // A selection is either a drag OR a double/triple-click (word /
                            // line select). A drag is a reliable signal, so we show the
                            // button straight away. A multi-click is ambiguous (double-
                            // clicking the desktop selects nothing), so it carries
                            // verify=true and the backend confirms a real selection exists
                            // before showing the button.
                            let is_multi_click = !had_drag && st.detect_multi_click(cx, cy);
                            let has_selection = had_drag || is_multi_click;

                            let _ = app.emit("mouse_selection_released", serde_json::json!({
                                "has_selection": has_selection,
                                "verify": is_multi_click,
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
