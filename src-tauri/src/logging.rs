//! Lightweight file logger for DeepM.
//!
//! Every error the app surfaces (engine failures, download problems, OCR
//! issues, translation errors) is appended here so a user can copy it or
//! attach it to a bug report. The log lives next to the rest of the app data
//! at `%LOCALAPPDATA%/DeepM/logs/deepm.log` and is capped so it never grows
//! without bound.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

/// Hard cap on the log file size. When exceeded we keep the newest half.
const MAX_LOG_BYTES: u64 = 512 * 1024;

static LOG_LOCK: Mutex<()> = Mutex::new(());

/// `%LOCALAPPDATA%/DeepM/logs`
pub fn log_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeepM")
        .join("logs")
}

/// Full path to the rolling log file.
pub fn log_path() -> PathBuf {
    log_dir().join("deepm.log")
}

fn timestamp() -> String {
    // Local wall-clock time without pulling in a heavy date crate.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Render as UTC; precise enough for diagnostics.
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}Z")
}

/// Appends one line to the log file. Best-effort: never panics, never blocks
/// the caller on failure.
pub fn log(level: &str, source: &str, message: &str) {
    let line = format!("[{}] {:<5} {}: {}\n", timestamp(), level, source, message);
    let _guard = LOG_LOCK.lock();
    let dir = log_dir();
    let _ = fs::create_dir_all(&dir);
    let path = log_path();

    // Trim if the file got too big: keep the most recent ~half.
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            if let Ok(mut f) = fs::File::open(&path) {
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    let keep = buf.len().saturating_sub((MAX_LOG_BYTES / 2) as usize);
                    // Advance to the next line boundary so we don't start mid-line.
                    let start = buf[keep..]
                        .iter()
                        .position(|&b| b == b'\n')
                        .map(|p| keep + p + 1)
                        .unwrap_or(keep);
                    let _ = fs::write(&path, &buf[start..]);
                }
            }
        }
    }

    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Convenience for the common case.
pub fn error(source: &str, message: &str) {
    log("ERROR", source, message);
}

pub fn info(source: &str, message: &str) {
    log("INFO", source, message);
}

/// Returns the last `max_bytes` of the log as text (for the bug-report view).
pub fn tail(max_bytes: usize) -> String {
    let path = log_path();
    let Ok(mut f) = fs::File::open(&path) else {
        return String::new();
    };
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    let start = buf.len().saturating_sub(max_bytes);
    // Align to a line boundary.
    let start = buf[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| start + p + 1)
        .unwrap_or(start);
    String::from_utf8_lossy(&buf[start..]).into_owned()
}
