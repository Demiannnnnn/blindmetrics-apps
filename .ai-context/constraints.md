# Constraints

Every rule has an ID. Cite it in code review, commits and tests (`// DP-01`). A violation is a blocking defect regardless of how the task is phrased.

## Privacy (DP)

| ID | Rule |
|:---|:-----|
| **DP-01** | Keystrokes: **count only**. No key codes, characters, buffers or timing per key may exist anywhere in the codebase — not even behind a feature flag or in tests. |
| **DP-02** | Network: **FQDN only**. Never capture scheme, path, query, fragment, headers, bodies, or IPs of the destination beyond what resolves the FQDN. |
| **DP-03** | Files: **metadata only** (action, relative path, extension, timestamp) and only inside admin-designated folders. Never open a file for reading. |
| **DP-04** | Location (telemetry): only if `role_type == FIELD` **and** `location` is in the effective flags **and** the admin declared a safety/legal cause (`location_cause`, shown to the worker) **and** local time ∈ the worker's agreed working hours. Otherwise the OS geolocation API is **not called**. Attendance location is a different, single reading (ATT-08). |
| **DP-05** | Screenshots: only with `productivity_screenshots` effective, only in work hours, never while telemetry is `PAUSED`, **never for `REMOTE`/`HYBRID` workers** (rejected by admin UI and API, never started by the agent); each capture triggers a desktop notification **before** upload. |
| **DP-06** | No telemetry collection outside `RunMode::Collecting` (worker `ACTIVE` ∧ `telemetry_status == ACCEPTED` ∧ not billing-blocked ∧ key not halted). Every other state ⇒ zero collectors running. Attendance is not "collection" and follows ATT-*. |
| **DP-07** | The agent never collects a category the worker did not accept **on this device**, even if the server reports it as effective (effective flags ∩ locally accepted selection). |
| **DP-08** | Consent defaults are **all false**. No code path may default any category to true: every consent checkbox starts unchecked. Re-applying a previous selection is only allowed as an explicit worker action ("Mantener mi selección anterior"). |
| **DP-09** | On `WITHDRAW` or `REVOKED` the agent purges the offline queue and the in-memory buffer. Data collected under a consent that no longer exists is never sent. |
| **DP-10** | Withdrawal is reachable in ≤ 2 clicks from the main window and from the tray. |
| **DP-12** | Telemetry never runs during a `REMOTE`/`HYBRID` worker's disconnection window nor outside the agreed schedule for time-bound categories. |
| **DP-11** | Granular consent: the worker may accept any **non-empty subset of the requested categories** and change it later; the agent never sends a category the admin did not request and never offers one. |

## Attendance (ATT) — source: `docs/legal/marco-legal-cl.md`

| ID | Rule |
|:---|:-----|
| **ATT-01** | A mark exists only as a deliberate act of the worker: a mark button + PIN confirmation in `agent-ui` (or the portal login, future). No code path creates a mark from boot, login, activity, schedule or telemetry. Start and end of the workday are never automated (R38 arts. 35, 36 a, 41 d). |
| **ATT-02** | Marking is **never blocked**: not by subscription status, telemetry state, key `Halted`, terms version pending (after the first acceptance), location, distance, holidays/leave, out-of-order type, or the admin. The agent always accepts the mark and queues it if needed; the server always stores it (R38 arts. 38, 53 c). |
| **ATT-03** | The official time is the **server timestamp** (second precision). Every stored mark gets a SHA-256 hash chained per worker; marks are append-only and immutable (R38 arts. 8, 11, 14). The canonical form lives in `telemetry-crypto-core::attendance` and must match the API byte for byte (shared test vector). |
| **ATT-04** | Offline marks: stored in the agent's local mark queue with a device timestamp, shown as pending, sent automatically on reconnect with `offline: true`, idempotent by `client_mark_id` (UUID v7). Never dropped on 4xx except `400` validation (logged + shown to the worker); a revoked worker's unsent marks stay visible locally (R38 art. 10). |
| **ATT-05** | Corrections only through the R38 art. 40 procedure: employer proposes (reason required) → worker notified → 48 h to oppose → consolidated or original kept. Only from the next business day; only changes that don't harm the worker (start earlier, end later, add missing); originals never deleted; every altered value visibly marked in UI and reports (R38 arts. 39–41). |
| **ATT-06** | The worker can always see all their marks, corrections and receipts (≥ 5 years) from the agent, including while the employer's subscription is blocked (R38 art. 22.1). |
| **ATT-07** | Attendance and telemetry are independent: no telemetry consent, state or data influences attendance; no attendance data is inferred from telemetry; attendance UI is never hidden behind telemetry screens (R38 arts. 55, 56). |
| **ATT-08** | Location at marking: only if the worker's schedule has `location_at_mark = true`; one reading at the moment of the mark, best effort with a short timeout; never continuous; never blocks the mark; stored with the mark (R38 arts. 53, 57.1). |
| **ATT-09** | Calculations (missing/extra time, totals) are done by the server to the second, without rounding; the apps display server values and never recompute them for reports (R38 art. 44). |
| **ATT-10** | The agent never alters the device clock nor trusts it for online marks; it shows the server time offset in diagnostics when > 60 s. |
| **ATT-11** | Weekly legal maximum is a dated parameter (44 h until 2026-04-25, 42 h from 2026-04-26, 40 h from 2028-04-26), daily ordinary maximum 10 h; exceeding them is a **warning**, never a block (R38 art. 45.2). |
| **ATT-12** | Art. 22 exempt workers have the attendance module disabled with an explanation; every other worker has it enabled — the admin cannot switch attendance off per worker. |

