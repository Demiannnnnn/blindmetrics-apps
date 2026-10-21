# API Contract (client side) — v2 + v3 (attendance, proposed)

> **v3 (2026-10-11):** §9–§12 and the changes marked **[v3]** are the contract the apps implement for
> the attendance pivot. They are **not implemented in `telemetry-api` yet** (planned as its
> TASK-18..23 after the apps). Until then, app code is tested against `wiremock` servers that reproduce
> these shapes exactly. Every v2 rule below still applies unless a [v3] note overrides it.

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
| `POST /workers/:id/portal-access` | **API TASK-27, en implementación.** `{ email }` → `200 WorkerResponse`. Ex-worker portal access (R38 art. 22.1): only `REVOKED` workers; read-only, `portal_access_until = now + 90 días`; replaces a previous one. `400 VALIDATION_ERROR` if not `REVOKED`; `404` worker of another admin; `409` email used by another worker of the admin (`personal_email` or a valid `portal_access_email`). Admin command `workers_grant_portal_access` (TASK-ADMIN-20). |
| `DELETE /workers/:id/portal-access` | **API TASK-27, en implementación.** `204`; removes it before it expires. Admin command `workers_revoke_portal_access`. |

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
  "created_at", "updated_at",
  "portal_access_until": "…" | null }        // API TASK-27, en implementación: ex-worker portal access deadline (RFC 3339); never the email
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


---

## 9. [v3] Attendance

All paths under `/api/v1`. Worker calls use the worker JWT; admin calls the admin JWT.
**None of them is blocked by the subscription guard** (BILL-04): worker attendance calls always work;
admin `GET` attendance calls work while blocked, admin writes (`POST /attendance/corrections`,
schedule changes) answer `402`/`403 SUBSCRIPTION_*` like the rest of the console.

### 9.1 Types

```jsonc
// MarkType
"WORKDAY_START" | "WORKDAY_END" | "BREAK_START" | "BREAK_END"
// MarkSource
"AGENT" | "PORTAL" | "CORRECTION"
// Mark
{ "id": "uuid", "client_mark_id": "uuid-v7" | null, "worker_id": "uuid", "type": "WORKDAY_START",
  "worker_run": "12345678-5", "employer_rut": "76123456-0",   // [v3.2] hashed fields: always present so the agent can recompute the hash
  "server_ts": "2026-10-11T12:00:05Z", "device_ts": "2026-10-11T12:00:04Z", "offline": false,
  "source": "AGENT", "identification": "DEVICE_PIN",
  "workplace": { "id": "uuid", "name": "Casa matriz", "address": "Av. Apoquindo 1234, piso 5, of. 501, Las Condes, Santiago, Región Metropolitana" },
  "location": { "lat": -33.41, "lon": -70.57, "accuracy_m": 25 } | null,
  "hash": "64hex", "prev_hash": "64hex",
  "corrected_by": "uuid (correction id)" | null,   // set when this mark was produced by a correction
  "superseded_by": "uuid (mark id)" | null,        // set on an original replaced by a consolidated correction
  "effective_ts": "2026-10-11T12:00:05Z" }         // [v3.1] the time that counts: device_ts if offline or source CORRECTION, else server_ts
```

**[v3.1]** Apps **display and group by `effective_ts`** (date and time of the mark); `server_ts` is
shown as "Registrada por el servidor" in the mark detail. `effective_ts` is not part of the hash.

### 9.2 Worker side

