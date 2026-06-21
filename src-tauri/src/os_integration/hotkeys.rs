use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri::Emitter;

const DRAG_THRESHOLD_PX: f64 = 8.0;
const MULTI_CLICK_MS: u64 = 400;
const MULTI_CLICK_RADIUS_PX: f64 = 6.0;

/// Hook settings that can change at runtime (from the Settings panel) without
/// restarting the global hook thread. Read live on every relevant event.
pub struct SharedHookConfig {
    translate_replace: Mutex<String>,
    triple_copy_interval_ms: AtomicU64,
    triple_copy_count: AtomicUsize,
}

impl SharedHookConfig {
    pub fn new(translate_replace: String, interval_ms: u64, count: u32) -> Self {
        Self {
            translate_replace: Mutex::new(translate_replace),
            triple_copy_interval_ms: AtomicU64::new(interval_ms),
            triple_copy_count: AtomicUsize::new((count as usize).max(2)),
        }
    }

    /// Applies new hotkey settings immediately.
    pub fn update(&self, translate_replace: String, interval_ms: u64, count: u32) {
        *self.translate_replace.lock().unwrap_or_else(|e| e.into_inner()) = translate_replace;
        self.triple_copy_interval_ms.store(interval_ms, Ordering::Relaxed);
        self.triple_copy_count
            .store((count as usize).max(2), Ordering::Relaxed);
    }

    fn translate_replace(&self) -> String {
        self.translate_replace
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }
    fn interval_ms(&self) -> u64 {
        self.triple_copy_interval_ms.load(Ordering::Relaxed)
    }
    fn copy_count(&self) -> usize {
        self.triple_copy_count.load(Ordering::Relaxed).max(2)
    }
}

// ── Virtual-key codes we care about ───────────────────────────────────────────
// We work directly with Win32 virtual-key codes (the value the low-level hook
// reports), which lets the hook proc stay trivial — it never calls ToUnicodeEx
// or touches the keyboard layout. (rdev did that on every keystroke, which
// disturbed the GDI caret in classic apps like Notepad — the bug this replaces.)
type Vk = u32;

const VK_SHIFT: Vk = 0x10;
const VK_CONTROL: Vk = 0x11;
const VK_MENU: Vk = 0x12; // Alt
const VK_CAPITAL: Vk = 0x14;
const VK_LWIN: Vk = 0x5B;
const VK_RWIN: Vk = 0x5C;
const VK_LSHIFT: Vk = 0xA0;
const VK_RSHIFT: Vk = 0xA1;
const VK_LCONTROL: Vk = 0xA2;
const VK_RCONTROL: Vk = 0xA3;
const VK_LMENU: Vk = 0xA4;
const VK_RMENU: Vk = 0xA5; // AltGr arrives as this (+ a synthetic LCtrl)
const VK_C: Vk = 0x43;

fn is_ctrl(vk: Vk) -> bool {
    matches!(vk, VK_CONTROL | VK_LCONTROL | VK_RCONTROL)
}

fn is_modifier_vk(vk: Vk) -> bool {
    matches!(vk,
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL |
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT |
        VK_MENU | VK_LMENU | VK_RMENU |
        VK_LWIN | VK_RWIN | VK_CAPITAL |
        0x70..=0x7B // F1..F12
    )
}

struct HookState {
    config: Arc<SharedHookConfig>,
    held_keys: HashSet<Vk>,
    c_press_times: Vec<Instant>,
    last_pos: (f64, f64),
    mouse_down_pos: Option<(f64, f64)>,
    /// Tracks consecutive quick clicks to detect double/triple-click word selection.
    last_click_time: Option<Instant>,
    last_click_pos: (f64, f64),
    /// True once the translate-replace combo has fired, until its keys are
    /// released. Prevents auto-repeat from firing it many times in a row.
    tr_fired: bool,
    /// True if the mouse cursor was the I-beam (text) at any point during the
    /// current left-drag. A text selection starts over text even if it's
    /// released elsewhere, so this is more robust than checking only on release.
    drag_saw_ibeam: bool,
}

