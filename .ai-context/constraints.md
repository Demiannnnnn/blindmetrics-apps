# Constraints

Every rule has an ID. Cite it in code review, commits and tests (`// DP-01`). A violation is a blocking defect regardless of how the task is phrased.

## Privacy (DP)

| ID | Rule |
|:---|:-----|
| **DP-01** | Keystrokes: **count only**. No key codes, characters, buffers or timing per key may exist anywhere in the codebase — not even behind a feature flag or in tests. |
| **DP-02** | Network: **FQDN only**. Never capture scheme, path, query, fragment, headers, bodies, or IPs of the destination beyond what resolves the FQDN. |
| **DP-03** | Files: **metadata only** (action, relative path, extension, timestamp) and only inside admin-designated folders. Never open a file for reading. |
| **DP-04** | Location: only if `role_type == FIELD` **and** `location` is in the effective flags **and** local time ∈ `[work_hours_start, work_hours_end)` in the worker's timezone. Otherwise the OS geolocation API is **not called**. |
| **DP-05** | Screenshots: only with `productivity_screenshots` effective, only in work hours, never while `PAUSED`; each capture triggers a desktop notification **before** upload. |
| **DP-06** | No collection outside `RunMode::Collecting`. `PENDING_CONSENT`, `PAUSED`, `REVOKED`, `Halted` ⇒ zero collectors running. |
| **DP-07** | The agent never collects a category the worker did not accept **on this device**, even if the server reports it as effective. |
| **DP-08** | Consent defaults are **all false**. No code path may default any category to true. |
| **DP-09** | On `WITHDRAW` or `REVOKED` the agent purges the offline queue and the in-memory buffer. Data collected under a consent that no longer exists is never sent. |
| **DP-10** | Withdrawal is reachable in ≤ 2 clicks from the main window and from the tray. |

## Security (SEC)

| ID | Rule |
|:---|:-----|
| **SEC-01** | The server never receives plaintext telemetry, `K_admin_priv`, a DEK or a shared secret. |
| **SEC-02** | All crypto in `telemetry-crypto-core`. No `aes-gcm`/`x25519-dalek`/`hkdf` imports in `agent` or `admin-client-core`. |
| **SEC-03** | Secrets (`K_admin_priv`, ephemeral keys, shared secrets, DEKs, passphrases) live in `Zeroizing<…>` and are never logged, serialized to disk in plaintext, or sent to the WebView. |
| **SEC-04** | `K_admin_priv` only in the OS keystore (or an encrypted backup file). Never `localStorage`, config files, env vars. |
| **SEC-05** | Tokens: admin access token in Rust RAM only; refresh tokens in the OS keystore. Never in the WebView, `localStorage`, logs or `config.json`. |
| **SEC-06** | The WebView never talks to the API directly. CSP keeps `default-src 'self'` and adds no `connect-src` to the API origin. |
| **SEC-07** | The agent verifies the enrollment commitment (QR) or requires safety-code confirmation before storing the key. Mismatch ⇒ abort and wipe. |
| **SEC-08** | The agent encrypts only to the **pinned** key and never replaces it from a server response. Fingerprint mismatch in `GET /workers/me` ⇒ `Halted`, stop sending, alert the worker. |
| **SEC-09** | Admin decryption verifies the inner `worker_id` and `category` against the record metadata; mismatches are shown as integrity errors, never as data. |
| **SEC-10** | Release builds use `https://` API URLs only. `http://` allowed only in debug builds. |
| **SEC-11** | Admin auto-lock (default 5 min, configurable 1–60): clears decrypted data from UI state and Rust memory, drops the access token, requires re-login. |

## Performance (PERF)

| ID | Rule |
|:---|:-----|
| **PERF-01** | Input-hook callbacks: O(1), no heap allocation, no locks, no syscalls — `AtomicU64::fetch_add(1, Relaxed)` only. |
| **PERF-02** | Idle time and foreground window: polling every 1–5 s. No global focus/mouse-move hooks. |
| **PERF-03** | `sysinfo` refresh every 30–60 s, only the needed kinds (never `refresh_all` in the loop). |
| **PERF-04** | Buffer hand-off with `std::mem::take` under the lock; serialization/encryption/HTTP happen outside it. |
| **PERF-05** | Agent steady-state budget: < 1 % of one core, < 50 MB RSS, zero perceptible input lag. |
| **PERF-06** | The egui thread never awaits network or disk; it reads shared state and sends commands over channels. |
| **PERF-07** | Admin decryption runs in `spawn_blocking` + `rayon`; the UI shows two-phase progress (download → decrypt). |

## Platform (PLAT)

| ID | Rule |
|:---|:-----|
| **PLAT-01** | Targets: Windows 10/11, macOS 12+, Linux (X11 and Wayland). Every collector documents per-OS behaviour and degrades to "unavailable" (reported in diagnostics), never to fake data. |
| **PLAT-02** | No hard-coded placeholder values in production paths (e.g. returning `"Finder"` as the active window). Unimplemented ⇒ `Err(Unsupported)` / `None`. |
| **PLAT-03** | macOS Accessibility / Screen Recording and Wayland portal permissions are requested explicitly with an explanation screen; denial is shown in diagnostics. |

## Legal (LEG)

| ID | Rule |
|:---|:-----|
| **LEG-01** | The consent screen shows, per category, the admin's description verbatim, plus the fixed rights text and the organization name. |
| **LEG-02** | The agent sends `manifest_hash` with every consent decision so the server evidence matches what was displayed. |
| **LEG-03** | Data-subject requests (export, deletion) are available from the worker UI at all times while enrolled. |
