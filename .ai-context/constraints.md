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
| **DP-07** | The agent never collects a category the worker did not accept **on this device**, even if the server reports it as effective (effective flags ∩ locally accepted selection). |
| **DP-08** | Consent defaults are **all false**. No code path may default any category to true: every consent checkbox starts unchecked. Re-applying a previous selection is only allowed as an explicit worker action ("Mantener mi selección anterior"). |
| **DP-09** | On `WITHDRAW` or `REVOKED` the agent purges the offline queue and the in-memory buffer. Data collected under a consent that no longer exists is never sent. |
| **DP-10** | Withdrawal is reachable in ≤ 2 clicks from the main window and from the tray. |
| **DP-11** | Granular consent: the worker may accept any **non-empty subset of the requested categories** and change it later; the agent never sends a category the admin did not request and never offers one. |

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
| **SEC-12** | Key backups: the **Master Password never leaves the machine** and Argon2id runs only on the client (`telemetry-crypto-core::backup_v2`). The server receives only `encrypted_private_key`, `argon2_params` and the public-key fingerprint. KEK, Master Password and decrypted keys live in `Zeroizing<…>`. |
| **SEC-13** | A key restored from a file or from the server is stored only if its fingerprint equals the key registered on the server. No error path may leave a local key different from the registered one (store → server call → rollback on failure). |
| **SEC-14** | Refresh is **single-flight** in both apps: one refresh at a time, a waiting task reuses the token another task obtained, a failed refresh is never retried with the same token. `401 DEVICE_REVOKED` never triggers a refresh. |
| **SEC-15** | Generating a new admin key when one is registered is destructive (rotation revokes every worker). It requires two explicit confirmations validated **in Rust**: `acknowledge_data_loss = true` and the exact phrase `GENERAR NUEVA LLAVE`. |
| **SEC-16** | Google sign-in on desktop: system browser, loopback redirect on `127.0.0.1` only, PKCE S256, random `state` and `nonce` (checked), one redirect accepted, 180 s timeout. The ID token is only forwarded to the API, never stored. |
| **SEC-17** | URLs opened in the system browser (billing portal) come from the server, are validated (`https`, no credentials; `http://localhost` only in debug) and are never taken from the WebView. |

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

## Billing (BILL)

| ID | Rule |
|:-----|:-----|
| **BILL-01** | The desktop apps never process payments or store billing data. Payment happens only in the web portal; the apps open `portal_url`. |
| **BILL-02** | Admin app: effective subscription `SUSPENDED`/`CANCELLED` ⇒ lock screen instead of the dashboard, decrypted data purged, subscription re-checked on every start. |
| **BILL-03** | Agent: `ingestion_blocked_reason` or an ingest `402`/`403 SUBSCRIPTION_*` ⇒ no collectors and no ingestion (`EmployerSuspended`), queued ciphertext **kept** (normal 7-day retention), worker rights (status, consent, data requests) unaffected. |

## Legal (LEG)

| ID | Rule |
|:---|:-----|
| **LEG-01** | The consent screen shows, per category, the admin's description verbatim, plus the fixed rights text and the organization name. |
| **LEG-02** | The agent sends `manifest_hash` (commitment of the **full** manifest displayed, including categories the worker did not choose) with every consent decision so the server evidence matches what was displayed. |
| **LEG-03** | Data-subject requests (export, deletion) are available from the worker UI at all times while enrolled. |
