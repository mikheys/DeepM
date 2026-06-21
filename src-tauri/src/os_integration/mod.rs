pub mod clipboard_util;
pub mod floating;
pub mod hotkeys;
pub mod ocr;
pub mod process;
pub mod tray;
pub mod uia;

// Re-export the high-level interfaces so lib.rs only imports from here.
pub use clipboard_util::{
    capture_selection, copy_selection_to_clipboard, get_selected_text, paste_from_clipboard,
    read_clipboard, restore_clipboard, save_clipboard, snapshot_clipboard, write_clipboard,
};
pub use floating::{
    create_floating_window, hide_floating, is_floating_visible, set_floating_expanded,
    show_floating,
};
pub use hotkeys::{spawn_hook, SharedHookConfig};
pub use process::{
    focus_source_window, foreground_is_excluded, list_app_processes, remember_source_window,
};
pub use tray::setup_tray;
