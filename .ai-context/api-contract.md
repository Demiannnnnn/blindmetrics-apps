# API Contract (client side) — v2

What the apps must send and expect from `telemetry-api` after its TASK-13..17 (commit `8e2171a`,
`telemetry-api/docs/api-specification.md`). Everything below is implemented server-side; there are
no longer endpoints "pending" on the server. The web portal (`telemetry-web`) uses the same API
through its own backend.

Base URL: `…/api/v1` (no trailing slash). Every request sends `X-App-Version: <crate version>`;
the agent also sends `User-Agent: telemetry-agent/<version>` (stored as consent evidence).

## 1. Envelopes and errors

```jsonc
{ "data": { … } }                                                       // single
{ "data": [ … ], "pagination": { "total", "page", "per_page", "total_pages" } }   // collection
{ "error": { "code": "CONFLICT", "message": "…", "details": { "reason": "…", … } | null } }
```

`error.code` (stable set): `VALIDATION_ERROR` 400 · `UNAUTHORIZED` 401 · `SUBSCRIPTION_SUSPENDED`
402 · `FORBIDDEN` / `SUBSCRIPTION_CANCELLED` 403 · `RESOURCE_NOT_FOUND` 404 · `CONFLICT` 409 ·
`PAYLOAD_TOO_LARGE` 413 · `UNSUPPORTED_MEDIA_TYPE` 415 · `RATE_LIMITED` 429 · `INTERNAL_ERROR` 500 ·
`BAD_GATEWAY` 502 · `SERVICE_UNAVAILABLE` 503. The domain cause is `error.details.reason`.
Malformed JSON bodies also get this envelope (`400`), never Axum's plain text.

`429` carries `Retry-After: <seconds>` when the wait is known (also `details.retry_after_seconds`).

Client rules:
- Read `body.data.*`, never `body.*`.
- Map errors by `code` + `details.reason`, never by message text (messages may change language).
- Only whitelisted `details` fields reach the admin UI (INT-02): `portal_url`, `support_email`,
  `subscription_status`, `retry_after_seconds`, `hint`, `status`, `current_consent_version`, `categories`.

## 2. Sessions, refresh and devices

`AuthResponse` (register, login, Google, refresh, inside enroll):

```json
{ "access_token": "eyJ…", "refresh_token": "hex…", "token_type": "Bearer", "expires_in": 900,
  "user": { "id": "uuid", "email": "…", "name": "…", "role": "ADMIN", "is_new_user": false } }
```

| Call | Body | Notes |
|:---|:---|:---|
| `POST /auth/register` → 201 | `{ email, password, name, organization, device? }` | Password 8–128 with upper, lower, digit, symbol. Starts `TRIAL` (14 days). |
| `POST /auth/login` → 200 | `{ email, password, role: "ADMIN", device? }` | `role: WORKER` → `400 WORKER_LOGIN_REMOVED`. Google-only accounts get the same `401` as a wrong password. |
| `POST /auth/google` → 200 | `{ id_token, role: "ADMIN", device? }` | Server verifies the ID token (JWKS, `aud`, `iss`, `exp`, `email_verified`). Errors: `403 GOOGLE_AUTH_DISABLED`, `401 INVALID_GOOGLE_TOKEN`, `409 GOOGLE_ACCOUNT_MISMATCH`, `503 GOOGLE_UNAVAILABLE`. New account ⇒ `is_new_user = true`, empty `organization`. |
| `POST /auth/refresh` → 200 | `{ refresh_token }` | **Single use.** Persist the new refresh token before using the new access token. |
| `POST /auth/logout` → 204 | `{ refresh_token }` | Revokes that token only. |

`device` (admin desktop app only; the web omits it):
```json
{ "fingerprint": "inst-3f9a…", "name": "MacBook de Ana", "public_key_fingerprint": "64 hex | null" }
```
`fingerprint` 16–512 chars `[A-Za-z0-9._:-]`, stable per installation; `name` 1–255. The session is
bound to the device (`did` claim).

