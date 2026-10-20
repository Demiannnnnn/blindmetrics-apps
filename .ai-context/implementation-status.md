# Implementation Status

> **2026-10-11 — v3 started.** Baseline on `develop-AGENTS` (= `dev-ecosystem/style-and-fixed-url`
> `e6b80d1`): `cargo fmt` ✅, `cargo test` ✅ (33 + 86 + 91), admin `npm run build` + `npm test` ✅ (28),
> clippy failed with Rust 1.98 (22 ×
> `clippy::result_large_err`), fixed by TASK-INT-03 (2026-10-11, clippy ✅). Everything in §6 below is ❌ until its
> task lands. The audit below (2026-09-30, contract v2) remains accurate for what it covers.

Audited against the code on **2026-09-30** (branch `claude/kind-planck-1ripey`, on top of `4d86bcd`), after closing the **API contract v2** work (`telemetry-api` TASK-13..17 → apps INT-02, CRYPTO-03, AGENT-09, ADMIN-09..12).

Verification run for this audit:

| Check | Result |
|:------|:-------|
| `cargo test --workspace` | ✅ `telemetry-crypto-core` 33 · `agent` 86 · `admin-client-core` 91 |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ clean |
| `cargo clippy --workspace -- -D clippy::unwrap_used -D clippy::expect_used` | ✅ clean (no `unwrap`/`expect` outside tests) |
| `cargo fmt --all --check` | ✅ |
| `cd admin-client && npx tsc --noEmit && npm test && npm run build` | ✅ 28/28 UI tests, production build OK |

Passing tests do **not** replace an end-to-end run: the Tauri window and the egui window were not driven interactively in the audit environment (no display). Everything marked ✅ is covered by unit/integration tests against `wiremock` servers that reproduce the documented API contract, not against a live API.

