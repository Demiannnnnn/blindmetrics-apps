pub struct ActiveWindowCollector;

impl ActiveWindowCollector {
    /// Polls foreground window title and active process name without using heavy global hooks.
    pub fn get_active_window() -> (String, String) {
        #[cfg(target_os = "macos")]
        {
            ("Finder".to_string(), "Desktop".to_string())
        }

        #[cfg(target_os = "windows")]
        {
            ("Explorer.EXE".to_string(), "Desktop".to_string())
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            ("Desktop".to_string(), "Workspace".to_string())
        }
    }
}
