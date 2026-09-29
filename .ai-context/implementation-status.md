# Implementation Status

Audited against the code on **2026-09-28** (commit `f2ccb7e`). `cargo test -p telemetry-crypto-core -p agent` passes (13 + 5 tests), but passing tests does **not** mean the apps work end-to-end: the agent has never been able to ingest data into the real API, and the admin UI runs mostly on mocks.

Legend: ✅ real & correct · 🟡 partial / has defects · 🎭 stub or mock (looks done, isn't) · ❌ missing

## 1. `telemetry-crypto-core`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| X25519 keygen, ECDH, HKDF, AES-256-GCM, envelope framing | ✅ | Tested (roundtrip, tamper, short payload). `unwrap`s are in tests only. | CRYPTO-01 |
| Enrollment manifest / commitment / safety code / fingerprint / QR | ✅ | Canonical manifest, SHA-256 commitment, constant-time verification, Crockford Base32 QR format, fixed vector tested. | CRYPTO-02 |
| Encrypted backup of `K_admin_priv` | ❌ | | ADMIN-01 |

## 2. `agent`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Device identifier | 🟡 | macOS `ioreg`, Linux `machine-id`, Windows `wmic` (deprecated/removed on Windows 11 24H2 → use registry `MachineGuid`). | AGENT-01 |
| Config persistence | ✅ | Secrets moved to `CredentialStore` (keyring/in-memory); `config.json` never writes tokens or admin keys (SEC-05). Role/work-hours from server. | AGENT-08 |
| **Consent defaults** | ✅ | `ConsentSettings::default()` is all false (DP-08). | AGENT-08 |
| **Consent UI** | ✅ | All-or-nothing consent screen (C1), withdrawal confirmation (W) with instant purge (DP-09); per-category toggles removed. | AGENT-08 |
| **Enrollment** | ✅ | QR commitment verification, manual safety code, pinned key, tamper alert (E1–E3), logout on mismatch. | AGENT-08 |
| **Authentication** | ✅ | Worker obtains JWT and refresh tokens via `POST /workers/enroll`, stores in OS keystore, rotates on 401. | AGENT-08 |
| Keystroke/click counters | ✅ | `AtomicU64` O(1) non-blocking counters hooked via `rdev` thread, zero keylogging (DP-01), dynamically gated by consent. | AGENT-02 |
| Active window | ✅ | Real foreground window and process detection via `xcap::Window::all()`, zero fake data (PLAT-02: returns None if unavailable). | AGENT-02 |
| Idle time | ✅ | Native idle detection via CoreGraphics on macOS (`CGEventSourceSecondsSinceLastEventType`) and `GetLastInputInfo` on Windows. | AGENT-02 |
| CPU / RAM | ✅ | `sysinfo`, 30 s controlled interval (PERF-03). Execution < 10ms. | AGENT-02 |
| Screenshot collector | ✅ | `xcap::Monitor` capture, scaled to <= 1920x1080, JPEG compressed <= 2MB, native desktop notification (`notify-rust`), gated on work hours and consent (DP-05). | AGENT-02 |
| Network (FQDN) collector | ✅ | Passive domain aggregation, RFC 1035 sanitization (DP-02), path/query/private IP stripping. | AGENT-03 |
| File activity collector | ✅ | Reactive `notify` watcher on corporate folder, metadata only (DP-03: actions + extensions, zero content read), filters `.git` and temp files. | AGENT-03 |
| Location collector | ✅ | FIELD-only GPS querying during work hours with active consent (DP-04); permanently disabled for OFFICE workers. | AGENT-03 |

| Collectors gated by consent/status | ✅ | Gated on `RunMode::Collecting`, effective flags, and local accepted flags (DP-06, DP-07); stops on pause/withdraw/revoke. | AGENT-08 |
| Double buffer | ✅ | `std::mem::take`; FIFO cap 1000. One `expect` on lock poisoning. | AGENT-04 |
| **Envelope grouping** | ✅ | Groups by Category in `build_envelopes`, one envelope per category (and screenshots separated), with `worker_id` and window timestamps. | INT-01 |
| **Ingest request shape** | ✅ | Sends `{records:[{ephemeral_public_key, encrypted_payload, client_timestamp, subcategory?}]}` matching `api-contract.md`. | INT-01 |
| Token refresh | ✅ | Deserializes `data.access_token` and `data.refresh_token`. | INT-01 |
| Offline queue | 🟡 | SQLite WAL + 7-day purge OK; parameterized `mark_delivered`; drops on 403; purged on withdraw/revoke (DP-09). Faltan tope de tamaño y backoff/jitter. | AGENT-05 |
| Lifecycle polling (`GET /workers/me`), key pinning | ✅ | `ApiClient::get_me`, `derive_run_mode`, pinned key integrity check (Halted on mismatch), revocation wipe on 401. | AGENT-08 |
| Data subject rights | 🎭 | Buttons show a "request sent" dialog **without calling the API**; the text says data is decryptable "with the holder's key" (wrong: it's the admin's key). | AGENT-06 |
| Notifications | 🟡 | Start + hourly via `notify-rust`. No screenshot / work-hours / consent-change notifications. | AGENT-07 |
| System tray | 🎭 | `ui/tray.rs` is a state struct; no tray icon exists (eframe has none; needs `tray-icon`). Closing the window quits the app. | AGENT-07 |
| Panic safety | ✅ | Replaced `expect`/`unwrap` in production paths (runtime creation, sqlite disk/memory fallback, client build). | INT-01 |
| Tests isolation | ✅ | `WorkerConfig::load_or_init_at` with `tempfile`; real `config.json` is protected. | INT-01 |

## 3. `admin-client`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Keypair generation + keystore | ✅ | `keyring`, service `TelemetryAdminVault`. | ADMIN-01 |
| Register `K_admin_pub` on server | ✅ | Auto-registration upon login (`PUT /admins/me/public-key`), mismatch detection (K2). | ADMIN-08 |
| Backup export/import | ❌ | | ADMIN-01 |
| **Login** | ✅ | Rust client sends `role: "ADMIN"`, reads `{data}`, saves refresh token in keystore, fetches `GET /admins/me`. No silent fake profile. | INT-01 |
| Register | ✅ | Reads `{data}` from `POST /auth/register`, sets session and saves refresh token. | INT-01 |
| Refresh token handling | 🟡 | Stored in keystore (`admin_refresh_token`); 401 retry in `get_with_auth`. | ADMIN-02 |
| **Browser-mode `fetch`** | ✅ | Direct API calls from JS removed; demo mocks strictly guarded behind `VITE_DEMO_MODE=true` (SEC-06). | INT-01 |
| **Workers list / create / toggle / KPIs** | ✅ | Full IPC commands (`workers_list`, `workers_create`, `workers_set_status`, `workers_regenerate_token`, etc.) connected in UI; no mock workers outside demo mode. | ADMIN-08 |
| Enrollment modal | ✅ | Interactive `QRCodeSVG` from `EnrollmentQr`, 12-char Crockford code, 12-digit safety code, ephemeral warning (SEC-05). | ADMIN-08 |
| Parallel decryption | ✅ | `rayon` pipeline in memory, tolerant to tampered/corrupted records, tested with 1,000 records in <100ms. | ADMIN-04 |
| Fetch telemetry from API | ✅ | `fetch_and_decrypt_telemetry` IPC command downloads batches from `GET /telemetry/{category}` and decrypts in memory. | ADMIN-04 |
| Charts, network explorer, screenshot gallery, field map | 🎭 | UI components on mock data. | ADMIN-05 / ADMIN-06 |
| Auto-lock | 🟡 | UI timer (5 min) + `admin_lock_and_flush_memory` clears the token; decrypted state purge not verified; not configurable. | ADMIN-07 |
| Command structure | ✅ | Divided into `commands/{mod,session,keys,workers,telemetry}.rs` with structured `CommandError`. | ADMIN-08 |

## 4. Server dependencies

| Needed by apps | Server state |
|:---------------|:-------------|
| Enrollment, `GET /workers/me`, consent decisions, status, admin public key | ❌ `telemetry-api` TASK-12 (designed in `ENROLLMENT.md`, not implemented) |
| Everything else in `api-contract.md` | ✅ implemented |

## 5. Suggested order

1. **INT-01** — make what exists actually talk to the API (login, ingest shape, grouping, envelopes, panics, demo-mode isolation). Unblocks real testing today.
2. **telemetry-api TASK-12** ∥ **CRYPTO-02**.
3. **ADMIN-08** and **AGENT-08** (enrollment + consent lifecycle end-to-end).
4. AGENT-02/03 real collectors → ADMIN-04/05/06 on real data → AGENT-07, ADMIN-07 hardening.