**Refresh must be single-flight.** Two concurrent refreshes with the same token look like a replay:
the loser gets `401 TOKEN_REUSE_DETECTED` and the whole token family is revoked. Serialize refreshes
and, after waiting, reuse a token another task already obtained.

| Refresh / guard answer | Meaning | Client action |
|:---|:---|:---|
| `401 TOKEN_REUSE_DETECTED` | Family revoked | Admin: session expired. Agent: treated as revoked (wipe). |
| `401 DEVICE_REVOKED` (guard or refresh) | Admin device revoked from another session | Clear session, **do not refresh**, show reason. |
| `401` other | Expired/unknown token, inactive account, revoked worker | Admin: session expired. Agent: revoked. |

## 3. Admin account

`GET /admins/me` (reachable even while the subscription is blocked):

```jsonc
{ "id", "email", "name", "organization", "is_active", "worker_count",
  "public_key": "b64 | null", "public_key_fingerprint": "64 hex | null",
  "has_password": true, "google_linked": false, "key_backup_enabled": true,
  "onboarding_completed": true,          // organization ≠ '' ∧ public key registered
  "created_at", "updated_at" }
```

| Call | Notes |
|:---|:---|
| `PUT /admins/me` | `{ name?, organization? }` (2–255 trimmed). |
| `PUT /admins/me/public-key` | `{ public_key }` → `{ public_key, public_key_fingerprint, public_key_registered_at }`. Write-once: same key ⇒ 200; other ⇒ `409 PUBLIC_KEY_ALREADY_REGISTERED`. |
| `POST /admins/me/public-key/rotate` | `{ public_key, previous_public_key_fingerprint, confirm_data_loss: true }` → `{ …, revoked_workers, key_backup_enabled: false }`. **Destructive:** revokes every worker, deletes the vault backup. Errors: `400 CONFIRMATION_REQUIRED`, `409 ADMIN_PUBLIC_KEY_MISSING`, `409 PUBLIC_KEY_MISMATCH`, `409 PUBLIC_KEY_UNCHANGED`. |

### 3.1 Key vault (zero-knowledge backup)

Format v2 (TASK-CRYPTO-03): Argon2id v19 (default m 65536 KiB, t 3, p 4, 16-byte salt) → KEK;
AES-256-GCM with AAD `"telemetry-key-backup-v2"`; plaintext = raw 32-byte `K_admin_priv`;
blob = `b64(nonce 12 ‖ tag 16 ‖ ct 32)` (60 bytes).

| Call | Notes |
|:---|:---|
| `POST /admins/me/vault/setup` | `{ encrypted_private_key, public_key_fingerprint, argon2_params: { memory_cost, time_cost, parallelism, salt, algorithm?, version? } }`. Unknown fields rejected. Ranges: m 32768–1048576, t 2–20, p 1–16, salt 16–64 B. Errors `400 INVALID_VAULT_BLOB`, `400 INSECURE_ARGON2_PARAMS`, `409 ADMIN_PUBLIC_KEY_MISSING`, `409 PUBLIC_KEY_MISMATCH`. → `{ key_backup_enabled, public_key_fingerprint, updated_at, replaced }`. |
| `GET /admins/me/vault/backup` | → `{ encrypted_private_key, argon2_params (normalized), public_key_fingerprint, updated_at }`. `404 VAULT_NOT_CONFIGURED`. Each download is audited. |
| `GET /admins/me/vault` | → `{ key_backup_enabled, public_key_fingerprint, updated_at, matches_registered_key }` (never the blob). |
| `DELETE /admins/me/vault` | 204, idempotent. |

### 3.2 Admin devices (reachable while the subscription is blocked)