| Call | Body / query | Response |
|:--|:--|:--|
| `POST /attendance/marks` | `{ client_mark_id, type, device_ts, offline, identification: "DEVICE_PIN", location? }` | `201 { mark }` · `200 { mark, duplicate_of: "uuid" }` when it repeats the previous type consecutively (R38 art. 36 c) or `client_mark_id` was already stored (idempotent retry) |
| `GET /attendance/me/today` | — | `{ date, timezone, schedule_today: { start, end, break_start, break_end } \| null, state: "OFF" \| "WORKING" \| "ON_BREAK", next_mark: MarkType, marks: [Mark], pending_corrections: n, server_time }` |
| `GET /attendance/me/marks` | `from`, `to` (dates, ≤ 5 years back), `page`, `per_page` | `Collection<Mark>` newest first |
| `GET /attendance/me/days` | `from`, `to` (≤ 62 days) | `[DaySummary]` (§9.5) |
| `GET /attendance/me/corrections` | `status?` | `Collection<Correction>` |
| `POST /attendance/me/corrections/:id/response` | `{ "accept": true \| false }` | `200 Correction` · `409 CORRECTION_NOT_PENDING` |
| `GET /attendance/me/schedule` | — | `ScheduleVersion` currently in force + next one if scheduled |

Errors: `400 VALIDATION_ERROR` (`details.reason`: `INVALID_MARK_TYPE`, `DEVICE_TS_IN_FUTURE` (> 5 min
ahead of server time), `OFFLINE_MARK_TOO_OLD` (> 30 days)); `403 ATTENDANCE_EXEMPT` (art. 22);
`403 TERMS_NOT_ACCEPTED` only before the **first** acceptance (LEG-05). Never `402`.

### 9.3 Hash

As in `security-model.md` §9.3. The response `mark` carries **every** field of the canonical form
(including `worker_run` and `employer_rut`, [v3.2]); the agent recomputes it — a missing hashed field
shows as `HASH_MISMATCH` on the worker's receipt. Test vector (shared with `telemetry-api`): `telemetry-crypto-core/tests/vectors/attendance-mark-v1.json`.

### 9.4 Admin side

| Call | Notes |
|:--|:--|
| `GET /attendance/board` | One row per active worker: `{ worker_id, name, state, last_mark, today_marks, alerts: ["MISSING_START", …] }` |
| `GET /attendance/marks?worker_id&from&to&page` | `Collection<Mark>` |
| `GET /attendance/days?worker_id&from&to` | `[DaySummary]` |
| `POST /attendance/corrections` | `{ worker_ids: [uuid], kind: "MODIFY" \| "ADD" \| "FIX", original_mark_id?, type, proposed_ts, reason (5–500) }` → `201 [Correction]`. Errors (`details.reason`): `CORRECTION_TOO_EARLY` (same day), `CORRECTION_HARMS_WORKER` (start later / end earlier), `ORIGINAL_MARK_REQUIRED`, `AUTOMATED_WORKDAY_MARK_FORBIDDEN`. |
| `GET /attendance/corrections?worker_id&status&page` | `Collection<Correction>` |
| `DELETE /attendance/corrections/:id` | Withdraw while `PENDING` → `204`. |
| `GET /attendance/reports/{report}` | `report` ∈ `attendance`, `daily-hours`, `sundays-holidays`, `shift-changes`, `incidents`. Filters (R38 art. 25): `worker_ids`, `search` (name/RUN), `period` (`last_week`, `last_fortnight`, `last_month`) or `from`/`to`, `journey_type`, `schedule_id`, `workplace_id`, `job_title`, `hash`. → `{ report, generated_at, employer: {legal_name, rut}, sections: [ { worker: {name, run, workplace, job_title}, header_notes: [..], columns: [..], rows: [[..]], totals: [[..]] } ], abbreviations: {..}, empty_message?: "No hay trabajadores que coincidan con la selección" }` — cell values already formatted as R38 requires (`dd/mm/aa`, `hh:mm:ss`, `+`/`-`), altered cells flagged `{ "v": "08:02:00", "altered": true }`. |
| `GET /attendance/reports/{report}/export?format=xlsx\|pdf\|docx&…same filters` | Binary file, `Content-Disposition` with name. Identical content to the JSON (R38 art. 28 a). |
| `GET /attendance/incidents`, `POST /attendance/incidents` | Technical incidents `{ started_at, ended_at?, description }`. |
| `PUT /attendance/settings` | `{ missing_mark_alerts: bool }` (R38 art. 45.1, default true). |
| **[v3.1]** `PUT /attendance/absences` | `{ worker_id, date, kind: "JUSTIFIED" \| "UNJUSTIFIED", abbreviation?, note? (≤300) }` → `200 Absence`. The employer qualifies an absence; `abbreviation` ∈ R38 art. 28 list (`A.I.`, `A.J`, `AT`, `C.T.`, `D.E.J.`, `H.E.`, `J.O.`, `L.M.`, `P.G.R`, `PREN.`, `P.S.G.R.`, `POSTN`, `S.A`, `VAC`). Without it, a workday without marks has `absence: null` ("Sin marcas") — the system never labels an absence unjustified by itself. |
| **[v3.1]** `DELETE /attendance/absences/:id` | `204`. |
| **[v3.1]** `GET /attendance/verify/:hash` (public) | R38 art. 8 receipt check: `{ valid, type, effective_date, effective_time, employer_legal_name, workplace_name, offline, corrected }` or `404`. Used by the web portal. |

