use crate::core::collectors::keystroke_counter::KeystrokeCollector;

/// Cross-platform OS input hooks manager.
/// Dispatches low-level keyboard and mouse events to atomic O(1) counters without keylogging.
pub struct SystemHooksManager;

impl SystemHooksManager {
    /// Starts the background OS hook listener thread.
    pub fn start_hook() {
        KeystrokeCollector::start_hook();
    }

    /// Toggles whether input hooks are actively processing events.
    pub fn set_enabled(enabled: bool) {
        KeystrokeCollector::set_hook_enabled(enabled);
    }

    /// Checks if input hooks are currently processing events.
    pub fn is_enabled() -> bool {
        KeystrokeCollector::is_hook_enabled()
    }

    /// Manual event trigger for testing or simulated input.
    pub fn on_key_event() {
        KeystrokeCollector::record_keystroke();
    }

    /// Manual event trigger for testing or simulated input.
    pub fn on_click_event() {
        KeystrokeCollector::record_click();
    }
}