| Call | Notes |
|:---|:---|
| `GET /admins/me/devices` | `[{ id, device_name, device_fingerprint, public_key_fingerprint, is_active, is_current, last_seen_at, created_at, revoked_at }]`, active first. |
| `PATCH /admins/me/devices/:id` | `{ device_name?, public_key_fingerprint? }` (≥ 1). `409 DEVICE_REVOKED`, `409 PUBLIC_KEY_MISMATCH`. |
| `DELETE /admins/me/devices/:id` | 204. Revokes its refresh tokens; its access tokens get `401 DEVICE_REVOKED` immediately. `409 LAST_ACTIVE_DEVICE`. |

## 4. Subscription

Effective status is computed on every request: `TRIAL` expired ⇒ `SUSPENDED (TRIAL_EXPIRED)`;
`ACTIVE` expired + 3 grace days ⇒ `SUSPENDED (SUBSCRIPTION_EXPIRED)`.

| Admin HTTP | When |
|:---|:---|
| `402 SUBSCRIPTION_SUSPENDED` (`details.reason` = `TRIAL_EXPIRED` / `SUBSCRIPTION_EXPIRED` / `SUBSCRIPTION_SUSPENDED`) | Suspended |
| `403 SUBSCRIPTION_CANCELLED` | Cancelled |
| `X-Trial-Days-Left: n` header | Every successful admin response during `TRIAL` |

`details`: `{ reason, subscription_status, portal_url, support_email, suspended_at | cancelled_at }`.
Exempt: `GET /admins/me`, `/admins/me/devices*`, `/auth/*`, `/subscriptions/*`.

| Call | Notes |
|:---|:---|
| `GET /subscriptions/me` | `{ status, effective_status, reason, expires_at, grace_period_ends_at, trial_days_left, status_changed_at, portal_url, support_email, billing_configured }`. Never blocked. |
| `GET /subscriptions/plans` | Public. Plans, `trial_days`, `billing_configured`. |
| `POST /subscriptions/create-checkout`, `GET /subscriptions/me/payments` | **Web portal only.** The desktop app never starts payments; it opens `portal_url`. |

Worker side: only `POST /telemetry/*` is blocked (`402`/`403` with the same codes);
`GET /workers/me` shows `ingestion_blocked_reason`.

## 5. Workers (admin side)

### Create — `POST /workers` → 201
```json
{ "email": "jperez@acme.cl", "name": "Juan Pérez", "role_type": "OFFICE",
  "work_hours_start": "09:00", "work_hours_end": "18:00:00", "timezone": "America/Santiago",
  "consent_flags": { "system_activity": true, "network_activity": true },
  "consent_descriptions": { "system_activity": "…10–500 chars…", "network_activity": "…" },
  "enrollment_ttl_hours": 24 }
```
→ `{ worker: WorkerResponse, enrollment: { code, expires_at, commitment, safety_code, qr_payload } }`.

- `code`: 12 symbols of `23456789ABCDEFGHJKMNPQRSTVWXYZ`, `XXXX-XXXX-XXXX`, shown once.
- `commitment` / `safety_code`: computed **by the server** — the admin app recomputes them with its
  **local** key and refuses to show the QR on mismatch (ADMIN-09).
- `qr_payload` = `base64(JSON {"t":"telemetry-enroll","v":1,"api_url","code","commitment"})`.
- Errors: `409 ADMIN_PUBLIC_KEY_MISSING`, `409 ORGANIZATION_NAME_MISSING`, `409` duplicate email
  (per admin), `400` (no category, missing description, `location` with `OFFICE`, TTL 1–72).

### Regenerate — `POST /workers/:id/enrollment-token` → 201
Body `{ enrollment_ttl_hours? }` → `{ enrollment: { … same shape … } }`.
`409 ENROLLMENT_ALREADY_COMPLETED` with `details.hint = "REVOKE_FIRST"` when the worker already
accepted consent on a device (revoke first, then regenerate). `REVOKED` workers can always be re-linked.

### Other admin calls

