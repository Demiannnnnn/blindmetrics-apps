# AI Agent Guidelines (AGENTS.md)

This document contains critical instructions and architectural context for AI assistants and developers working on the `telemetry-apps` monorepo. **Always refer to these rules before making code modifications.**

## 1. Zero-Knowledge & Cryptographic Integrity
- **The Server is Blind:** The telemetry backend (API/PostgreSQL) must NEVER receive plaintext telemetry data or symmetric/private keys.
- **Shared Crypto Core:** All encryption and decryption logic MUST reside in the `telemetry-crypto-core` crate to guarantee binary compatibility. Do not duplicate cryptographic functions in `telemetry-worker` or `telemetry-admin`.
- **Primitives:** Use `x25519-dalek` (key exchange), `hkdf` with SHA-256 (key derivation), and `aes-gcm` (symmetric encryption).
- **Memory Safety (Zeroize):** Immediately securely wipe sensitive variables (ephemeral private keys, derived keys, shared secrets) from RAM using the `zeroize` crate once they are no longer needed.

## 2. Worker Performance & Architecture
- **Ultra-Lightweight Footprint:** The `telemetry-worker` runs continuously on employee machines. It must NEVER cause input lag or excessive CPU consumption.
- **OS Hooks Constraint:** When implementing low-level OS hooks (e.g., keyboard hooks via `WH_KEYBOARD_LL` or `CGEventTapCreate`), the callback must be **O(1) and block-free**. Never allocate memory on the heap (`String`, `Vec`), serialize data, or lock heavy Mutexes within the hook. Only use `AtomicU64` with `Ordering::Relaxed` to count events.
- **Polling vs. Hooks:** For window focus and idle time, use a polling loop (every 1-5 seconds) instead of global event hooks to prevent CPU spikes.
- **Buffer Snapshotting:** When moving telemetry data from the collectors to the HTTP sender thread, use lock-free snapshotting with `std::mem::take` to minimize Mutex hold times.

## 3. Data Minimization & Privacy Compliance
- **No Keystroke Logging:** The worker must NEVER log actual keystrokes or passwords. It only tracks `keystrokes_per_minute` and `clicks_per_minute`.
- **Network Privacy:** Never capture full URLs, GET queries, or HTTP bodies. Only capture the Fully Qualified Domain Name (FQDN) via passive DNS/SNI sniffing.
- **File Privacy:** Only monitor file metadata (creation, modification, deletion) in specific corporate directories. Never read or transmit file contents.
- **Geolocation Rules:** Only collect GPS data if the worker role is `FIELD`, the user has granted consent, and the current local time falls within their configured working hours (`work_hours_start` to `work_hours_end`). Suspend collection automatically outside these hours.

## 4. UI/UX Rules
- **Transparency in Worker:** The `telemetry-worker` interface must always indicate when monitoring is active and allow one-click revocation for any consent.
- **Real-Time Notifications:** Use `notify-rust` to explicitly alert the user when specific monitoring actions occur (e.g., when a screenshot is taken or when work hours begin/end).
- **Admin Security:** The `telemetry-admin` app must auto-lock after a configurable period of inactivity, flushing decrypted data from RAM and requiring re-authentication.

## 5. Branching Strategy
- `dev-architecture`: Core architectural changes and shared components.
- `dev-features`: Implementation of specific features across the workspace.
- `dev-admin/*`: Specific developments for the Tauri Admin app.
- `dev-agent/*`: Specific developments for the Rust/egui Worker app.
