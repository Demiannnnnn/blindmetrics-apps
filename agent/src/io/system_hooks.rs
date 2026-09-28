use crate::core::collectors::keystroke_counter::KeystrokeCollector;

/// Cross-platform listener trigger.
/// When OS hooks are active, invokes KeystrokeCollector::record_keystroke() and record_click().
pub struct SystemHooksManager;

impl SystemHooksManager {
    pub fn on_key_event() {
        KeystrokeCollector::record_keystroke();
    }

    pub fn on_click_event() {
        KeystrokeCollector::record_click();
    }
}