| Call | Notes |
|:---|:---|
| `GET /workers?page&per_page&search&role_type&is_active&status` | `CollectionResponse<WorkerResponse>`. |
| `GET /workers/:id` | `WorkerResponse`. |
| `PUT /workers/:id` | `{ name?, role_type?, work_hours_start?, work_hours_end?, timezone? }`. Switching to `OFFICE` narrows `location`. |
| `PUT /workers/:id/consent-config` | `{ consent_flags, consent_descriptions }` → `{ consent_version, requires_worker_acceptance, worker }`. |
| `PATCH /workers/:id/status` | `{ status: PAUSED \| ACTIVE \| REVOKED, reason? }`. |
| `GET /workers/:id/consent-records` | Evidence, newest first (below). |
| `DELETE /workers/:id` | 204, soft delete (`REVOKED`). |

`WorkerResponse`:
```jsonc
{ "id", "admin_id", "email", "name",
  "device_identifier": "64hex" | null, "device_info": { "hostname", "os", "app_version" } | null,
  "role_type": "OFFICE" | "FIELD", "is_active": true,
  "status": "PENDING_ENROLLMENT" | "PENDING_CONSENT" | "ACTIVE" | "PAUSED" | "REVOKED", "status_changed_at",
  "consent_flags": { …6 bools… },            // effective = what the worker accepted
  "requested_consent_flags": { …6 bools… },  // what the admin asks for
  "consent_descriptions": { …6 × string|null… },
  "consent_version": 2, "accepted_consent_version": 1 | null, "consent_accepted_at": "…" | null,
  "enrollment_completed_at": "…" | null,     // first acceptance
  "enrollment_redeemed_at": "…" | null,      // code redeemed on a device
  "enrollment_expires_at": "…" | null,       // only while PENDING_ENROLLMENT
  "enrollment_expired": false,
  "work_hours_start": "09:00:00" | null, "work_hours_end": "18:00:00" | null, "timezone": "America/Santiago",
  "created_at", "updated_at" }
```

Consent record:
```jsonc
{ "id", "worker_id", "admin_id", "consent_version",
  "decision": "ACCEPTED" | "REJECTED" | "WITHDRAWN" | "NARROWED",
  "consent_flags": { … requested flags shown … },
  "accepted_consent_flags": { … } | null,   // ACCEPTED: chosen subset (null in rows written before granular consent = all requested); REJECTED: null; WITHDRAWN: all false; NARROWED: effective after reduction
  "consent_descriptions": { … }, "manifest_hash": "64hex" | null,
  "ip_address", "user_agent", "app_version", "created_at" }
```

## 6. Workers (agent side)

### Enroll — `POST /workers/enroll` (public)
```json
{ "token": "A68K-2PZS-PC4H", "device_identifier": "64hex",
  "device_info": { "hostname": "JPEREZ-PC", "os": "Windows 11", "app_version": "0.3.0" } }
```
The server normalizes the code (uppercase, strips spaces/hyphens, `I`/`L`→`1`, `O`→`0`).
→ `{ worker_id, worker_name, organization, role_type, work_hours_start, work_hours_end, timezone,
status: "PENDING_CONSENT", auth: AuthResponse, admin_public_key, admin_public_key_fingerprint,
consent_version, consent_flags /* REQUESTED */, consent_descriptions }`.

| Answer | `details.reason` |
|:---|:---|
| `400` | `INVALID_ENROLLMENT_TOKEN` (unknown, expired, revoked or malformed — always the same body) |
| `409` | `ENROLLMENT_ALREADY_COMPLETED` (code already redeemed) |
| `409` | `DEVICE_ALREADY_ENROLLED` (device bound to another worker of the same admin) |
| `429` | `ENROLLMENT_LOCKED` + `Retry-After` (3 failures / 15 min per IP) |

The agent recomputes the commitment from the response with the received key and compares it with
the QR (or shows the safety code) **before** pinning the key (SEC-07).

