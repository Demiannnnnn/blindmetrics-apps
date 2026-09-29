pub mod active_window;
pub mod files;
pub mod idle_time;
pub mod keystroke_counter;
pub mod location;
pub mod network;
pub mod screenshot;

use crate::core::metrics::SystemItem;
use chrono::Utc;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

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
    /// Runs on controlled intervals (30-60 seconds) to avoid CPU load (PERF-03).
    pub fn collect(&mut self) -> SystemItem {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        let cpu = self.sys.global_cpu_info().cpu_usage();
        let mem_used = self.sys.used_memory() / 1024 / 1024;
        let mem_total = self.sys.total_memory() / 1024 / 1024;
        let (app, title) = active_window::ActiveWindowCollector::get_active_window();
        let idle = idle_time::IdleTimeCollector::get_idle_seconds();

        SystemItem {
            ts: Utc::now(),
            foreground_app: app,
            window_title: title,
            idle_seconds: idle,
            cpu_pct: cpu,
            mem_used_mb: mem_used,
            mem_total_mb: mem_total,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            usb_events: Vec::new(),
        }
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_system_collector_returns_valid_metrics() {
        let mut collector = SystemCollector::new();
        let metric = collector.collect();

        // CPU percentage range [0.0, 100.0 * num_cpus]
        assert!(metric.cpu_pct >= 0.0);
        // Total memory must be greater than zero
        assert!(metric.mem_total_mb > 0);
        assert!(metric.mem_used_mb <= metric.mem_total_mb);
    }

    #[test]
    fn test_system_collector_performance() {
        let mut collector = SystemCollector::new();

        let start = Instant::now();
        let _ = collector.collect();
        let elapsed = start.elapsed();

        // DoD 3: collect() executes quickly (typically < 10ms for refresh_cpu_usage and refresh_memory)
        tracing::info!("SystemCollector::collect elapsed: {:?}", elapsed);
        assert!(
            elapsed.as_millis() < 500,
            "Collector should not stall the thread (took {:?})",
            elapsed
        );
    }
}