Legend: ✅ real & correct · 🟡 partial / has defects · 🎭 stub or mock (looks done, isn't) · ❌ missing

## 1. `telemetry-crypto-core`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| X25519 keygen, ECDH, HKDF, AES-256-GCM, envelope framing | ✅ | Tested (roundtrip, tamper, short payload). `unwrap`s are in tests only. | CRYPTO-01 |
| Enrollment manifest / commitment / safety code / fingerprint / QR | ✅ | Canonical manifest, SHA-256 commitment, constant-time verification, fixed vector tested. `EnrollmentQr::to_payload()` produces the server's `qr_payload` (base64 of the JSON) and `parse_payload()` accepts base64 **or** raw JSON, max 4096 chars. | CRYPTO-02, AGENT-09 |
| **Key backup v2 (`.tkb`)** | ✅ | `backup_v2.rs`: Master Password NFC-normalised (not trimmed, ≥ 12 code points, ≤ 1024 bytes) → Argon2id v19 (default m=64 MiB, t=3, p=4, 16-byte salt; accepted ranges validated before deriving) → KEK → AES-256-GCM with AAD `telemetry-key-backup-v2`. Blob `base64(nonce‖tag‖ct)` = 60 bytes. Strict `.tkb` JSON (`deny_unknown_fields`, ≤ 64 KiB), fingerprint check after decrypting (SEC-13), `Zeroizing` everywhere (SEC-02). Reference vector shared with `telemetry-web`, verified independently with Python (`argon2-cffi` + `cryptography`). | CRYPTO-03 |
| Legacy backup v1 (`.vault`) | ✅ | PBKDF2-HMAC-SHA256 import **only** (export is test-only since ADMIN-11); iteration count capped at 10,000,000 to avoid a CPU DoS from a crafted file. | CRYPTO-03 |

## 2. `agent`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Device identifier | ✅ | macOS `ioreg`, Linux `machine-id`, Windows `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` (wmic fallback), persistent fallback UUID. Deterministic. | AGENT-01 |
| Config persistence | ✅ | Secrets in `CredentialStore` (keyring/in-memory); `config.json` never writes tokens or admin keys (SEC-05). Role/work-hours from server. | AGENT-08 |
| Consent defaults | ✅ | `ConsentSettings::default()` is all false; C1 checkboxes start **unchecked** even for categories the admin enabled (DP-08). | AGENT-08, AGENT-09 |
| **Granular consent (C1)** | ✅ | One checkbox per category the admin offers; `ACEPTAR SELECCIÓN (n)` / `RECHAZAR TODO`; "Mantener mi selección anterior" when a new version arrives (categories the worker never accepted are marked "(no aceptada antes)" and stay unchecked). Decisions go to `PATCH /workers/me/consent {accepted, consent_version, manifest_hash, consent_flags}` with the displayed manifest's commitment (LEG-02); the local selection is stored only after the server confirms it and is always intersected with the admin's offer. "Cambiar mi selección" narrows/extends without leaving the app. | AGENT-09 (DP-11) |
| Withdrawal | ✅ | Purges the in-memory buffer and on-disk queue first (DP-09), then sends the **current** `consent_version` + accepted manifest hash; on `409` with a newer current version it retries once with that version. If the request fails (offline) `pending_withdrawal` is persisted and re-sent on every lifecycle poll until confirmed (AGENT-10); collection stays stopped. | AGENT-09 |
| **Enrollment** | ✅ | E1 accepts the QR payload in **base64** (v2 `qr_payload`) or JSON, or manual code (safety code E2); the server URL is fixed at build time (`TELEMETRY_API_URL`) and a QR for another server is rejected; server URL must be `https` (http only for loopback in debug, SEC-10); pinned key; tamper alert E3. Errors mapped by `details.reason`/code (`INVALID_ENROLLMENT_TOKEN`, `ENROLLMENT_ALREADY_COMPLETED`, `DEVICE_ALREADY_ENROLLED`, `ENROLLMENT_LOCKED` with a `Retry-After` countdown). No camera/image QR decoding (no decoder dependency). | AGENT-08, AGENT-09 |
| **Authentication / refresh** | ✅ | JWT + refresh in the OS keystore. **Single-flight refresh** (`static REFRESH_LOCK`): concurrent 401s wait for one rotation and reuse its token, so the API's refresh-family revocation (`TOKEN_REUSE_DETECTED`) is never triggered by the agent itself (SEC-14). `DEVICE_REVOKED`/invalid refresh ⇒ credentials wiped. | AGENT-09 |
| **Employer billing pause** | ✅ | `402 SUBSCRIPTION_SUSPENDED` / `403 SUBSCRIPTION_CANCELLED` on ingest or `GET /workers/me` (`ingestion_blocked_reason`) ⇒ `RunMode::EmployerSuspended { reason }`: collectors stop, already-encrypted envelopes stay queued (not quarantined), yellow tray/header, one notification on entering and one on leaving; the worker's rights (ARCO, withdrawal) keep working (BILL-02, BILL-03). | AGENT-09 |
| Keystroke/click counters | ✅ | `AtomicU64` O(1) counters via `rdev`, zero keylogging (DP-01), gated by consent. | AGENT-02 |
| Active window / idle / CPU-RAM / screenshots | ✅ | Real sources only (PLAT-02); screenshots scaled + JPEG ≤ 2 MB with notification (DP-05). | AGENT-02 |
| Network (FQDN), file activity, location | ✅ | DP-02 sanitisation, DP-03 metadata only, DP-04 FIELD-only in work hours. | AGENT-03 |
| Collectors gated by consent/status | ✅ | `RunMode::Collecting` + effective flags ∩ locally accepted flags (DP-06, DP-07). | AGENT-08, AGENT-09 |
| Double buffer, envelope grouping, ingest shape | ✅ | `std::mem::take`, FIFO cap 1000, one envelope per category, `{records:[…]}`. | AGENT-04, INT-01 |
| Offline queue | ✅ | SQLite WAL, 7-day retention, 5 MB/payload, 100 MB cap, quarantine at 10 retries, exponential backoff with jitter (DP-09). `429` on ingest re-queues with that backoff; `Retry-After` (header or `details.retry_after_seconds`, capped at 24 h) is parsed into `ApiError` and drives the enrollment lock countdown. | AGENT-05, INT-02 |
| Lifecycle polling, key pinning | ✅ | `GET /workers/me` every 5 min and on demand; new consent version holds collection; fingerprint mismatch halts; revocation wipes everything. | AGENT-08 |
| Data subject rights | ✅ | `POST /workers/me/data-request` and `/data-deletion` with confirmation, request id and status. | AGENT-06 |
| Notifications & tray | ✅ | Full catalogue incl. employer-suspended/reactivated; tray colours 🟢/🟡/⚪/🔴; Linux tray on its own GTK thread, degrades without display. | AGENT-07, AGENT-09 |
| Panic safety, UI ↔ engine bridge, test isolation | ✅ | No `unwrap`/`expect` in production paths; egui never blocks (`EngineHandle`, `mpsc` + `watch`); tests use `tempfile`. | INT-01, AGENT-08 |

## 3. `admin-client`

| Capability | State | Notes | Task |
|:-----------|:-----:|:------|:-----|
| Keystore | ✅ | `keyring` service `TelemetryAdminVault`; private key, refresh token and installation id; loaders return `Zeroizing<String>`. | ADMIN-01, ADMIN-10 |
| **No implicit key generation** | ✅ | A key is generated only in the "first key" flow (no key locally or on the server) or after the destructive double confirmation; login, telemetry self-test and worker creation never create one. | ADMIN-11 |
| Register `K_admin_pub` | ✅ | `PUT /admins/me/public-key` only when the server has none; mismatch (K2) opens the recovery wizard instead of overwriting. | ADMIN-08, ADMIN-11 |
| **Key vault (APP-02)** | ✅ | `KeyRecoveryWizard` (K0 no key anywhere, K1 key only on server, K2 mismatch) + `KeyVault` view: export `.tkb` (`telemetry_key_{org}_{YYYY-MM-DD}.tkb` in Downloads, `0600`, never overwrites) with the safe-keeping warning; server backup `POST /admins/me/vault/setup` (encrypted blob + Argon2 params only, SEC-12); recover from **A** file (`.tkb` v2 or legacy `.vault`) or **B** server (`GET /admins/me/vault/backup`) — the restored key must match the registered fingerprint (SEC-13) and the keystore is rolled back on failure; **C** new key only with `acknowledge_data_loss` + phrase `GENERAR NUEVA LLAVE` checked in Rust (SEC-15) → `POST /admins/me/public-key/rotate` (revokes all workers; the server discards the old backup in the same transaction) and the previous local key is restored if the call fails. | ADMIN-11 |
| **Login / register / session** | ✅ | Email + password and **Google** (system browser, loopback `127.0.0.1:<random>`, PKCE S256, `state` + `nonce`, 180 s timeout, SEC-16). Every sign-in sends the device descriptor `{fingerprint: inst-…, name, public_key_fingerprint}`. No fake profile: if `GET /admins/me` fails the refresh token is revoked and the session discarded. | INT-01, ADMIN-10 |
| **Refresh handling** | ✅ | Access token only in Rust memory (`Zeroizing`); **single-flight** refresh behind a `tokio::sync::Mutex`, a request that fails with an already-rotated token reuses the new one (SEC-14); `DEVICE_REVOKED` ⇒ immediate logout without refresh; `session-expired` event to the UI. | ADMIN-02, INT-02 |
| **Typed errors** | ✅ | `ApiError`/`CommandError {code, message, reason?, details?}` from the canonical envelope; only whitelisted `details` keys reach the WebView; UI copy in `lib/errors.ts` keyed by code/reason. | INT-02 |
| **Authorised devices** | ✅ | `GET/PATCH/DELETE /admins/me/devices*`: list with "este equipo", rename, revoke (revoking the current one logs out). Exempt from the subscription block, so a lost device can be revoked while suspended. | ADMIN-10 |
| **Subscription (APP-01)** | ✅ | `GET /subscriptions/me` before showing any data on every start/login/unlock; `SubscriptionLockScreen` replaces the whole console for `SUSPENDED`/`CANCELLED` (copy by reason), "Abrir en navegador" opens only the **server-provided** `portal_url` after validation (https, no credentials, ≤ 2048, SEC-17); re-check button + every 60 s; any `402`/`403 SUBSCRIPTION_*` from any command raises `subscription-blocked` and unmounts decrypted views; `TrialBanner` at ≤ 7 days (also from `X-Trial-Days-Left`). | ADMIN-12 |
| Organization prompt | ✅ | `ORGANIZATION_NAME_MISSING` ⇒ `OrganizationPrompt` (`PUT /admins/me`) and retry; no default organization is invented. | ADMIN-09 |
| Workers list / edit / pause / delete | ✅ | Server pagination, filters, `PUT /workers/:id`, pause/resume, typed-name soft delete. | ADMIN-03 |
| Worker creation (W2) | ✅ | Consent toggles default false (DP-08), descriptions 10–500 chars, location locked for OFFICE, live C1 preview, TTL 1–72 h. | ADMIN-08 |
| **Enrollment package v2** | ✅ | Before showing anything the Rust side recomputes the manifest commitment and safety code from the **local** public key and the returned worker and compares them with the server's `commitment`, `safety_code` and decoded `qr_payload` (API URL validated); any mismatch ⇒ `ENROLLMENT_PACKAGE_MISMATCH`, nothing displayed. The QR renders the verified `qr_payload`. | ADMIN-09 |
| Regenerate / move device | ✅ | `409 ENROLLMENT_ALREADY_COMPLETED` with hint `REVOKE_FIRST` ⇒ dialog that explains revoking the current device first. | ADMIN-09 |
| **Consent evidence** | ✅ | `/workers/:id/consent-records?per_page=100` shows decision (`ACCEPTED`/`REJECTED`/`WITHDRAWN`/`NARROWED`) and accepted categories; CSV with new columns and formula-injection-safe cells. | ADMIN-09 |
| Dashboard KPIs & alerts | 🟡 | Counts per status, keystore state, K2 mismatch, server backup configured, codes expiring < 2 h. Missing: rejections and "ACTIVE without data 24 h" — API exposes no `last_seen`/last decision. | ADMIN-08 |
| Parallel decryption & telemetry fetch | ✅ | `rayon` in memory, tolerant to corrupted records; `GET /telemetry/{category}` with validated category/UUID/subcategory and percent-encoded dates. | ADMIN-04, INT-02 |
| Charts, network explorer, screenshots, files, field map | ✅ | Real decrypted data, LTTB, blur mode, DP-04 lock for OFFICE. | ADMIN-05, ADMIN-06 |
| Auto-lock & RAM purge | ✅ | 5–30 min, zeroize + purge (SEC-11), password unlock with revocation after 5 failures, Google-only accounts can unlock by signing in with Google again. The unverified biometric unlock (gap since ADMIN-07) was removed by ADMIN-13 (2026-10-11): unlock is password or Google only. | ADMIN-07, ADMIN-10 |
| Command structure | ✅ | `commands/{session,keys,vault,devices,subscription,workers,telemetry}.rs`, `client.rs`, `device.rs`, `google_oauth.rs`, `keystore.rs`. | ADMIN-08..12 |

## 4. Server dependencies

| Needed by apps | Server state (`telemetry-api`, branch `claude/kind-planck-1ripey`) |
|:---------------|:-------------------------------|
| Enrollment, `GET /workers/me`, consent decisions, admin public key (TASK-12) | ✅ implemented |
| Hardened schema: normalized emails, refresh-token families, `SYSTEM`/`ANONYMOUS` audit actors, vault/devices/subscription columns (TASK-13) | ✅ implemented |
| Enrollment v2 package (`commitment`, `safety_code`, `qr_payload`), `409 ENROLLMENT_ALREADY_COMPLETED` / `REVOKE_FIRST`, IP lockout, granular consent `PATCH /workers/me/consent` (TASK-14) | ✅ implemented |
| `POST /auth/google`, device descriptor, `/admins/me/devices*`, `401 DEVICE_REVOKED` (TASK-15) | ✅ implemented |
| Key vault `/admins/me/vault/*`, `POST /admins/me/public-key/rotate` (TASK-16) | ✅ implemented |
| Subscriptions `/subscriptions/*`, `402`/`403` guard, `X-Trial-Days-Left`, `ingestion_blocked_reason`, Flow webhook (TASK-17) | ✅ implemented |

## 5. Known gaps

Items 4, 6 and 7 must be fixed before a production release; the rest are limitations.

1. Dashboard alerts for rejections / "ACTIVE without data 24 h" need a `last_seen`/last-decision field in the API (ADMIN-08).
2. The agent cannot decode a QR from the camera or an image; the payload is pasted (AGENT-08).
3. ~~Google sign-in without cancel~~ — closed by ADMIN-19 (2026-10-20).
4. ~~Security: unverified biometric unlock~~ — closed by ADMIN-13 (button and command removed).
5. ~~Admin WebView CSP allowed `unsafe-inline`/`unsafe-eval`~~ — closed by ADMIN-18 (2026-10-19): production `script-src 'self'`, `devCsp` only for `tauri dev`, test scanning `dist/` for `eval`/`new Function`. ADMIN-18 also fixed correction times sent as UTC (`zonedLocalToUtcIso`).
6. ~~Privacy: offline withdrawal not re-sent~~ — closed by AGENT-10 (`pending_withdrawal` persisted and re-sent on every lifecycle poll; `test_offline_withdrawal_sets_pending_and_clears_queue`). Audit 2026-10-18.
7. ~~Privacy: `--headless` without tray~~ — closed by AGENT-10 (flag refused; no tray ⇒ no collectors).
8. Rate limits and lockouts are enforced by the API only; the apps show the `Retry-After` countdown but do not throttle locally.
9. End-to-end runs against a live API + real desktop sessions are still pending; see "Verification" above.


## 6. v3 — attendance, legal, worker app on Tauri (2026-10-11)

| Capability | State | Task |
|:--|:--:|:--|
| Green clippy baseline | ✅ | INT-03 |
| Attendance mark hash + vector, legal hash/placeholders, PIN verifier, RUT | ✅ | CRYPTO-04 |
| Worker app split: `agent` service + `agent-ipc` + tray-only, no egui | ✅ | AGENT-10 |
| Worker window `agent-ui` (Tauri) with parity | ✅ (not yet driven interactively) | AGENT-11 |
| Service: contract v3 lifecycle, terms, PIN, marks + offline queue, corrections, local alerts | ✅ | AGENT-12 (2026-10-15; location at mark not sent until a real OS location backend exists) |
| Window: terms, PIN, attendance home, receipts, records, corrections | ✅ | AGENT-13 (2026-10-15; not run interactively — no display) |
| Admin: employer terms, employer legal profile, workplaces, legal docs; biometric button removed | ✅ | ADMIN-13 |
| Admin: workers v3 (legal data, agreed schedule, optional telemetry, telemetry pause) | ✅ | ADMIN-14 |
| Admin: attendance board, days/marks, corrections | ✅ | ADMIN-15 |
| Admin: R38 reports, exports, read-only reports while blocked | ✅ | ADMIN-16 (2026-10-15; server computes and exports, app validates art. 25 filters, previews and saves; not tested against the real API — v3 not implemented there yet) |
| Admin: absence qualification + `effective_ts` display (contract v3.1) | ✅ | ADMIN-17 (2026-10-15; removing a qualification needs `absence_id` in `DaySummary`, see handoff) |
| `telemetry-api` contract v3 (attendance, legal, employer, emails, reports/exports) | ✅ | API TASK-18..23 (merged 2026-10-16) |
| `telemetry-web`: receipt verification page, worker secondary login, inspection portal | ❌ | web tasks (to be written) |
| DT certification and authorization | ❌ | Owner decision (`docs/legal/marco-legal-cl.md` §8) |

- 2026-10-19: **INT-04 merged** — the worker app lives in `worker-client/` (window at its root + `src-tauri/`, service in `service/`, IPC in `ipc/`), like `admin-client/`; package names unchanged; `npm run tauri build` in each app builds it (worker: sidecar service included). **ADMIN-18 merged** earlier the same day. Known macOS-only test race in `agent-ui-core` → INT-06.
