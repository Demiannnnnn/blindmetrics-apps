# Implementation Status

Audited against the code on **2026-09-28** (commit `f2ccb7e`). `cargo test -p telemetry-crypto-core -p agent` passes (13 + 5 tests), but passing tests does **not** mean the apps work end-to-end: the agent has never been able to ingest data into the real API, and the admin UI runs mostly on mocks.

Legend: ✅ real & correct · 🟡 partial / has defects · 🎭 stub or mock (looks done, isn't) · ❌ missing

## 1. `telemetry-crypto-core`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| X25519 keygen, ECDH, HKDF, AES-256-GCM, envelope framing | ✅ | Tested (roundtrip, tamper, short payload). `unwrap`s are in tests only. | CRYPTO-01 |
| Enrollment manifest / commitment / safety code / fingerprint | ❌ | | CRYPTO-02 |
| Encrypted backup of `K_admin_priv` | ❌ | | ADMIN-01 |

## 2. `agent`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Device identifier | 🟡 | macOS `ioreg`, Linux `machine-id`, Windows `wmic` (deprecated/removed on Windows 11 24H2 → use registry `MachineGuid`). | AGENT-01 |
| Config persistence | 🟡 | `config.json` stores **access & refresh tokens in plaintext** (SEC-05). Role/work-hours come from env vars instead of the server. | AGENT-08 |
| **Consent defaults** | 🎭❗ | `ConsentSettings::default()` enables system, network, productivity_basic **before any consent** (DP-08). | AGENT-08 |
| **Consent UI** | 🎭 | Per-category checkboxes change local state only; no `PATCH` is ever sent; model contradicts "admin defines, worker accepts". | AGENT-08 |
| **Enrollment** | 🎭 | Pastes a JSON `{api_url, worker_id, admin_public_key, organization}`; no server call, no credentials obtained, no commitment check. | AGENT-08 |
| **Authentication** | ❌ | The agent never obtains a token, so `access_token` is always `None`. Every send is unauthenticated. | AGENT-08 |
| Keystroke/click counters | 🟡 | `AtomicU64` counters correct (PERF-01) but **no OS hook feeds them** (`io/system_hooks.rs` is never registered) → always 0. | AGENT-02 |
| Active window | 🎭 | Returns hard-coded `"Finder"/"Desktop"`, `"Explorer.EXE"` (PLAT-02). | AGENT-02 |
| Idle time | 🎭 | Returns `0` on every OS. | AGENT-02 |
| CPU / RAM | ✅ | `sysinfo`, 30 s. Net bytes & USB missing. | AGENT-02 |
| Network / files / location / screenshots collectors | ❌ | Only structs in `core/metrics.rs`. | AGENT-03 |
| Collectors gated by consent/status | ❌ | Collection loop ignores consent and runs unconditionally (DP-06). | AGENT-08 |
| Double buffer | ✅ | `std::mem::take`; FIFO cap 1000. One `expect` on lock poisoning. | AGENT-04 |
| **Envelope grouping** | 🟡❗ | Encrypts the whole mixed buffer as one envelope and posts it to `batch[0].category()` → system data can land in `/productivity`. | INT-01 |
| **Ingest request shape** | 🎭❗ | Sends `{ephemeral_public_key, encrypted_payload}`; API requires `{records:[{…, client_timestamp}]}` (+ `subcategory` for productivity) → always 422/400. | INT-01 |
| Token refresh | 🟡 | Reads `body.access_token` instead of `body.data.access_token`. | INT-01 |
| Offline queue | 🟡 | SQLite WAL + 7-day purge OK; no size cap, no backoff/jitter, no drop-on-403, no purge on withdraw. `mark_delivered` builds SQL by string concat (ids are ints, but use params). | AGENT-05 |
| Lifecycle polling (`GET /workers/me`), key pinning | ❌ | | AGENT-08 |
| Data subject rights | 🎭 | Buttons show a "request sent" dialog **without calling the API**; the text says data is decryptable "with the holder's key" (wrong: it's the admin's key). | AGENT-06 |
| Notifications | 🟡 | Start + hourly via `notify-rust`. No screenshot / work-hours / consent-change notifications. | AGENT-07 |
| System tray | 🎭 | `ui/tray.rs` is a state struct; no tray icon exists (eframe has none; needs `tray-icon`). Closing the window quits the app. | AGENT-07 |
| Panic safety | 🟡 | `main.rs` `expect` on runtime creation; background `expect` on SQLite open; transport `expect` on client build. With `panic = "abort"` any of these kills the agent silently. | INT-01 |
| Tests isolation | 🟡 | `ui::tests::test_traffic_light_states` calls `WorkerConfig::load_or_init()` → overwrites the developer's real `config.json`. | INT-01 |

## 3. `admin-client`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Keypair generation + keystore | ✅ | `keyring`, service `TelemetryAdminVault`. | ADMIN-01 |
| Register `K_admin_pub` on server | ❌ | Needs API [T12]. | ADMIN-08 |
| Backup export/import | ❌ | | ADMIN-01 |
| **Login** | 🟡❗ | Rust client omits the required `role: "ADMIN"` and reads `body.access_token` (not `data.…`) → fails against the real API. The TS bridge then **silently falls back to a fake offline profile**, so the user appears logged in. | INT-01 |
| Register | 🟡 | Ignores returned tokens and fabricates a random UUID profile. | INT-01 |
| Refresh token handling | ❌ | Not stored; no 401→refresh→retry. | ADMIN-02 |
| **Browser-mode `fetch`** | 🟡❗ | `tauri-bridge.ts` calls the API from JS when not in Tauri (SEC-06). Must be demo-only. | INT-01 |
| **Workers list / create / toggle / KPIs** | 🎭 | `mockWorkers` array in `tauri-bridge.ts`; no Tauri command exists for workers. | ADMIN-03 / ADMIN-08 |
| Enrollment modal | 🎭 | Shows the obsolete JSON package with a fake worker id. | ADMIN-08 |
| Parallel decryption | ✅ | `rayon` pipeline tested with 50 records. | ADMIN-04 |
| Fetch telemetry from API | ❌ | Only `admin_simulate_and_decrypt`. Charts use generated data. | ADMIN-04 / ADMIN-05 |
| Charts, network explorer, screenshot gallery, field map | 🎭 | UI components on mock data. | ADMIN-05 / ADMIN-06 |
| Auto-lock | 🟡 | UI timer (5 min) + `admin_lock_and_flush_memory` clears the token; decrypted state purge not verified; not configurable. | ADMIN-07 |
| Command structure | 🟡 | Single `commands.rs`; errors as `String`. Target: `commands/{auth,keys,workers,telemetry,session}.rs` + `CommandError`. | ADMIN-02 |

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
