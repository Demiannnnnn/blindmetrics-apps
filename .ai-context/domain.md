# Domain Model

## 1. What the product is

An **employee-device telemetry system with a blind server**. An organization's admin decides which categories of activity data are collected from each employee's computer; the employee sees exactly that, in plain language, and accepts or rejects it; the employee's machine encrypts everything to the admin's public key before it leaves the device; the server stores ciphertext it cannot read; only the admin's desktop app can decrypt.

It is **not** spyware: collection is visible (status indicator, notifications), consented, minimized (no keystroke content, no URLs, no file contents), time-bounded (GPS only in work hours) and revocable.

## 2. Actors

| Actor | Software | Holds | Can | Cannot |
|:------|:---------|:------|:----|:-------|
| **Admin** | `admin-client` (Tauri) | `K_admin_priv` (OS keystore), admin JWT | Register, register `K_admin_pub`, create workers, set requested consent + descriptions, issue enrollment codes, pause/resume/revoke workers, fetch & decrypt telemetry. | Force consent. See data of other admins' workers. Recover data if `K_admin_priv` is lost and there is no backup. |
| **Worker** (employee) | `agent` (egui) | Pinned `K_admin_pub`, worker JWT pair | Enroll with a code, read the consent manifest, accept / reject / withdraw, see status, request data export / deletion. | Choose categories à la carte (accept-all-or-nothing). Read the encrypted data (no private key). |
| **Server** | `telemetry-api` (Axum) | Ciphertext, metadata, `K_admin_pub` (public), hashed enrollment codes | Authenticate, authorize, enforce consent/status at ingestion, store blobs, keep audit + consent evidence. | Decrypt anything. Silently change the key or consent the worker saw (see `security-model.md`). |

## 3. Ubiquitous language

| Term | Meaning |
|:-----|:--------|
| **Admin keypair** | X25519 pair. `K_admin_priv` never leaves the admin machine; `K_admin_pub` is registered on the server once and pinned by every agent at enrollment. |
| **Enrollment code** | Single-use, 24 h, 12-char Crockford Base32 code (`K7QM-4XRT-9PWD`) issued by the server when the admin creates a worker. Stored hashed server-side. |
| **Enrollment QR** | `{ t, v, api_url, code, commitment }` rendered by the admin app. Carries the code *and* the commitment over a channel the server does not control. |
| **Consent manifest** | `{ admin_public_key, consent_version, consent_flags, consent_descriptions }` — the exact package the worker is asked to accept. |
| **Commitment** | `SHA-256("telemetry-enroll-v1\0" ‖ canonical(manifest))`, computed locally by the admin app, verified locally by the agent. |
| **Safety code** | 12 decimal digits derived from the commitment, shown in both apps for manual comparison when the QR is not used. |
| **Pinned key** | The `K_admin_pub` the agent stored at enrollment. The agent encrypts **only** to this key and never replaces it from server responses. |
| **Requested flags** | What the admin asks to collect (`requested_consent_flags`). |
| **Effective flags** | What may actually be collected (`consent_flags`). Always `effective ⊆ requested`, and non-empty only after the worker accepted. |
| **Consent version** | Integer bumped by the server whenever the admin changes requested flags or descriptions. |
| **Envelope** | `{ ephemeral_public_key, encrypted_payload }` produced by `telemetry-crypto-core`. `encrypted_payload = nonce(12) ‖ tag(16) ‖ ciphertext`. |
| **Record** | One envelope + `client_timestamp` as stored by the API (one row). One record = one category. |
| **Flush** | Periodic drain of the in-memory buffer → group by category → encrypt → send (or queue). |

## 4. Worker lifecycle

Server-authoritative. The agent mirrors it via `GET /workers/me`.

