pub struct IdleTimeCollector;

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

impl IdleTimeCollector {
    /// Polls system idle time in seconds using native OS APIs.
    /// Runs on a 1-5 second polling loop to prevent high CPU interrupts.
    /// Returns None if unsupported or if OS call fails (PLAT-02: zero fake data).
    pub fn get_idle_seconds() -> Option<u64> {
        #[cfg(target_os = "macos")]
        {
            // kCGEventSourceStateCombinedSessionState = 0, kCGAnyInputEventType = ~0 (u32::MAX)
            let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
            if seconds >= 0.0 && !seconds.is_nan() && !seconds.is_infinite() {
                Some(seconds as u64)
            } else {
                None
            }
        }

        #[cfg(target_os = "windows")]
        {
            #[repr(C)]
            struct LastInputInfo {
                cb_size: u32,
                dw_time: u32,
            }

            extern "system" {
                fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
                fn GetTickCount() -> u32;
            }

            let mut lii = LastInputInfo {
                cb_size: std::mem::size_of::<LastInputInfo>() as u32,
                dw_time: 0,
            };

            if unsafe { GetLastInputInfo(&mut lii) } != 0 {
                let now = unsafe { GetTickCount() };
                let elapsed_ms = now.wrapping_sub(lii.dw_time);
                Some((elapsed_ms / 1000) as u64)
            } else {
                None
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_time_collector_native() {
        let idle = IdleTimeCollector::get_idle_seconds();
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            assert!(
                idle.is_some(),
                "Idle time should be detected on supported OS"
            );
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert!(idle.is_none());
        }
    }
}