```jsonc
// Absence ([v3.2]; PUT /attendance/absences response)
{ "id": "uuid", "worker_id": "uuid", "date": "2026-10-09", "kind": "JUSTIFIED" | "UNJUSTIFIED",
  "abbreviation": "A.J" | null, "note": "…" | null, "created_at", "updated_at" }
// Correction
{ "id", "worker_id", "kind": "MODIFY", "original_mark": Mark | null, "type", "proposed_ts",
  "reason", "status": "PENDING" | "ACCEPTED" | "OPPOSED" | "CONSOLIDATED" | "WITHDRAWN",
  "created_at", "notified_at", "respond_before", "responded_at" | null, "resulting_mark_id" | null }
```

### 9.5 Day summary (server-computed, ATT-09)

```jsonc
{ "date": "2026-10-09", "agreed": { "start": "09:00:00", "end": "18:00:00", "break": "13:00:00-14:00:00", "break_counts_as_work": false } | null,
  "marks": [Mark], "attended": true, "absence": null | "JUSTIFIED" | "UNJUSTIFIED", "absence_note": null,
  "absence_id": null | "uuid", "absence_abbreviation": null | "A.J",   // [v3.2] needed to edit/remove a qualification
  "missing": "-00:12:30", "extra": "+00:40:00", "anomalies": ["BREAK_WITHOUT_END"], "altered": false }
```

## 10. [v3] Legal documents and terms acceptance

| Call | Body | Notes |
|:--|:--|:--|
| `GET /legal/current` (public) | — | `{ worker_terms: {version, hash}, employer_terms: {version, hash}, privacy: {version, hash} }` — the versions the server requires. |
| `POST /workers/me/terms-acceptance` | `{ document: "WORKER_TERMS", version, hash, privacy_version, privacy_hash }` | `200 { status, terms_accepted_version }`. First acceptance: `PENDING_TERMS → ACTIVE`. `409 TERMS_VERSION_MISMATCH` (`details.current_version`) if the app embeds an older version. Stored as evidence (IP, user agent, app version). |
| `POST /admins/me/terms-acceptance` | `{ document: "EMPLOYER_TERMS", version, hash, privacy_version, privacy_hash }` | Same. Exempt from the subscription guard. |

`GET /workers/me` and `GET /admins/me` add `terms_accepted_version`, `terms_required_version`.

## 11. [v3] Employer profile, workplaces and worker legal data

| Call | Body | Notes |
|:--|:--|:--|
| `GET /admins/me/employer` / `PUT` | `{ kind: "LEGAL_ENTITY" \| "NATURAL_PERSON", legal_name, rut, address: Address }` | RUT validated (mod 11), stored as `76.123.456-7`. Required before creating workers: `409 EMPLOYER_PROFILE_MISSING`. |
| `GET/POST /admins/me/workplaces`, `PUT/DELETE /admins/me/workplaces/:id` | `{ name, address: Address }` | Delete only if no worker uses it. |

