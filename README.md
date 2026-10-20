# Telemetry Apps

Desktop clients of the Telemetry platform: an **Admin** app (`admin-client`, Tauri 2 + React) and a **Worker** app (`worker-client`, Tauri 2 + React window with background service), sharing one crypto crate. Since 2026-10-11 the main purpose is the **legal attendance record** (Chile, R38) with **optional** zero-knowledge telemetry. They talk to [`telemetry-api`](https://github.com/Demiannnnnn/telemetry-api), a server that stamps, chains and verifies attendance marks while keeping telemetry zero-knowledge (ciphertext only), and to the web portal `telemetry-web` only by opening it in the browser (sign-up, billing, verification, downloads).

> **Project status (2026-10-19 — v3 stage).** Both apps speak the **API contract v3** with legal attendance records (Chile, DT Res. Ex. N°38/2024), versioned terms and privacy policy (`docs/legal/`), and the worker app ported to Tauri 2 with separate processes (background service + window). Verified in the audit: `cargo fmt --all --check` ✅, `cargo clippy --workspace --all-targets -- -D warnings` ✅, `cargo test --workspace` ✅ (390 tests: `telemetry-crypto-core` 51, `agent` 155, `agent-ipc` 21, `agent-ui-core` 9, `admin-client-core` 154), worker UI `npm test` ✅ (26 tests) and `npm run build` ✅, admin UI `npm test` ✅ (91 tests) and `npm run build` ✅.

---

## Contents

- [What the system does](#what-the-system-does)
- [Architecture](#architecture)
- [Repository layout](#repository-layout)
- [Features](#features)
- [Security and privacy guarantees](#security-and-privacy-guarantees)
- [Prerequisites](#prerequisites)
- [Configuration](#configuration)
- [Running in development](#running-in-development)
- [Google sign-in setup (admin app)](#google-sign-in-setup-admin-app)
- [Key backup format (`.tkb`)](#key-backup-format-tkb)
- [How the apps use the API](#how-the-apps-use-the-api)
- [Testing and quality gates](#testing-and-quality-gates)
- [Building releases](#building-releases)
- [Troubleshooting](#troubleshooting)
- [Privacy guarantees by category](#privacy-guarantees-by-category)
- [Documentation map](#documentation-map)
- [Contributing](#contributing)

---

## What the system does

1. The **admin** creates an account (email + password or Google, in the web portal or the admin app) and an X25519 keypair. The private key stays in the OS keystore; the public key is registered on the server. The admin keeps an encrypted copy of the private key — a `.tkb` file and/or a server backup — both encrypted **on the client** with a Master Password (Argon2id + AES-256-GCM). The server never sees the Master Password or the private key in clear.
2. The admin creates a worker, chooses which data categories may be collected and **writes in plain language what each one means**. The server returns a single-use enrollment package.
3. The admin app **verifies the enrollment package** against its own key (manifest commitment, 12-digit safety code, QR payload) and only then shows the **QR** and the short code, which the admin gives to the employee in person.
4. The **employee** installs the agent and pastes the QR payload or types the code. The agent receives its credentials, the admin's public key and the consent request, **verifies them against the QR commitment** (or the safety code read aloud) so a compromised server cannot swap the key, and pins the key.
5. The employee sees exactly what the employer asks for and **chooses category by category** (all boxes start unchecked). They can change the selection or withdraw at any time; every decision is stored by the server as immutable evidence.
6. While `ACTIVE`, the agent collects only the accepted categories, encrypts each batch to the pinned key (ephemeral X25519 + HKDF-SHA256 + AES-256-GCM) and sends it; the server stores blobs it cannot read.
7. The admin app downloads the ciphertext, **decrypts it locally in parallel**, visualizes it and wipes it on auto-lock.
8. Billing happens only in the web portal (Flow). If the subscription is suspended or cancelled, the admin app shows a lock screen that opens the portal, and agents stop collecting (already-encrypted data stays queued; the employee's rights keep working).

Full design: [`.ai-context/domain.md`](.ai-context/domain.md) · enrollment protocol: `telemetry-api/ENROLLMENT.md` · contract v2 design: `telemetry-api/ARCHITECTURE-CHANGES.md`.

## Architecture

```mermaid
flowchart LR
    subgraph Admin["admin-client (Tauri 2)"]
        UI["React UI<br/>(no tokens, no keys)"] -- "IPC commands / events" --> RS[Rust core]
        RS --> KS[("OS keystore<br/>K_admin_priv, refresh token,<br/>installation id")]
    end
    subgraph Worker["worker-client"]
        WUI["React window (Tauri 2)<br/>closable at any time"] -- "local IPC (agent-ipc)<br/>token-authenticated" --> SVC["agent service (Rust)<br/>tray + attendance + telemetry"]
        SVC --> AK[("OS keystore & local state<br/>worker tokens, pinned key,<br/>PIN verifier, offline.db")]
    end
    API[("telemetry-api<br/>attendance + blind telemetry")]
    WEB["telemetry-web<br/>sign-up, onboarding, billing (Flow), downloads"]
    G["Google OAuth<br/>(system browser)"]
    RS -- "HTTPS: workers, attendance, vault,<br/>devices, subscription" --> API
    SVC -- "HTTPS: enroll, attendance marks,<br/>encrypted telemetry" --> API
    RS -. "loopback + PKCE" .-> G
    RS -. "opens portal_url" .-> WEB
    Admin -. "QR / code in person" .-> Worker
```

- **Admin app (`admin-client`):** the React UI only calls typed Tauri commands (`src/api/tauri-bridge.ts`); every HTTP request, token, key and Master Password lives in Rust (`admin-client/src-tauri`). Rust emits `session-expired` and `subscription-blocked` events to the UI.
- **Worker app (`worker-client`):** two processes (`PROC-01`). The background service (`worker-client/service/`, binary `telemetry-agent`, crate `agent`) runs in the background, displays a visible system tray icon, manages attendance queuing, transparent telemetry and API calls, and hosts a token-authenticated local IPC server (`worker-client/ipc/`, crate `agent-ipc`). The window (`worker-client/`, crate `agent-ui-core`, package `telemetry-worker-ui`) connects via IPC and can be closed at any time without stopping the service.
- **Crypto core (`telemetry-crypto-core`):** pure library crate shared across the workspace: telemetry envelopes, enrollment commitment and QR, key backups (`.tkb` v2 Argon2id), attendance mark canonical hash chain, legal document verification, PIN policy and Argon2id verifier, Chilean RUT/RUN.

Details: [`.ai-context/architecture.md`](.ai-context/architecture.md).

## Repository layout

| Path | Package | What it is |
|:-----|:--------|:-----------|
| `telemetry-crypto-core/` | `telemetry-crypto-core` | Envelope encryption (X25519, HKDF-SHA256, AES-256-GCM, zeroize), enrollment manifest/commitment/safety code/QR payload (CRYPTO-02), key backups: `.tkb` v2 with Argon2id (`backup_v2.rs`, CRYPTO-03) and legacy `.vault` v1 **import only** (`backup.rs`), attendance mark hash (SHA-256 chain), legal document hash/placeholders, PIN verifier (Argon2id), Chilean RUT/RUN (CRYPTO-04). |
| `worker-client/` | `telemetry-worker-ui` (npm) | Worker app UI and Tauri frontend (React 18, TypeScript, Tailwind). |
| `worker-client/src-tauri/` | `agent-ui-core` | Worker app window Rust backend (Tauri 2, IPC client to background service). |
| `worker-client/service/` | `agent` (bin `telemetry-agent`) | Worker background service: system tray, IPC server, attendance engine and local queue, transparent telemetry collectors. |
| `worker-client/ipc/` | `agent-ipc` | Worker app local IPC protocol (service ⇄ window): types, framing, transport and client. |
| `admin-client/src-tauri/` | `admin-client-core` | Admin app Rust backend: `client.rs` (API client: typed errors, single-flight refresh, events), `google_oauth.rs`, `device.rs`, `keystore.rs`, `commands/{session,keys,vault,devices,subscription,workers,telemetry,attendance,employer,legal}.rs`. |
| `admin-client/src/` | `telemetry-admin` (npm) | Admin UI: React 18, TypeScript, Tailwind, ECharts, Leaflet, `qrcode.react`. Pure helpers in `src/lib/` and components are unit-tested with `node --test`. |
| `docs/` | — | Functional specs ([admin](docs/admin-app-spec.md), [worker](docs/worker-app-spec.md)), legal framework ([marco legal Chile](docs/legal/marco-legal-cl.md)) and [plaintext payload schemas](docs/telemetry-payloads.md). |
| `.ai-context/` | — | Domain, architecture, constraints, security model, **API contract**, implementation status. **Start here.** |
| `tasks/` | — | Implementation tasks with requirements, validations, DoD and tests ([roadmap](tasks/README.md)). |
| `AGENTS.md` | — | Rules for AI assistants and contributors. |

## Features

### Admin app (`admin-client`)

| Area | What it does | Task |
|:---|:---|:---|
| Sign-in | Email + password or **Google** (system browser, loopback `127.0.0.1:<random>`, PKCE S256, `state` + `nonce`, 180 s timeout); every sign-in sends the device descriptor; no fake profiles. | ADMIN-02, ADMIN-10 |
| Session | Access token only in Rust memory (`Zeroizing`), refresh token in the keystore, **single-flight refresh**, `DEVICE_REVOKED` ⇒ immediate logout, auto-lock 5–30 min with RAM purge. | ADMIN-07, INT-02 |
| Start-up gates | Subscription → organization → key, before any data view; network failure ⇒ "Reintentar" (no state assumed). | ADMIN-12, ADMIN-09, ADMIN-11 |
| Subscription (TASK-APP-01) | Lock screen for `SUSPENDED`/`CANCELLED` with copy per reason, **Abrir en navegador** (server-provided portal URL only), support e-mail, "Ya pagué, verificar de nuevo", re-check every 60 s; trial banner ≤ 7 days. | ADMIN-12 |
| Key vault (TASK-APP-02) | First key (K0), recovery wizard (K1 key only on server, K2 mismatch): **A** `.tkb`/`.vault` file, **B** server backup + Master Password, **C** new key with double confirmation (`GENERAR NUEVA LLAVE`, checked in Rust); `.tkb` export; server backup setup/delete; no implicit key generation anywhere. | ADMIN-11 |
| Devices | List ("este equipo", last use, key fingerprint), rename, revoke (not the last active one; revoking this one logs out). | ADMIN-10 |
| Workers | Paginated list with filters, create with described consent (DP-08), **enrollment package v2 verified locally** before showing the QR, regenerate code (`REVOKE_FIRST` dialog), edit, pause/resume/revoke, delete, consent configuration, **granular evidence** (decision + accepted categories) with formula-safe CSV. | ADMIN-03, ADMIN-08, ADMIN-09 |
| Telemetry | Parallel decryption (`rayon`) tolerant to corrupted records; system, network, productivity/screenshots (blur mode), files and field map views (location locked for OFFICE). | ADMIN-04/05/06 |

### Worker app (`worker-client`)

| Area | What it does | Task |
|:---|:---|:---|
| Attendance | Legal Chilean attendance record (R38): start/end of workday and meal breaks with PIN confirmation (ATT-01); offline mark queue with device timestamp (ATT-04); receipt views; 48 h correction opposition (ATT-05); no telemetry dependency. | AGENT-12, AGENT-13 |
| Terms & PIN | Versioned worker terms and privacy policy acceptance (LEG-04/05); local Argon2id PIN verification in OS keystore (SEC-19); reset via emailed code. | AGENT-12, AGENT-13 |
| Enrollment | Paste the QR payload (v2 base64 or JSON) or type the code (the server is fixed at build time; a QR for another server is rejected); commitment verification or safety code; pinned key; tamper alert; errors by reason including `ENROLLMENT_LOCKED` countdown. | AGENT-08, AGENT-09 |
| Consent | One **unchecked** checkbox per category the admin requested, "Mantener mi selección anterior", "Cambiar mi selección", reject all, withdraw (purges buffer and queue first). | AGENT-09 |
| Collection | Only in `ACTIVE`, only accepted categories, time-bound categories only in work hours; keystroke/click **counts**, active window, idle, CPU/RAM, FQDN-only network, file metadata in designated folders, notified screenshots, FIELD-only location. | AGENT-02/03 |
| Delivery | One encrypted envelope per category, SQLite WAL queue (7 days, 100 MB, 5 MB/payload), backoff with jitter, single-flight refresh. | AGENT-04/05 |
| Billing pause | Employer suspended/cancelled ⇒ `EmployerSuspended`: collectors stop, queue kept, yellow light, notifications on enter/leave; rights and attendance keep working. | AGENT-09, BILL-04 |
| Transparency | Traffic light, schedule tab, rights tab (portability / erasure requests), diagnostics, notifications, system tray icon. | AGENT-06/07, AGENT-10 |

## Security and privacy guarantees

| Guarantee | How | Rule |
|:---|:---|:---|
| The server cannot read telemetry | Ephemeral X25519 + HKDF-SHA256 + AES-256-GCM per envelope, to the admin key pinned at enrollment | SEC-01, SEC-02 |
| The server never sees the admin private key or the Master Password | Argon2id and AES-GCM run only in the client; only the 60-byte blob + Argon2 parameters are uploaded | SEC-12 |
| A restored key is the right key | Public key derived after decrypting must match the registered fingerprint; keystore rolled back otherwise | SEC-13 |
| Refresh never self-revokes the session | Single-flight refresh in both apps | SEC-14 |
| Destroying the key requires intent | Checkbox + typed phrase, verified in Rust, then server rotation that revokes workers | SEC-15 |
| Google sign-in cannot be intercepted by another app | Loopback redirect on a random port, PKCE S256, `state`, `nonce` | SEC-16 |
| The UI cannot open arbitrary URLs | Only the portal URL received from the server, validated (https, no credentials, ≤ 2048) | SEC-17 |
| A compromised server cannot swap the admin key for a worker | QR commitment / safety code + key pinning; admin app re-verifies the package | SEC-07, SEC-08 |
| Secrets are wiped from memory | `zeroize` / `Zeroizing` for keys, KEKs, Master Passwords, tokens | SEC-02, SEC-11 |
| Nothing is collected without an explicit choice | Unchecked defaults, only requested categories, server-confirmed decisions | DP-06..DP-08, DP-11 |
| Billing never removes the worker's rights | Only ingestion stops; consent, withdrawal and data requests keep working | BILL-02, BILL-03 |

All rules with IDs: [`.ai-context/constraints.md`](.ai-context/constraints.md) · threat model: [`.ai-context/security-model.md`](.ai-context/security-model.md).

## Prerequisites

- **Rust stable** with clippy and rustfmt (`rustup`).
- **Node.js ≥ 22** + npm for both UIs (the unit tests import `.ts` modules directly with Node's type stripping; `npm run build` also works on Node 18/20).
- A running **`telemetry-api`** with PostgreSQL for live integration (see its README).
- **macOS:** `xcode-select --install`. The worker agent needs Accessibility (input counts, active window) and Screen Recording (screenshots) permissions, granted in System Settings → Privacy & Security.
- **Windows:** Visual Studio C++ Build Tools; WebView2 ships with Windows 10/11.
- **Linux (Debian/Ubuntu)** — packages verified to build the whole workspace (including Tauri and `xcap`):
  ```bash
  sudo apt install -y build-essential pkg-config libssl-dev \
      libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
      libxdo-dev libxtst-dev libx11-dev libxcb1-dev libxrandr-dev libxkbcommon-dev \
      libdbus-1-dev libegl1-mesa-dev libwayland-dev libgbm-dev libdrm-dev libpipewire-0.3-dev libclang-dev
  ```
- **Linux (Arch Linux)**:
  ```bash
  sudo pacman -S --needed base-devel pkgconf openssl \
      gtk3 webkit2gtk-4.1 libayatana-appindicator librsvg \
      xdotool dbus pipewire mesa libdrm wayland libxcb libxrandr clang
  ```
  At runtime the OS keystore needs a Secret Service provider (GNOME Keyring or KWallet) and the tray needs an AppIndicator-capable desktop.

## Configuration

```bash
cp .env.example .env
cp .env.example admin-client/.env
cp .env.example worker-client/service/.env
```

| Variable | Used by | Default | Notes |
|:---------|:--------|:--------|:------|
| `TELEMETRY_API_URL` | agent (build) | `http://localhost:8080/api/v1` in debug | Server the worker agent is **built for**: read at compile time from `worker-client/service/.env` or the environment and embedded in the binary. Required and `https://` in release builds. The worker never types it; a QR for another server is rejected. |
| `WORKER_FLUSH_INTERVAL` | agent | `30` | Seconds between flushes; values below 5 are ignored. |
| `TELEMETRY_API_URL` | admin (build) | `http://localhost:8080/api/v1` in debug | Same for the admin app: read at compile time from `admin-client/.env` or the environment (not exposed to the WebView). Required and `https://` in release builds; the sign-in screen has no server field. |
| `VITE_DEMO_MODE` | admin (UI) | unset | `true` enables the demo data store for UI work only; never set it in a release build (SEC-06). |
| `ADMIN_GOOGLE_CLIENT_ID` | admin (Rust) | empty | Google OAuth client of type **Desktop app**. Empty ⇒ the "Continuar con Google" button is hidden. Must also be listed in the API's `GOOGLE_CLIENT_ID`. |
| `ADMIN_GOOGLE_CLIENT_SECRET` | admin (Rust) | empty | Secret of that desktop client (Google does not treat it as confidential, but its token endpoint requires it). |
| `ADMIN_AUTO_LOCK_MINUTES` | — | `5` | Not read by the code: the auto-lock timeout (5–30 min) is chosen in the app (ADMIN-07). |

**Local state**

| App | Where | Contents |
|:---|:---|:---|
| Worker app | `<config_dir>/telemetry-worker/` (`~/Library/Application Support` · `%APPDATA%` · `~/.config`) | `config.json` (no secrets), `offline.db` (encrypted telemetry envelopes + pending marks journal) |
| Worker app | OS keystore | Worker access/refresh tokens, pinned admin public key and fingerprint, accepted manifest hash and selection, Argon2id PIN verifier |
| Worker app | `<runtime_dir>/telemetry-worker/` (`$TMPDIR` · `%LOCALAPPDATA%\Temp` · `$XDG_RUNTIME_DIR`) | `ipc.token` (`0600`, per-session IPC authentication token) |
| Admin | OS keystore, service `TelemetryAdminVault` | `K_admin_priv`, refresh token, installation id (`inst-<32 hex>`) |
| Admin | Downloads folder | `.tkb` exports (permissions `0600` on Unix; never overwritten — an existing name gets `-2`, `-3`, … before `.tkb`; Downloads falls back to Documents, then home) |

## Running in development

### Worker app (`worker-client`)

```bash
# Option A: Run the background service directly (tray + IPC server)
cargo run -p agent

# Option B: Full worker desktop app with Tauri (compiles service before dev server)
cd worker-client
npm install
npm run app:dev        # alias for npm run tauri dev
npm run dev            # UI only in a browser
```

In dev mode, `beforeDevCommand` runs `cargo build -p agent` from the workspace root and starts Vite. The Tauri window connects to the running service or starts `target/debug/telemetry-agent` automatically if absent (`PROC-04`).

### Admin app (`admin-client`)

```bash
cd admin-client
npm install
npm run app:dev        # alias for npm run tauri dev
npm run dev            # UI only in a browser (with VITE_DEMO_MODE=true for mock data)
```

Typical local flow: start `telemetry-api` → register in the admin app (trial starts) → create the first key and a backup → create a worker → copy the QR payload → launch worker-client → accept terms and set PIN → choose telemetry categories (optional) → mark attendance.

## Google sign-in setup (admin app)

1. In Google Cloud Console create an OAuth client of type **Desktop app** (loopback redirects to `127.0.0.1` are allowed automatically).
2. Put its id and secret in `ADMIN_GOOGLE_CLIENT_ID` / `ADMIN_GOOGLE_CLIENT_SECRET`.
3. Add the same client id to the API's `GOOGLE_CLIENT_ID` (comma-separated with the web portal's client id) and set `GOOGLE_AUTH_ENABLED=true`.
4. The app opens the system browser, receives the code on `http://127.0.0.1:<random port>/`, exchanges it with PKCE at `https://oauth2.googleapis.com/token`, checks `state` and the ID token `nonce`, and sends the ID token to `POST /auth/google`. Codes: `GOOGLE_NOT_CONFIGURED`, `GOOGLE_SIGN_IN_CANCELLED`, `GOOGLE_SIGN_IN_TIMEOUT`, `GOOGLE_STATE_MISMATCH`, `GOOGLE_NONCE_MISMATCH`, `GOOGLE_TOKEN_EXCHANGE_FAILED`, `BROWSER_UNAVAILABLE`.

## Key backup format (`.tkb`)

Shared with `telemetry-web` (onboarding) and the API vault; specified in [`tasks/crypto-core/task-03-key-backup-v2-argon2id.md`](tasks/crypto-core/task-03-key-backup-v2-argon2id.md), which also contains the **reference vector** both implementations must reproduce.

```jsonc
{
  "format": "telemetry-key-backup", "version": 2,
  "kdf": { "algorithm": "argon2id", "version": 19, "memory_cost": 65536, "time_cost": 3, "parallelism": 4, "salt": "<b64 16 B>" },
  "encrypted_private_key": "<b64(nonce 12 ‖ tag 16 ‖ ciphertext 32)>",   // AES-256-GCM, AAD "telemetry-key-backup-v2"
  "public_key": "<b64 X25519>", "public_key_fingerprint": "<64 hex>",
  "organization": "Acme SpA", "created_at": "2026-09-30T12:00:00Z"
}
```

- Master Password: Unicode NFC, **not trimmed**, ≥ 12 characters, ≤ 1024 bytes. Accepted KDF ranges: `memory_cost` 32768–1048576 KiB, `time_cost` 2–20, `parallelism` 1–16, salt 16–64 bytes. Unknown fields are rejected; files are limited to 64 KiB.
- File name: `telemetry_key_{organization-slug}_{YYYY-MM-DD}.tkb`.
- Legacy `.vault` (v1, PBKDF2) can still be **imported**, never exported.

## How the apps use the API

Exact shapes, error mapping and events: [`.ai-context/api-contract.md`](.ai-context/api-contract.md) (contract v3). Summary:

| App | Endpoints |
|:---|:---|
| Admin | `POST /auth/{register,login,google,refresh,logout}` · `GET/PUT /admins/me` · `PUT /admins/me/public-key` · `POST /admins/me/public-key/rotate` · `GET/DELETE /admins/me/vault` · `POST /admins/me/vault/setup` · `GET /admins/me/vault/backup` · `GET /admins/me/devices` · `PATCH/DELETE /admins/me/devices/:id` · `GET /subscriptions/me` · `/workers` CRUD · `POST /workers/:id/enrollment-token` · `PUT /workers/:id/consent-config` · `PATCH /workers/:id/status` · `GET /workers/:id/consent-records` · `GET /telemetry/{category}` · `GET /attendance/days` · `GET/POST /attendance/corrections` · `GET /reports/*` |
| Agent | `GET /health` · `POST /workers/enroll` · `POST /auth/refresh` · `GET /workers/me` · `POST /workers/me/terms-acceptance` · `POST /workers/me/pin` · `POST /attendance/marks` · `GET /attendance/me/today` · `GET /attendance/me/days` · `GET /attendance/me/marks` · `GET/POST /attendance/me/corrections` · `PATCH /workers/me/consent` · `POST /telemetry/{category}` · `POST /workers/me/data-request` · `POST /workers/me/data-deletion` |

Rules both apps follow: branch on `error.code` + `error.details.reason` (never on `message`); one refresh in flight; `401 DEVICE_REVOKED` (admin) or a failed refresh (agent) ends the session; `402 SUBSCRIPTION_SUSPENDED` / `403 SUBSCRIPTION_CANCELLED` switch to the lock screen (admin) or to `EmployerSuspended` (agent); `Retry-After` is honoured for `429`.

## Testing and quality gates

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd admin-client && npm ci && npm run build && npm test
cd worker-client && npm ci && npm run build && npm test
```

| Suite | Highlights |
|:---|:---|
| `telemetry-crypto-core` | Envelope encryption, enrollment commitment vector, QR payload parsing, `.tkb` reference vector, attendance mark hash chain, legal document hash, PIN verifier, Chilean RUT/RUN (51 tests) |
| `agent` (service) | Lifecycle contract v3, attendance mark journal & offline queue, hash mismatch detection, 48h corrections, PIN retry backoff, transparent collectors, SQLite offline queue (155 tests) |
| `agent-ipc` | IPC protocol types, length-prefixed framing, Unix domain socket / named pipe transport, token handshake, snapshot subscriptions (21 tests) |
| `agent-ui-core` | Worker window IPC client commands, service binary auto-launch, event forwarding (9 tests) |
| `admin-client-core` | Typed errors and safe details, single-flight refresh, subscription lock, Google OAuth PKCE, enrollment package verification, key vault `.tkb`, R38 attendance reports (154 tests) |
| Admin UI (`npm test`) | Attendance calculations, R38 formats, form validations, lock screen, portal URL checks, Master Password, CSV evidence (91 tests) |
| Worker UI (`npm test`) | Attendance views, PIN modal validation, banner priority, legal text scrolling, offline sync (26 tests) |

HTTP code is tested against `wiremock` servers that reproduce the API contract; tests never touch the real config directory or OS keystore (`tempfile`, in-memory test keystore).

### Live contract tests

Opt-in contract verification tests against a real running `telemetry-api` (v3 API with empty or test database):

```bash
# Admin client suite:
TELEMETRY_LIVE_API_URL=http://127.0.0.1:18080/api/v1 cargo test -p admin-client-core --features live-api live_api

# Worker service suite:
TELEMETRY_LIVE_API_URL=http://127.0.0.1:18080/api/v1 cargo test -p agent --features live-api live_api
```

These require `TELEMETRY_LIVE_API_URL` pointing to an active API v3 instance. When compiled without `--features live-api`, these tests are excluded from the binary, leaving `cargo test --workspace` unchanged.

## Building releases

### Worker app (`worker-client`)

```bash
cd worker-client
npm install
npm run app:build       # alias for npm run tauri build
```

This executes `beforeBuildCommand` (`npm run sidecar && npm run build`), which builds the `telemetry-agent` service sidecar in release mode into `src-tauri/binaries/`, compiles the frontend, and packages both into `target/release/bundle/` (e.g. `target/release/bundle/dmg/` on macOS, `target/release/bundle/deb/` on Linux, `target/release/bundle/msi/` on Windows).

### Admin app (`admin-client`)

```bash
cd admin-client
npm install
npm run app:build       # alias for npm run tauri build
```

Compiles the admin app and produces the installer under `target/release/bundle/`.

> **Note on release builds:** Release builds of the service and the admin app require `TELEMETRY_API_URL` to be configured (`https://`) and fail if legal documents in `docs/legal/` still contain unpopulated provider placeholders (`LEG-06`). For testing the bundle pipeline in development, use `npx tauri build --no-bundle` or `--debug`.

### Automated release pipeline (GitHub Actions)

Multiplatform releases are automated via `.github/workflows/release.yml` (triggered on `v*` tag pushes or manual `workflow_dispatch`). The workflow builds both desktop apps across macOS Apple Silicon (`aarch64-apple-darwin`), macOS Intel (`x86_64-apple-darwin`), Windows x64 (`x86_64-pc-windows-msvc`), and Linux x64 (`x86_64-unknown-linux-gnu`), computes SHA-256 checksums (`SHA256SUMS.txt`), and uploads all installers to a draft GitHub Release.

Continuous integration is handled by `.github/workflows/ci.yml` on every pull request and push to `main` and `develop-AGENTS`, verifying all five quality gates of `AGENTS.md` §9.

#### Release workflow variables and secrets

| Name | Type | Required? | Purpose |
|:-----|:-----|:---------:|:--------|
| `TELEMETRY_API_URL` | Var or Secret | **Yes** | Server API base URL embedded at compile time (must start with `https://`, SEC-10). |
| `APPLE_CERTIFICATE` | Secret | Optional | Base64-encoded Developer ID Application `.p12` certificate for macOS code signing. |
| `APPLE_CERTIFICATE_PASSWORD` | Secret | Optional | Password for the macOS `.p12` signing certificate. |
| `APPLE_SIGNING_IDENTITY` | Secret | Optional | Code signing identity name (e.g. `Developer ID Application: ...`). |
| `APPLE_ID` | Secret | Optional | Apple ID email address used for macOS app notarization (`notarytool`). |
| `APPLE_PASSWORD` | Secret | Optional | App-specific password for the Apple ID notarization account. |
| `APPLE_TEAM_ID` | Secret | Optional | 10-character Apple Developer Team ID for notarization. |
| `WINDOWS_CERTIFICATE` | Secret | Optional | Base64-encoded code signing certificate (`.pfx` / `.p12`) for Windows installer signing. |
| `WINDOWS_CERTIFICATE_PASSWORD` | Secret | Optional | Password for the Windows `.pfx` code signing certificate. |
| `TAURI_SIGNING_PRIVATE_KEY` | Secret | Optional | Private key for signing Tauri automatic updates. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Secret | Optional | Password for the Tauri update signing private key. |
| `ADMIN_GOOGLE_CLIENT_ID` | Var or Secret | Optional | Google OAuth Desktop client ID for Google sign-in in the admin desktop client. |
| `ADMIN_GOOGLE_CLIENT_SECRET` | Var or Secret | Optional | Google OAuth Desktop client secret for Google sign-in in the admin desktop client. |

> **Note on code signing:** When signing secrets are not configured or are left empty, the release workflow continues and produces unsigned installers (`.dmg`, `.app.tar.gz`, `.deb`, `.AppImage`, `.msi`, `-setup.exe`) alongside `SHA256SUMS.txt`.

The release profile uses LTO, `strip`, `panic = "abort"` and `opt-level = 3`; the dev profile compiles `argon2`/`blake2` with `opt-level = 3` so key derivation stays fast in debug builds. Code signing and notarization are executed when the secrets above are configured in GitHub. `telemetry-web` serves the installers from GitHub Releases. Before a production release, fix the blockers listed at the top of this file.

## Troubleshooting

| Symptom | Cause / fix |
|:---|:---|
| Admin shows "Suscripción suspendida" / "Período de prueba terminado" | `402`/`403` from the API: pay in the portal ("Abrir en navegador"), then "Ya pagué, verificar de nuevo". |
| Admin logs out with "Este equipo fue revocado" | The device was revoked from another session (`DEVICE_REVOKED`); sign in again (a new device entry is created). |
| "Continuar con Google" is missing | `ADMIN_GOOGLE_CLIENT_ID` is empty in the build environment. |
| Google sign-in fails with `GOOGLE_AUTH_DISABLED` / `INVALID_GOOGLE_TOKEN` | Enable Google on the API and add the desktop client id to `GOOGLE_CLIENT_ID`. |
| Recovery wizard says the key does not match | The file/backup belongs to another key (`BACKUP_KEY_MISMATCH`); use the backup made for the registered key, or generate a new key (destroys access to old data). |
| Creating a worker asks for the organization | `ORGANIZATION_NAME_MISSING`: the organization is shown to employees and is required. |
| Agent: "Demasiados intentos fallidos" | `ENROLLMENT_LOCKED`: wait for the countdown (`Retry-After`), then enter a valid code. |
| Agent: "Este código ya fue utilizado" | `ENROLLMENT_ALREADY_COMPLETED`: ask the admin for a new code (to move a device the admin revokes the old one first). |
| Agent light is yellow with "la cuenta de tu empleador no está activa" | Employer subscription blocked; collection resumes automatically after payment. |
| Linux: keystore errors | No Secret Service running (start GNOME Keyring / KWallet); in headless CI use the in-memory test vault. |
| Linux build fails on WebKit/GTK/dbus | Install the packages listed in [Prerequisites](#prerequisites). |

## Privacy guarantees by category

| Category | Collected | Never collected |
|:---------|:----------|:----------------|
| System | Foreground app & title, idle time, CPU/RAM, USB events | Screen content |
| Network | Domain names (FQDN) + counts | URLs, paths, queries, page content |
| Productivity | Keystrokes and clicks **per minute**; screenshots only if requested, accepted and each one notified | Which keys were pressed, any text |
| Files | Create/modify/delete/rename events in designated folders (relative paths, extensions) | File contents, other folders, absolute paths |
| Location | GPS, `FIELD` role only, work hours only | Anything for `OFFICE` workers or off-hours |

Nothing is collected before the employee accepts, outside the categories the employee accepted, while the admin has the worker paused or the employer's subscription is suspended, or after withdrawal or revocation. Legal context: GDPR, Chilean Law 19.628 / 21.719. Whether consent is a valid legal basis in an employment relationship must be validated by counsel.

## Documentation map

| Document | What it covers |
|:---|:---|
| [`.ai-context/README.md`](.ai-context/README.md) | Reading order and naming |
| [`.ai-context/domain.md`](.ai-context/domain.md) | Actors, lifecycle, consent model, categories, glossary |
| [`.ai-context/architecture.md`](.ai-context/architecture.md) | Crates, processes, threads, data flow |
| [`.ai-context/constraints.md`](.ai-context/constraints.md) | Hard rules with IDs (DP, SEC, PERF, BILL, …) |
| [`.ai-context/security-model.md`](.ai-context/security-model.md) | Keys, tokens, enrollment, vault, threat model |
| [`.ai-context/api-contract.md`](.ai-context/api-contract.md) | HTTP contract v3 as the apps use it |
| [`.ai-context/implementation-status.md`](.ai-context/implementation-status.md) | What is real, partial or missing, with known gaps |
| [`docs/admin-app-spec.md`](docs/admin-app-spec.md) · [`docs/worker-app-spec.md`](docs/worker-app-spec.md) | Screens, flows, commands, acceptance scenarios |
| [`docs/telemetry-payloads.md`](docs/telemetry-payloads.md) | Plaintext payload schemas (agent ⇄ admin) |
| [`tasks/README.md`](tasks/README.md) | Roadmap, dependency graph, Definition of Done |
| `telemetry-api/docs/api-specification.md`, Swagger `/swagger-ui` | Server contract (source of truth) |
| `telemetry-api/telemetry-apps.md` | Server-side mirror of these apps' spec and endpoint mapping |

## Contributing

Branches: never commit to `main`, `dev`, `develop` or `develop-AGENTS`; implementers work only on `dev/<task-topic>` branches. Conventional commits. Read [`AGENTS.md`](AGENTS.md), pick a task from [`tasks/README.md`](tasks/README.md), and follow the Definition of Done.
