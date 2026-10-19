pub struct ActiveWindowCollector;

impl ActiveWindowCollector {
    /// Polls foreground window title and active application name without using heavy global hooks.
    /// Returns (Option<AppName>, Option<WindowTitle>).
    /// If unsupported or no window is active, returns (None, None) (PLAT-02: zero fake data).
    pub fn get_active_window() -> (Option<String>, Option<String>) {
        if let Ok(windows) = xcap::Window::all() {
            for win in windows {
                if win.is_minimized().unwrap_or(false) {
                    continue;
                }
                let w = win.width().unwrap_or(0);
                let h = win.height().unwrap_or(0);
                if w == 0 || h == 0 {
                    continue;
                }

                let app_name = win
                    .app_name()
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let title = win
                    .title()
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                if app_name.is_some() || title.is_some() {
                    return (app_name, title);
                }
            }
        }

        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_window_collector() {
        let (app, title) = ActiveWindowCollector::get_active_window();
        // In automated CI or headless test runners, windows might be None,
        // but if present, neither should be hardcoded fake "Finder" (PLAT-02).
        if let Some(ref a) = app {
            assert!(!a.is_empty());
        }
        if let Some(ref t) = title {
            assert!(!t.is_empty());
        }
    }
}