`Address = { street, number, floor?, office?, commune, city, region }` (region from the 16 official
regions).

**[v3] Worker create/update** (`POST /workers`, `PUT /workers/:id`) adds:

```jsonc
{ "run": "12.345.678-5", "personal_email": "juan.perez@gmail.com",   // unique per employer (R38 art. 12 e)
  "job_title": "Vendedor", "workplace_id": "uuid",                   // REMOTE: workplace may be the worker's home address
  "work_mode": "ON_SITE" | "REMOTE" | "HYBRID",
  "art22_exempt": false,
  "schedule": ScheduleInput,                                         // required unless art22_exempt
  "location_cause": "string 20–500" | null }                          // required to request telemetry `location`
// ScheduleInput
{ "journey_type": "FIXED", "exceptional_resolution": { "number": "…", "date": "2025-01-10" } | null,
  "days": [ { "weekday": 1, "start": "09:00", "end": "18:00", "break_start": "13:00", "break_end": "14:00" } ],
  "break_counts_as_work": false,
  "disconnection": { "start": "20:00", "end": "08:00" } | null,     // REMOTE/HYBRID: ≥ 12 h
  "location_at_mark": false,                                         // true only with written consent (R38 art. 57.1)
  "effective_from": "2026-10-14", "requested_by": "EMPLOYER" | "WORKER", "note"? }
```

- `consent_flags` may now be **all false** (attendance-only worker). [v3] — v2 answered `400`.
- `productivity_screenshots` with `work_mode != ON_SITE` → `400 SCREENSHOTS_NOT_ALLOWED_REMOTE`.
- `location` without `location_cause` → `400 LOCATION_CAUSE_REQUIRED`.
- Schedule warnings (not errors, ATT-11): response `warnings: ["DAILY_MAX_EXCEEDED", "WEEKLY_MAX_EXCEEDED"]`.
- `PUT /workers/:id/schedule` (`ScheduleInput`) creates a new schedule version (feeds the shift-changes report).
- `WorkerResponse` adds `run`, `personal_email`, `job_title`, `workplace`, `work_mode`, `art22_exempt`,
  `schedule` (current), `telemetry_status`, `terms_accepted_version`.
- `work_hours_start`/`work_hours_end` stay for compatibility, derived from the schedule (earliest
  start / latest end of the current day) and used by telemetry DP-04/DP-05.

## 12. [v3] Worker lifecycle and telemetry state

- **[v3.2]** `GET /workers/me` also returns the worker's own `personal_email` (used by the agent for
  receipts and PIN-reset texts, shown masked). Only to the worker themself.
- `status` ∈ `PENDING_ENROLLMENT` | `PENDING_TERMS` | `ACTIVE` | `REVOKED` (v2 `PENDING_CONSENT` →
  `PENDING_TERMS`; v2 `PAUSED` → `telemetry_status: PAUSED`).
- `telemetry_status` ∈ `NOT_REQUESTED` | `PENDING_DECISION` | `ACCEPTED` | `DECLINED` | `PAUSED`.
- `PATCH /workers/me/consent`: unchanged body; `accepted: false` sets `DECLINED` and **does not change
  `status`**; allowed only when `status == ACTIVE` (`403 TERMS_NOT_ACCEPTED` otherwise).
- `PATCH /workers/:id/status` keeps only `{ status: "REVOKED" }`; telemetry pause moves to
  `PATCH /workers/:id/telemetry-status { status: "PAUSED" | "ACTIVE", reason? }`.
- Telemetry ingestion: `403 WORKER_NOT_ACTIVE` also when `telemetry_status != ACCEPTED`.
- PIN events and reset (worker): `POST /workers/me/credential-events { type: "PIN_SET" | "PIN_CHANGED" }` → `204` (server emails, R38 art. 7 f);
  `POST /workers/me/pin-reset/request` → `202` (code to personal email, 10 min);
  `POST /workers/me/pin-reset/verify { code }` → `204` · `400 INVALID_RESET_CODE` · `429` after 5 tries.
