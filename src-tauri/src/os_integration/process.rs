//! Foreground-window / running-app detection used by the app-exclusion list.
//! Implemented with inline Win32 FFI to avoid pulling in a heavy dependency.

#[cfg(target_os = "windows")]
mod imp {
    use core::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn EnumWindows(cb: extern "system" fn(*mut c_void, isize) -> i32, lparam: isize) -> i32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        fn GetWindowTextLengthW(hwnd: *mut c_void) -> i32;
        fn GetWindow(hwnd: *mut c_void, cmd: u32) -> *mut c_void;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            handle: *mut c_void,
            flags: u32,
            buf: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(h: *mut c_void) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const GW_OWNER: u32 = 4;

    /// Lower-cased executable base name (e.g. "mobaxterm.exe") for a PID.
    unsafe fn process_name_for_pid(pid: u32) -> Option<String> {
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        let name = path
            .rsplit(|c| c == '\\' || c == '/')
            .next()
            .unwrap_or(&path)
            .to_lowercase();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// Executable base name of the window the user is currently working in.
    pub fn foreground_process_name() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            process_name_for_pid(pid)
        }
    }

    extern "system" fn enum_cb(hwnd: *mut c_void, lparam: isize) -> i32 {
        unsafe {
            let vec = &mut *(lparam as *mut Vec<String>);
            // Only real, top-level, titled application windows.
            if IsWindowVisible(hwnd) == 0
                || GetWindowTextLengthW(hwnd) == 0
                || !GetWindow(hwnd, GW_OWNER).is_null()
            {
                return 1;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if let Some(name) = process_name_for_pid(pid) {
                if !vec.contains(&name) {
                    vec.push(name);
                }
            }
            1 // continue enumeration
        }
    }

    /// Distinct executable names of apps that currently own a visible window.
    pub fn list_app_processes() -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        unsafe {
            EnumWindows(enum_cb, &mut result as *mut Vec<String> as isize);
        }
        // Don't offer our own process as an exclusion candidate.
        result.retain(|n| n != "deepm.exe");
        result.sort();
        result
    }
}

#[cfg(target_os = "windows")]
pub use imp::{foreground_process_name, list_app_processes};

#[cfg(not(target_os = "windows"))]
pub fn foreground_process_name() -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn list_app_processes() -> Vec<String> {
    Vec::new()
}

/// True if the current foreground app is in the user's exclusion list.
pub fn foreground_is_excluded(exclusions: &[String]) -> bool {
    if exclusions.is_empty() {
        return false;
    }
    match foreground_process_name() {
        Some(name) => exclusions.iter().any(|e| e.eq_ignore_ascii_case(&name)),
        None => false,
    }
}
