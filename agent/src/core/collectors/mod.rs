pub mod active_window;
pub mod idle_time;
pub mod keystroke_counter;

use chrono::Utc;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use crate::core::metrics::SystemMetric;

pub struct SystemCollector {
    sys: System,
}

impl SystemCollector {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_all();
        Self { sys }
    }

    /// Samples hardware metrics and foreground window state.
    /// Runs on controlled intervals (30-60 seconds) to avoid CPU load.
    pub fn collect(&mut self) -> SystemMetric {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        let cpu = self.sys.global_cpu_info().cpu_usage();
        let mem_used = self.sys.used_memory() / 1024 / 1024;
        let mem_total = self.sys.total_memory() / 1024 / 1024;
        let (app, title) = active_window::ActiveWindowCollector::get_active_window();
        let idle = idle_time::IdleTimeCollector::get_idle_seconds();

        SystemMetric {
            client_timestamp: Utc::now(),
            cpu_usage_pct: cpu,
            memory_used_mb: mem_used,
            memory_total_mb: mem_total,
            active_app_name: app,
            active_window_title: title,
            idle_seconds: idle,
        }
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}