impl HookState {
    fn new(config: Arc<SharedHookConfig>) -> Self {
        Self {
            config,
            held_keys: HashSet::new(),
            c_press_times: Vec::new(),
            last_pos: (0.0, 0.0),
            mouse_down_pos: None,
            last_click_time: None,
            last_click_pos: (0.0, 0.0),
            tr_fired: false,
            drag_saw_ibeam: false,
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
        self.held_keys.contains(&VK_CONTROL)
            || self.held_keys.contains(&VK_LCONTROL)
            || self.held_keys.contains(&VK_RCONTROL)
    }

    fn shift_held(&self) -> bool {
        self.held_keys.contains(&VK_SHIFT)
            || self.held_keys.contains(&VK_LSHIFT)
            || self.held_keys.contains(&VK_RSHIFT)
    }

    fn alt_held(&self) -> bool {
        self.held_keys.contains(&VK_MENU)
            || self.held_keys.contains(&VK_LMENU)
            || self.held_keys.contains(&VK_RMENU)
    }

    fn record_c_press(&mut self) -> bool {
        let now = Instant::now();
        let window = Duration::from_millis(self.config.interval_ms());
        self.c_press_times.retain(|t| now.duration_since(*t) <= window);
        self.c_press_times.push(now);
        if self.c_press_times.len() >= self.config.copy_count() {
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

/// Maps a key name (e.g. "T", "5", "F3") to its Win32 virtual-key code. Letters
/// and digits share their ASCII value as the VK code; F1..F12 are 0x70..0x7B.
fn key_from_name(name: &str) -> Option<Vk> {
    let n = name.trim().to_ascii_uppercase();
    if n.len() == 1 {
        let c = n.as_bytes()[0];
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as Vk);
        }
        return None;
    }
    if let Some(num) = n.strip_prefix('F') {
        if let Ok(k) = num.parse::<u32>() {
            if (1..=12).contains(&k) {
                return Some(0x70 + (k - 1));
            }
        }
    }
    None
}

/// True if the mouse cursor is currently the system I-beam (text) cursor — the
/// universal sign that the pointer is over selectable text. Works in any app
/// (browser, Electron, native), unlike a Win32 caret which only exists in
/// editable fields. Used to allow the clipboard fallback ONLY over real text,
/// so canvas apps (Photoshop brush = crosshair) are never disturbed.
#[cfg(target_os = "windows")]
fn cursor_is_ibeam() -> bool {
    use std::ffi::c_void;
    #[repr(C)]
    struct Point { x: i32, y: i32 }
    #[repr(C)]
    struct CursorInfo {
        cb_size: u32,
        flags: u32,
        h_cursor: *mut c_void,
        pt_screen_pos: Point,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorInfo(pci: *mut CursorInfo) -> i32;
        fn LoadCursorW(hinstance: *mut c_void, lpcursorname: *const u16) -> *mut c_void;
    }
    const IDC_IBEAM: *const u16 = 32513usize as *const u16;
    unsafe {
        let mut ci: CursorInfo = std::mem::zeroed();
        ci.cb_size = std::mem::size_of::<CursorInfo>() as u32;
        if GetCursorInfo(&mut ci) == 0 || ci.h_cursor.is_null() {
            return false;
        }
        ci.h_cursor == LoadCursorW(std::ptr::null_mut(), IDC_IBEAM)
    }
}
#[cfg(not(target_os = "windows"))]
fn cursor_is_ibeam() -> bool { false }

// ── Event pipeline ────────────────────────────────────────────────────────────

/// A raw input event from the low-level hook. The hook proc only ever produces
/// these (cheap: it reads a vk code or a point and returns immediately); all
/// real processing happens on a separate thread so nothing slow runs inside the
/// system hook.
enum RawEvent {
    KeyDown(Vk),
    KeyUp(Vk),
    LButtonDown,
    LButtonUp,
    MouseMove(i32, i32),
}

/// Runs the hotkey/selection state machine for one raw event and performs the
/// resulting side effects (emit events, hide the floating button). Runs on the
/// dedicated processing thread, never inside the hook proc.
fn handle_event(st: &mut HookState, app: &AppHandle, ev: RawEvent) {
    match ev {
        RawEvent::KeyDown(vk) => {
            st.held_keys.insert(vk);

            if vk == VK_C && st.ctrl_held() && st.record_c_press() {
                let _ = app.emit("hotkey_triple_copy", ());
            }

            // ARM translate-replace while the chord is held; it FIRES on release
            // (below). Firing on press would run the synthetic Ctrl+C/Ctrl+V
            // while the user still holds the modifiers, mangling the input.
            let tr_hotkey = st.config.translate_replace();
            if st.is_translate_replace_combo(&tr_hotkey) {
                st.tr_fired = true;
            }

            // Hide the floating button when the user starts typing — but only if
            // it's actually shown, so normal typing touches nothing.
            if !is_modifier_vk(vk) && !st.ctrl_held() && super::floating::floating_is_shown() {
                super::hide_floating(app);
            }
        }
        RawEvent::KeyUp(vk) => {
            st.held_keys.remove(&vk);
            if is_ctrl(vk) {
                st.c_press_times.clear();
            }
            // Fire translate-replace once the chord is released, so no modifiers
            // are still physically held when the synthetic Ctrl+C / Ctrl+V run.
            if st.tr_fired {
                let tr_hotkey = st.config.translate_replace();
                if !st.is_translate_replace_combo(&tr_hotkey) {
                    st.tr_fired = false;
                    let _ = app.emit("hotkey_translate_replace", ());
                }
            }
        }
        RawEvent::LButtonDown => {
            st.mouse_down_pos = Some(st.last_pos);
            // A text selection starts over text → I-beam at press.
            st.drag_saw_ibeam = cursor_is_ibeam();
        }
        RawEvent::LButtonUp => {
            let (cx, cy) = st.last_pos;

            let had_drag = st.mouse_down_pos.take().map_or(false, |(dx, dy)| {
                let dist = ((cx - dx).powi(2) + (cy - dy).powi(2)).sqrt();
                dist >= DRAG_THRESHOLD_PX
            });

            // After a real drag, reset the click chain so the next single click
            // isn't mistaken for a double-click.
            if had_drag {
                st.last_click_time = None;
            }

            let is_multi_click = !had_drag && st.detect_multi_click(cx, cy);
            let has_selection = had_drag || is_multi_click;

            let text_cursor = st.drag_saw_ibeam || cursor_is_ibeam();
            st.drag_saw_ibeam = false;

            let _ = app.emit("mouse_selection_released", serde_json::json!({
                "has_selection": has_selection,
                "x": cx,
                "y": cy,
                "text_cursor": text_cursor,
            }));
        }
        RawEvent::MouseMove(x, y) => {
            let (x, y) = (x as f64, y as f64);
            st.last_pos = (x, y);
            // While dragging, note if the cursor passes over text.
            if st.mouse_down_pos.is_some() && !st.drag_saw_ibeam && cursor_is_ibeam() {
                st.drag_saw_ibeam = true;
            }
            // Only track cursor position while a drag is in progress (when a
            // selection is being made); no idle-mouse firehose.
            if st.mouse_down_pos.is_some() {
                let _ = app.emit("cursor_move", serde_json::json!({ "x": x, "y": y }));
            }
        }
    }
}

// ── Native low-level hook (Windows) ───────────────────────────────────────────

#[cfg(target_os = "windows")]
mod win_hook {
    use super::RawEvent;
    use std::ffi::c_void;
    use std::sync::mpsc::SyncSender;
    use std::sync::OnceLock;

    // The hook procs are plain C callbacks and can't capture state, so the
    // channel they push to lives in a static. SyncSender is Sync (so it fits a
    // OnceLock) and try_send never blocks the hook thread.
    static RAW_TX: OnceLock<SyncSender<RawEvent>> = OnceLock::new();

    const WH_KEYBOARD_LL: i32 = 13;
    const WH_MOUSE_LL: i32 = 14;
    const HC_ACTION: i32 = 0;

    const WM_KEYDOWN: u32 = 0x0100;
    const WM_KEYUP: u32 = 0x0101;
    const WM_SYSKEYDOWN: u32 = 0x0104;
    const WM_SYSKEYUP: u32 = 0x0105;
    const WM_MOUSEMOVE: u32 = 0x0200;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct KbdLlHookStruct {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    struct MsLlHookStruct {
        pt: Point,
        mouse_data: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut c_void,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: Point,
    }

    type HookProc = unsafe extern "system" fn(i32, usize, isize) -> isize;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(id: i32, func: HookProc, hmod: *mut c_void, thread: u32) -> *mut c_void;
        fn CallNextHookEx(hhk: *mut c_void, code: i32, w: usize, l: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    unsafe extern "system" fn keyboard_proc(code: i32, w: usize, l: isize) -> isize {
        if code == HC_ACTION {
            if let Some(tx) = RAW_TX.get() {
                let kb = &*(l as *const KbdLlHookStruct);
                let vk = kb.vk_code;
                match w as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => { let _ = tx.try_send(RawEvent::KeyDown(vk)); }
                    WM_KEYUP | WM_SYSKEYUP => { let _ = tx.try_send(RawEvent::KeyUp(vk)); }
                    _ => {}
                }
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    unsafe extern "system" fn mouse_proc(code: i32, w: usize, l: isize) -> isize {
        if code == HC_ACTION {
            if let Some(tx) = RAW_TX.get() {
                let ms = &*(l as *const MsLlHookStruct);
                match w as u32 {
                    WM_LBUTTONDOWN => { let _ = tx.try_send(RawEvent::LButtonDown); }
                    WM_LBUTTONUP => { let _ = tx.try_send(RawEvent::LButtonUp); }
                    WM_MOUSEMOVE => { let _ = tx.try_send(RawEvent::MouseMove(ms.pt.x, ms.pt.y)); }
                    _ => {}
                }
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    /// Installs the keyboard + mouse low-level hooks and pumps messages so they
    /// stay alive. Blocks forever — call on a dedicated thread.
    pub fn install_and_pump(tx: SyncSender<RawEvent>) {
        let _ = RAW_TX.set(tx);
        unsafe {
            let hmod = GetModuleHandleW(std::ptr::null());
            let kb = SetWindowsHookExW(WH_KEYBOARD_LL, keyboard_proc, hmod, 0);
            let ms = SetWindowsHookExW(WH_MOUSE_LL, mouse_proc, hmod, 0);
            if kb.is_null() || ms.is_null() {
                log::error!("SetWindowsHookExW failed (keyboard={kb:?}, mouse={ms:?})");
                return;
            }
            // Low-level hooks are dispatched while this thread pumps messages.
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
        }
    }
}

/// Spawns the global keyboard/mouse hook plus a processing thread that runs the
/// hotkey/selection state machine off the hook.
pub fn spawn_hook(app: AppHandle, config: Arc<SharedHookConfig>) {
    // Buffered so the hook proc's try_send never blocks; the processing thread
    // drains it continuously.
    let (tx, rx) = std::sync::mpsc::sync_channel::<RawEvent>(4096);

    // Processing thread: owns the state machine and the AppHandle.
    std::thread::Builder::new()
        .name("deepm-hook-proc".into())
        .spawn(move || {
            let mut st = HookState::new(config);
            while let Ok(ev) = rx.recv() {
                handle_event(&mut st, &app, ev);
            }
        })
        .expect("failed to spawn hook processing thread");

    // Hook thread: installs the native LL hooks and pumps their messages.
    #[cfg(target_os = "windows")]
    std::thread::Builder::new()
        .name("deepm-hook".into())
        .spawn(move || win_hook::install_and_pump(tx))
        .expect("failed to spawn hook thread");

    #[cfg(not(target_os = "windows"))]
    let _ = tx;
}