### Self — `GET /workers/me`
```jsonc
{ "id", "status", "role_type", "work_hours_start", "work_hours_end", "timezone",
  "consent_version", "accepted_consent_version",
  "consent_flags", "requested_consent_flags", "consent_descriptions",
  "admin_public_key_fingerprint", "enrollment_completed_at",
  "ingestion_blocked_reason": null | "SUBSCRIPTION_SUSPENDED" | "SUBSCRIPTION_CANCELLED" }
```
Poll every 5 min, on startup and after any ingest `403`/`402`. `401` ⇒ revoked. Never returns the
public key (use the pinned one).

### Consent — `PATCH /workers/me/consent`
```json
{ "accepted": true, "consent_version": 3, "manifest_hash": "64 hex",
  "consent_flags": { "system_activity": true, "network_activity": false } }
```
- `accepted: true` + non-empty subset of the requested categories ⇒ effective = subset
  (`PENDING_CONSENT → ACTIVE`; `ACTIVE`/`PAUSED` keep status). Same version again with another subset
  changes the selection.
- `accepted: false` ⇒ reject (in `PENDING_CONSENT`) or withdraw (in `ACTIVE`/`PAUSED` → `PENDING_CONSENT`).
- `manifest_hash` = commitment of the **full** manifest displayed (requested flags + descriptions +
  pinned key), stored as evidence.
- Errors: `400 NO_CATEGORIES_SELECTED`, `403 CONSENT_FLAG_NOT_REQUESTED` (`details.categories`),
  `409 CONSENT_VERSION_MISMATCH` (`details.current_consent_version`), `403` (`details.status`) when
  `PENDING_ENROLLMENT`/`REVOKED`.
→ `{ status, consent_version, accepted_consent_version, consent_flags, consent_accepted_at, consent_rejected_at, enrollment_completed_at }`.

`PATCH /workers/:id/consent` (`decision: ACCEPT|REJECT|WITHDRAW`) is **deprecated** and removed in
v2 of the API; the apps no longer use it.

### Data subject rights (never blocked by the employer's subscription)
- `POST /workers/me/data-request` `{ "type": "EXPORT", "categories": [...], "from"?, "to"? }` → `202 { request_id, status, estimated_completion }`.
- `POST /workers/me/data-deletion` `{ "categories": ["all"] | [...], "reason"? }` → `202 { request_id, status, message }`.
- 5 requests per worker per 24 h (`429`).

## 7. Telemetry

### Ingest — `POST /telemetry/{system|network|productivity|files|location}` (worker)
```json
{ "records": [ { "ephemeral_public_key": "b64", "encrypted_payload": "b64",
                 "client_timestamp": "2026-09-30T14:00:00Z", "subcategory": "BASIC" } ] }
```
1–100 records, body ≤ 10 MiB, `subcategory` (`BASIC` | `SCREENSHOT`) only for productivity.
→ `201 { batch_id, records_created, category, server_timestamp }`.

Order of checks and what the agent does:

| Answer | Agent |
|:---|:---|
| `402` / `403` `SUBSCRIPTION_*` | **Keep** the envelope queued, stop collecting (`EmployerSuspended`), poll `/workers/me` |
| `403 WORKER_NOT_ACTIVE` / `CONSENT_NOT_GRANTED` / location outside FIELD+hours | **Drop** (never retry data without consent), sync `/workers/me` |
| `400` (incl. `EMPTY_PAYLOAD` + `details.index`) / `413` | Drop, log `error` |
| `401` | Single-flight refresh, retry once |
| `429` / `5xx` / network | Queue with backoff |

### Retrieve — `GET /telemetry/{category}?worker_id&from&to&page&per_page&sort&order[&subcategory]` (admin)
`CollectionResponse<{ id, worker_id, ephemeral_public_key, encrypted_payload (b64), payload_size, client_timestamp, server_timestamp, batch_id, subcategory? }>`.
`GET /telemetry/{category}/:id`, `DELETE /telemetry/{category}/:id`.

## 8. Health

`GET /health` → `{ status: "healthy", version, timestamp }`.