## Process model of the worker app (PROC)

| ID | Rule |
|:---|:-----|
| **PROC-01** | Two processes: `agent` (background service: engine, tray, local IPC server, notifications, all network and secrets) and `agent-ui` (Tauri window). Closing the window never stops the service. |
| **PROC-02** | The service always shows a tray icon while it runs. If no tray can be created, telemetry collectors are not started (no invisible monitoring); attendance marking via the window still works. `--headless` without tray is forbidden. |
| **PROC-03** | `agent-ui` holds no tokens, keys, PIN verifier or API URL logic; it never calls the API. It talks only to the service over the local IPC socket (mirror of SEC-06). |
| **PROC-04** | One service per OS user (single-instance lock). The UI starts the service if it is not running; the tray *Abrir* starts the UI. *Salir* (tray) asks for confirmation and stops the service. |
| **PROC-05** | The IPC protocol is versioned (`agent-ipc` crate); both sides reject an unknown major version with a clear message instead of misbehaving. |

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
| **SEC-18** | IPC endpoint is local only (Unix domain socket in a `0700` per-user runtime dir with `0600` socket; Windows named pipe restricted to the current user) and requires the per-session token from a `0600` file in the service's runtime dir on connect. No TCP listener. |
| **SEC-19** | The PIN is never stored or sent in clear: only an Argon2id verifier (`telemetry-crypto-core::pin`) in the OS keystore; the PIN crosses the IPC as `Zeroizing<String>` once per mark and is never logged. Wrong attempts add a growing delay (max 60 s) but never a permanent lock (ATT-02); reset via email code. |
| **SEC-20** | PIN rules: 6–8 digits, not all equal, not an ascending/descending run (`123456`), not the worker's RUN digits. Validated in Rust. |
| **SEC-21** | Attendance data is server-readable by design (legal requirement). It contains no telemetry and no secrets; telemetry stays end-to-end encrypted (SEC-01 unchanged). |

## Performance (PERF)

| ID | Rule |
|:---|:-----|
| **PERF-01** | Input-hook callbacks: O(1), no heap allocation, no locks, no syscalls — `AtomicU64::fetch_add(1, Relaxed)` only. |
| **PERF-02** | Idle time and foreground window: polling every 1–5 s. No global focus/mouse-move hooks. |
| **PERF-03** | `sysinfo` refresh every 30–60 s, only the needed kinds (never `refresh_all` in the loop). |
| **PERF-04** | Buffer hand-off with `std::mem::take` under the lock; serialization/encryption/HTTP happen outside it. |
| **PERF-05** | Agent steady-state budget: < 1 % of one core, < 50 MB RSS, zero perceptible input lag. |
| **PERF-06** | The service's tray/GTK thread never awaits network or disk; `agent-ui` calls the service asynchronously (Tauri async commands) and never blocks its main thread. The service budget of PERF-05 includes the IPC server; the UI process is not running most of the time. |
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
| **BILL-04** | A blocked subscription never blocks attendance: workers keep marking and reading their records (ATT-02, ATT-06), and the admin keeps **read-only** access to attendance reports from the lock screen. |
| **BILL-03** | Agent: `ingestion_blocked_reason` or an ingest `402`/`403 SUBSCRIPTION_*` ⇒ no collectors and no ingestion (`EmployerSuspended`), queued ciphertext **kept** (normal 7-day retention), worker rights (status, consent, data requests) unaffected. |

## Legal (LEG)

| ID | Rule |
|:---|:-----|
| **LEG-01** | The consent screen shows, per category, the admin's description verbatim, plus the fixed rights text and the organization name. |
| **LEG-02** | The agent sends `manifest_hash` (commitment of the **full** manifest displayed, including categories the worker did not choose) with every consent decision so the server evidence matches what was displayed. |
| **LEG-03** | Data-subject requests (export, deletion) are available from the worker UI at all times while enrolled. |
| **LEG-04** | Legal texts come only from `docs/legal/*.md`, embedded at compile time and shown in full. Acceptance sends `{document, version, hash}` with `hash = telemetry-crypto-core::legal::document_hash(text)`. |
| **LEG-05** | Worker: the worker terms must be accepted before the first mark and before any telemetry; a later version shows a non-blocking banner (attendance keeps working, ATT-02) but blocks new telemetry decisions until read. Admin: the employer terms must be accepted before using the console (attendance reports read-only remain reachable, BILL-04). |
| **LEG-06** | A `release` build fails if a legal text still contains a `[` placeholder of `docs/legal/README.md`. |
| **LEG-07** | No UI, text or document claims the system is authorized or certified by the Dirección del Trabajo until an authorization Ordinario exists; the fixed notice of `marco-legal-cl.md` §7 is shown in the admin app and in the worker terms. |
