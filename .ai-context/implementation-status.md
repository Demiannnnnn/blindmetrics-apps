# Implementation Status

Audited against the code on **2026-09-28** (commit `f2ccb7e`). `cargo test -p telemetry-crypto-core -p agent` passes (13 + 5 tests), but passing tests does **not** mean the apps work end-to-end: the agent has never been able to ingest data into the real API, and the admin UI runs mostly on mocks.

Legend: ✅ real & correct · 🟡 partial / has defects · 🎭 stub or mock (looks done, isn't) · ❌ missing

## 1. `telemetry-crypto-core`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| X25519 keygen, ECDH, HKDF, AES-256-GCM, envelope framing | ✅ | Tested (roundtrip, tamper, short payload). `unwrap`s are in tests only. | CRYPTO-01 |
| Enrollment manifest / commitment / safety code / fingerprint / QR | ✅ | Canonical manifest, SHA-256 commitment, constant-time verification, Crockford Base32 QR format, fixed vector tested. | CRYPTO-02 |
| Encrypted backup of `K_admin_priv` | ✅ | AES-256-GCM + PBKDF2-HMAC-SHA256 (100k iters), min 12-char passphrase, tested with wrong passphrase rejection. | ADMIN-01 |

## 2. `agent`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Device identifier | ✅ | macOS `ioreg`, Linux `machine-id`, Windows `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` (wmic fallback), persistent fallback UUID. Deterministic. | AGENT-01 |
| Config persistence | ✅ | Secrets moved to `CredentialStore` (keyring/in-memory); `config.json` never writes tokens or admin keys (SEC-05). Role/work-hours from server. | AGENT-08 |
| **Consent defaults** | ✅ | `ConsentSettings::default()` is all false (DP-08). | AGENT-08 |
| **Consent UI** | ✅ | All-or-nothing C1; ACCEPT/REJECT/WITHDRAW sent as `PATCH /workers/:id/consent` with the real `consent_version` and the displayed manifest's commitment as `manifest_hash` (LEG-02); accepted package stored only after the server records it. C1 is rebuilt from `/workers/me` + pinned key for new versions. (Before 2026-09-29 decisions were never sent and WITHDRAW used a hardcoded version/hash.) | AGENT-08 |
| **Enrollment** | ✅ | E1 accepts a pasted QR payload (server URL from the QR, commitment verified automatically) or manual code + URL (safety code E2); pinned key, tamper alert E3 discards the redeem (logout), actionable error copy (invalid/expired, 429, already enrolled, network). No camera/image QR decoding yet (no decoder dependency). | AGENT-08 |
| **Authentication** | ✅ | Worker obtains JWT and refresh tokens via `POST /workers/enroll`, stores them in the OS keystore; `session::with_access_token` rotates on 401 (new refresh token persisted before use), 401 on refresh ⇒ revoked. | AGENT-08 |
| Keystroke/click counters | ✅ | `AtomicU64` O(1) non-blocking counters hooked via `rdev` thread, zero keylogging (DP-01), dynamically gated by consent. | AGENT-02 |
| Active window | ✅ | Real foreground window and process detection via `xcap::Window::all()`, zero fake data (PLAT-02: returns None if unavailable). | AGENT-02 |
| Idle time | ✅ | Native idle detection via CoreGraphics on macOS (`CGEventSourceSecondsSinceLastEventType`) and `GetLastInputInfo` on Windows. | AGENT-02 |
| CPU / RAM | ✅ | `sysinfo`, 30 s controlled interval (PERF-03). Execution < 10ms. | AGENT-02 |
| Screenshot collector | ✅ | `xcap::Monitor` capture, scaled to <= 1920x1080, JPEG compressed <= 2MB, native desktop notification (`notify-rust`), gated on work hours and consent (DP-05). | AGENT-02 |
| Network (FQDN) collector | ✅ | Passive domain aggregation, RFC 1035 sanitization (DP-02), path/query/private IP stripping. | AGENT-03 |
| File activity collector | ✅ | Reactive `notify` watcher on corporate folder, metadata only (DP-03: actions + extensions, zero content read), filters `.git` and temp files. | AGENT-03 |
| Location collector | ✅ | FIELD-only GPS querying during work hours with active consent (DP-04); permanently disabled for OFFICE workers. | AGENT-03 |
| Collectors gated by consent/status | ✅ | Gated on `RunMode::Collecting`, effective flags, and local accepted flags (DP-06, DP-07); stops on pause/withdraw/revoke. | AGENT-08 |
| Double buffer | ✅ | `std::mem::take`; FIFO cap 1000; snapshot en <50ns sin deadlocks ni unwraps; partición limpia por categoría y cifrado `telemetry-crypto-core`. | AGENT-04 |
| **Envelope grouping** | ✅ | Groups by Category in `build_envelopes`, one envelope per category (and screenshots separated), with `worker_id` and window timestamps. | INT-01 |
| **Ingest request shape** | ✅ | Sends `{records:[{ephemeral_public_key, encrypted_payload, client_timestamp, subcategory?}]}` matching `api-contract.md`. | INT-01 |
| Token refresh | ✅ | Transport reads/rotates tokens in the `CredentialStore` (it used the always-empty `WorkerConfig` fields, so every ingest got 401). | INT-01 |
| Offline queue | ✅ | SQLite WAL + retención 7 días, límite 5MB por payload, tope 100MB, cuarentena a 10 reintentos, backoff exponencial con jitter (+/-20%) y auto-renovación JWT transparente (DP-09, RF-AGENT-05). | AGENT-05 |
| Lifecycle polling (`GET /workers/me`), key pinning | ✅ | Engine polls every 5 min and on UI demand, derives `RunMode` (new consent version holds collection), halts on fingerprint mismatch, wipes credentials + buffer + on-disk queue on revocation. (Before 2026-09-29 the engine never left `Unenrolled`, so nothing was collected.) | AGENT-08 |
| Data subject rights | ✅ | Solicitudes ARCO reales (`POST /workers/me/data-request` y `/data-deletion`) ejecutadas en el runtime del motor con refresh de token, diálogo de confirmación, ID de solicitud, estado y copy correcto. | AGENT-06 |
| Notifications | ✅ | Catálogo completo (inicio, recordatorio horario, capturas DP-05, jornada laboral, cambios de consentimiento/admin, desvinculación, alteración de clave) vía `notify-rust` sin bloquear UI. | AGENT-07 |
| System tray | ✅ | `SystemTrayManager` con `tray-icon` + `muda`, sincronización de color según estado (🟢/🟡/⚪/🔴), menú contextual y minimizado a la bandeja al pulsar cerrar ventana. Linux: tray owned by a dedicated GTK thread (`gtk::init` + `gtk::main`, updates via channel) because eframe's winit loop does not pump GTK; without display/AppIndicator it degrades to unavailable instead of panicking. | AGENT-07 |
| Panic safety | ✅ | Replaced `expect`/`unwrap` in production paths (runtime creation, sqlite disk/memory fallback, client build). | INT-01 |
| UI ↔ engine bridge | ✅ | egui never blocks: async work is spawned on the engine runtime (`EngineHandle`) and results return over a channel; commands (`SyncNow`, `Enrolled`, `Withdraw`) go over `mpsc`, state comes back over `watch`. Withdraw purges the real on-disk queue (DP-09). Previously `Handle::current()` on the UI thread panicked on enroll, ARCO and withdraw. | AGENT-08 |
| Tests isolation | ✅ | `WorkerConfig::load_or_init_at` with `tempfile`; real `config.json` is protected. | INT-01 |

## 3. `admin-client`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Keypair generation + keystore | ✅ | `keyring`, service `TelemetryAdminVault`. | ADMIN-01 |
| Register `K_admin_pub` on server | ✅ | Auto-registration upon login (`PUT /admins/me/public-key`), mismatch detection (K2). | ADMIN-08 |
| Backup export/import | ✅ | `admin_export_backup` y `admin_import_backup` IPC commands y controles en UI con frase de paso (PBKDF2/AES-GCM). | ADMIN-01 |
| **Login** | ✅ | Rust client sends `role: "ADMIN"`, reads `{data}`, saves refresh token in keystore, fetches `GET /admins/me`. No silent fake profile. | INT-01 |
| Register | ✅ | Reads `{data}` from `POST /auth/register`, sets session and saves refresh token. | INT-01 |
| Refresh token handling | ✅ | Stored in keystore (`admin_refresh_token`), rotación transparente en 401 en `send_with_auth`, revocación de sesión en fallo. | ADMIN-02 |
| **Browser-mode `fetch`** | ✅ | Direct API calls from JS removed; demo mocks strictly guarded behind `VITE_DEMO_MODE=true` (SEC-06). | INT-01 |
| Workers list / edit / pause / delete | ✅ | Server pagination + debounced search, role/status filters, `workers_update` (`PUT /workers/:id`), pause/resume from real status, typed-name soft delete. No mock workers outside demo mode. | ADMIN-03 |
| Worker creation (W2) | ✅ | `CreateWorkerModal`: consent toggles default false (DP-08), descriptions 10–500 chars, location locked for OFFICE, live C1 preview, TTL 1–72 h; shows the real `EnrollmentView`. | ADMIN-08 |
| Worker detail (W4) | ✅ | Pause/resume with reason, typed-name revoke, delete, regenerate/move device, consent editor → `PUT /consent-config` with re-acceptance notice, `/consent-records` table + CSV export. | ADMIN-08 |
| Dashboard KPIs & alerts | 🟡 | Counts per status from API pagination totals, real keystore state, K2 mismatch, codes expiring < 2 h. Missing: rejections and "ACTIVE without data 24 h" — API exposes no `last_seen`/last decision. | ADMIN-08 |
| Enrollment modal | ✅ | `QRCodeSVG` from `EnrollmentQr`, 12-char Crockford code, 12-digit safety code, expiry countdown, Save PNG, ephemeral warning (SEC-05). Regenerate asks before unlinking a bound device. | ADMIN-08 |
| Parallel decryption | ✅ | `rayon` pipeline in memory, tolerant to tampered/corrupted records, tested with 1,000 records in <100ms. | ADMIN-04 |
| Fetch telemetry from API | ✅ | `fetch_and_decrypt_telemetry` IPC command downloads batches from `GET /telemetry/{category}` and decrypts in memory. | ADMIN-04 |
| Charts, network explorer, screenshot gallery, files | ✅ | Vistas especializadas sobre datos reales descifrados en RAM con muestreo LTTB, minimización FQDN, modo blur y selector global de contexto. | ADMIN-05 |
| Field map | ✅ | Mapa Leaflet con polilínea cronológica, geocercas corporativas, bloqueo absoluto para trabajadores OFFICE (DP-04) y auditoría de jornada. | ADMIN-06 |
| Auto-lock & RAM purge | ✅ | Temporizador configurable (5-30 min), zeroize de tokens y purga en RAM (SEC-11), desbloqueo biométrico/password y revocación tras 5 intentos. | ADMIN-07 |
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