```mermaid
stateDiagram-v2
    [*] --> PENDING_ENROLLMENT: admin creates worker (code issued)
    PENDING_ENROLLMENT --> PENDING_CONSENT: agent redeems code
    PENDING_CONSENT --> ACTIVE: worker ACCEPT
    PENDING_CONSENT --> PENDING_CONSENT: worker REJECT
    ACTIVE --> PENDING_CONSENT: worker WITHDRAW
    ACTIVE --> PAUSED: admin pause
    PAUSED --> ACTIVE: admin resume
    PAUSED --> PENDING_CONSENT: worker WITHDRAW
    PENDING_ENROLLMENT --> REVOKED: admin revoke/delete
    PENDING_CONSENT --> REVOKED: admin revoke/delete
    ACTIVE --> REVOKED: admin revoke/delete
    PAUSED --> REVOKED: admin revoke/delete
    REVOKED --> PENDING_ENROLLMENT: admin issues new code
```

| Status | Agent behaviour | Admin sees |
|:-------|:----------------|:-----------|
| `PENDING_ENROLLMENT` | Not bound (shows enrollment screen). | "Invited — code expires at …" / "Code expired" + *Regenerate*. |
| `PENDING_CONSENT` | Bound but **inert**: consent screen, zero collectors, no sends. | "Waiting for employee consent" (+ rejected-at if any). |
| `ACTIVE` | Collectors for effective flags only. | Live; device info; accepted version & date. |
| `PAUSED` | Collectors stopped; queue kept; banner "Paused by your employer". | Paused + *Resume*. |
| `REVOKED` | Next refresh → 401 → wipe credentials, pinned key, queue → enrollment screen. | Revoked + *Re-enroll* (new code). |

## 5. Consent model

- **Admin defines, worker decides.** The admin chooses categories and writes a plain-language description per enabled category. The worker accepts or rejects the **whole package**. There are no per-category toggles for the worker.
- **Withdrawal is always one click** (legal requirement) and stops all collection immediately.
- **Versioning:**
  - Admin *narrows* (removes categories) → applied immediately, no re-acceptance, agent shows an info notice.
  - Admin *widens* or rewrites descriptions → new version; agent shows the consent screen again; until accepted, the old effective flags keep applying.
- **Evidence:** every decision is an append-only `consent_records` row server-side including the `manifest_hash` the agent computed.
- **The agent is the last line of defence:** it only starts a collector if the category is in the effective flags **and** it was part of a manifest the worker accepted on this device.

## 6. Telemetry categories

| API flag | API path | Agent collector | Collected | Never collected |
|:---------|:---------|:----------------|:----------|:----------------|
| `system_activity` | `/telemetry/system` | `SystemCollector` | Foreground app name & window title, idle seconds, CPU %, RAM, net bytes, USB connect/disconnect (VID/PID/name). | Screen content, process arguments. |
| `network_activity` | `/telemetry/network` | `NetworkCollector` | FQDN + hit count (+ optional local category). | Full URL, path, query, headers, bodies. |
| `productivity_basic` | `/telemetry/productivity` (`subcategory: BASIC`) | `KeystrokeCollector` | Keystrokes/min, clicks/min. | Which keys, any text. |
| `productivity_screenshots` | `/telemetry/productivity` (`subcategory: SCREENSHOT`) | `ScreenshotCollector` | Periodic JPEG/WebP ≤1920×1080, each one notified. | Anything outside work hours; screenshots while paused. |
| `file_activity` | `/telemetry/files` | `FileCollector` | Created/Modified/Deleted/Renamed, relative path, extension — only inside admin-designated folders. | File contents; anything outside designated folders. |
| `location` | `/telemetry/location` | `LocationCollector` | Lat, lon, accuracy — only `FIELD` role, only inside work hours. | Anything for `OFFICE` workers; anything off-hours. |

Plaintext schemas: [`docs/telemetry-payloads.md`](../docs/telemetry-payloads.md).

## 7. Worker roles

| Role | Location | Notes |
|:-----|:---------|:------|
| `OFFICE` | **Forbidden** — rejected by admin UI, API and DB constraints; agent never starts the collector. | Default. |
| `FIELD` | Allowed with consent, only within `work_hours_start..work_hours_end` in the worker's IANA `timezone`. | Changing a worker to `OFFICE` removes `location` (a narrowing). |
