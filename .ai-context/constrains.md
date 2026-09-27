# Project Constraints

- **Input Latency:** The worker app MUST NOT introduce input lag. Keyboard hooks must be O(1) atomic increments.
- **Performance:** Polling APIs for window states must run every 1-5 seconds, never continuously.
- **Memory Footprint:** Use optimized binaries. Release profiles must strip symbols and use LTO.
- **Cross-OS Compatibility:** Must run on Windows 10+, macOS 12+, and Linux (X11/Wayland). Avoid platform-specific libraries when cross-platform alternatives (e.g., `xcap`, `wgpu`, `notify-rust`) exist, except for necessary OS hooks.
