//! Reads the currently selected text via UI Automation — WITHOUT sending any
//! synthetic keystroke.
//!
//! The floating button used to fire a synthetic Ctrl+C on every mouse drag just
//! to "check" whether text was selected, which wrecked non-text apps: e.g.
//! space-drag panning in Photoshop was seen as a drag, the blind Ctrl+C ran, and
//! Photoshop switched tools. UIA queries the focused element's text selection
//! directly, so it only yields text when there genuinely is one, and never
//! disturbs the foreground app.

#[cfg(target_os = "windows")]
pub fn selection_via_uia() -> Option<String> {
    use windows::core::Interface;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
    };

    unsafe {
        // Tokio blocking threads aren't COM-initialized; do it per call and
        // balance it. RPC_E_CHANGED_MODE (thread already STA) → don't uninit.
        let inited = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();

        let result = (|| -> Option<String> {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let element = automation.GetFocusedElement().ok()?;
            let pattern = element.GetCurrentPattern(UIA_TextPatternId).ok()?;
            let text_pattern: IUIAutomationTextPattern = pattern.cast().ok()?;
            let selection = text_pattern.GetSelection().ok()?;
            if selection.Length().ok()? < 1 {
                return None;
            }
            let range = selection.GetElement(0).ok()?;
            let text = range.GetText(-1).ok()?.to_string();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })();

        if inited {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(target_os = "windows"))]
pub fn selection_via_uia() -> Option<String> {
    None
}
