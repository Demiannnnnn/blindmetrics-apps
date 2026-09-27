# Architecture Overview

## Monorepo Context
The `telemetry-apps` monorepo uses Cargo Workspaces to bundle three crates:
1. `telemetry-crypto-core` (Rust)
2. `telemetry-worker` (Rust + egui)
3. `telemetry-admin` (Rust + Tauri + React)

## Key Workflows
- **Crypto Abstraction:** All Zero-Knowledge HPKE logic lives in the shared crate to ensure cross-app binary compatibility.
- **Background Engine:** The worker uses multi-threaded polling and lock-free snapshotting (`std::mem::take`) to collect events and avoid UI/OS blocking.
- **Admin Engine:** Uses `rayon` for concurrent decryption of bulk telemetry records on fetch.
