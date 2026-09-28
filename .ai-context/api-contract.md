# API Contract (client side)

What the apps must send and expect. Verified against `telemetry-api` source (`src/*/models.rs`, `src/models/mod.rs`). Endpoints marked **[T12]** exist only after `telemetry-api` TASK-12 (`ENROLLMENT.md`); until then they return 404 and the apps must not assume them.

## 1. Envelopes

```jsonc
// success, single
{ "data": { … } }
// success, collection
{ "data": [ … ], "pagination": { "total": 120, "page": 1, "per_page": 20, "total_pages": 6 } }
// error
{ "error": { "code": "VALIDATION_ERROR", "message": "…", "details": null | { "reason": "…" } } }
```

`error.code` ∈ `VALIDATION_ERROR` (400) · `UNAUTHORIZED` (401) · `FORBIDDEN` (403) · `RESOURCE_NOT_FOUND` (404) · `CONFLICT` (409) · `PAYLOAD_TOO_LARGE` (413) · `RATE_LIMITED` (429) · `INTERNAL_ERROR` (5xx). Domain reasons ([T12]) arrive in `error.details.reason`.

Common client bug to avoid: reading `body.access_token` instead of `body.data.access_token`.

## 2. Auth

### Admin register — `POST /auth/register` (public) → `201`
```json
{ "email": "a@acme.cl", "password": "…", "name": "Ana", "organization": "Acme" }
```
Response `data` = `AuthResponse` (same as login). Log the admin in directly with it.

### Admin login — `POST /auth/login` (public) → `200`
```json
{ "email": "a@acme.cl", "password": "…", "role": "ADMIN" }
```
`role` is **required**. Response:
```json
{ "data": {
  "access_token": "eyJ…", "refresh_token": "hex…", "token_type": "Bearer", "expires_in": 900,
  "user": { "id": "uuid", "email": "…", "name": "…", "role": "ADMIN" } } }
```
`user` has no `organization`; fetch it with `GET /admins/me`.

### Worker login — `POST /auth/login` with `role: WORKER`
**Do not use.** It authenticates by email only (vulnerability) and is removed by [T12] (`WORKER_LOGIN_REMOVED`). Workers obtain tokens only from `POST /workers/enroll` and then rotate them.

### Refresh — `POST /auth/refresh` → `200`
`{ "refresh_token": "…" }` → `data: AuthResponse`. **Single use**: persist the new refresh token *before* using the new access token. A 401 here means the family was revoked (or the worker is `REVOKED` [T12]) → wipe the session.

### Logout — `POST /auth/logout` → `204`
`{ "refresh_token": "…" }`.

## 3. Admin

| Call | Notes |
|:-----|:------|
| `GET /admins/me` | `{ id, email, name, organization, is_active, worker_count, created_at, updated_at }` + [T12] `public_key`, `public_key_fingerprint` (nullable). |
| `PUT /admins/me` | `{ name?, organization? }`. |
| `PUT /admins/me/public-key` **[T12]** | `{ public_key }` → `{ public_key, public_key_fingerprint, public_key_registered_at }`. Write-once; same key ⇒ 200; different ⇒ 409 `PUBLIC_KEY_ALREADY_REGISTERED`. Call right after first-run key generation and on every login if `public_key` is null. |

## 4. Workers (admin side)

### Create — `POST /workers`
Current (pre-T12): requires `device_identifier` — **unusable** in practice, do not build UI around it.

[T12]:
```json
{
  "email": "jperez@acme.cl", "name": "Juan Pérez", "role_type": "OFFICE",
  "work_hours_start": "09:00:00", "work_hours_end": "18:00:00", "timezone": "America/Santiago",
  "consent_flags": { "system_activity": true, "network_activity": true, "productivity_basic": true,
                     "productivity_screenshots": false, "file_activity": false, "location": false },
  "consent_descriptions": { "system_activity": "…", "network_activity": "…", "productivity_basic": "…" },
  "enrollment_ttl_hours": 24
}
```
→ `201 { data: { worker: WorkerResponse, enrollment: { code, expires_at } } }`. The code is shown **once**; the admin app must not persist it.

Errors: `409 ADMIN_PUBLIC_KEY_MISSING`, `400` (missing description for an enabled flag, `location` with `OFFICE`).

### Other admin calls

| Call | Notes |
|:-----|:------|
| `GET /workers?page&per_page&search&role_type&is_active&status[T12]` | `CollectionResponse<WorkerResponse>`. |
| `GET /workers/:id` | `WorkerResponse`. |
| `PUT /workers/:id` | `{ name?, role_type?, work_hours_start?, work_hours_end?, timezone? }` ([T12] removes `device_identifier`, `is_active`). |
| `DELETE /workers/:id` | 204, soft delete ([T12] also `REVOKED`). |
| `POST /workers/:id/enrollment-token` **[T12]** | `{ enrollment_ttl_hours? }` → `{ code, expires_at }`. Re-binding revokes the old device. |
| `PUT /workers/:id/consent-config` **[T12]** | `{ consent_flags, consent_descriptions }` → `{ consent_version, requires_worker_acceptance, worker }`. |
| `PATCH /workers/:id/status` **[T12]** | `{ status: "PAUSED" \| "ACTIVE" \| "REVOKED", reason? }`. |
| `GET /workers/:id/consent-records` **[T12]** | Consent evidence, newest first. |

`WorkerResponse` ([T12] shape):
```jsonc
{
  "id": "uuid", "admin_id": "uuid", "email": "…", "name": "…",
  "device_identifier": "64hex" | null, "device_info": { "hostname", "os", "app_version" } | null,
  "role_type": "OFFICE" | "FIELD", "is_active": true,
  "status": "PENDING_ENROLLMENT" | "PENDING_CONSENT" | "ACTIVE" | "PAUSED" | "REVOKED",
  "status_changed_at": "…",
  "consent_flags": { …6 bools… },              // effective
  "requested_consent_flags": { …6 bools… },
  "consent_descriptions": { …6 × string|null… },
  "consent_version": 2, "accepted_consent_version": 1 | null, "consent_accepted_at": "…" | null,
  "enrollment_completed_at": "…" | null, "enrollment_expires_at": "…" | null,
  "work_hours_start": "09:00:00" | null, "work_hours_end": "18:00:00" | null, "timezone": "America/Santiago",
  "created_at": "…", "updated_at": "…"
}
```

## 5. Workers (agent side)

### Enroll — `POST /workers/enroll` **[T12]** (public, 10 req / 15 min / IP)
```json
{ "token": "K7QM-4XRT-9PWD", "device_identifier": "64hex",
  "device_info": { "hostname": "JPEREZ-MBP", "os": "macOS 15.1", "app_version": "0.2.0" } }
```
→ `200 { data: { worker_id, worker_name, organization, role_type, work_hours_start, work_hours_end, timezone, status, auth: AuthResponse, admin_public_key, admin_public_key_fingerprint, consent_version, consent_flags /*requested*/, consent_descriptions } }`.
Errors: `400 INVALID_ENROLLMENT_TOKEN` (always the same, whatever the cause), `409 DEVICE_ALREADY_ENROLLED`, `429`.

### Self — `GET /workers/me` **[T12]**
`{ id, status, role_type, work_hours_start, work_hours_end, timezone, consent_version, accepted_consent_version, consent_flags, requested_consent_flags, consent_descriptions, admin_public_key_fingerprint }`. Poll every 5 min and after any 403 from ingestion. 401 ⇒ revoked.

### Consent — `PATCH /workers/:id/consent`
Current (pre-T12): `{ consent_flags: {…} }` — worker-chosen matrix. **Obsolete model; do not build UI on it.**
[T12]: `{ "decision": "ACCEPT" | "REJECT" | "WITHDRAW", "consent_version": 1, "manifest_hash": "64hex" }` → `{ status, consent_version, accepted_consent_version, consent_flags, consent_accepted_at }`.
Errors: `409 CONSENT_VERSION_MISMATCH` (refresh `/workers/me`, re-show screen), `409 INVALID_STATUS_TRANSITION`, `403`.

### Data subject rights
- `POST /workers/me/data-request` `{ "type": "EXPORT", "categories": ["system", …], "from"?, "to"? }` → `202 { request_id, status, estimated_completion }`.
- `POST /workers/me/data-deletion` `{ "categories": ["all"] | […], "reason"? }` → `202 { request_id, status, message }`.

## 6. Telemetry

### Ingest — `POST /telemetry/{system|network|files|location}` (worker)
```json
{ "records": [
  { "ephemeral_public_key": "b64 (44 chars)", "encrypted_payload": "b64", "client_timestamp": "2026-09-28T14:00:00Z" }
] }
```
1–100 records per request; body ≤ 10 MB. One record = one envelope = one `PlaintextBatch` of **that** category.
→ `201 { data: { batch_id, records_created, category, server_timestamp } }`.

### Ingest — `POST /telemetry/productivity`
Same, plus `"subcategory": "BASIC" | "SCREENSHOT"` per record. BASIC and SCREENSHOT batches are separate envelopes.

Rejections (`403`): missing consent flag (`CONSENT_NOT_GRANTED` [T12]), worker not `ACTIVE` (`WORKER_NOT_ACTIVE` [T12]), location from non-FIELD or outside work hours. **Drop these envelopes; never queue them.**

### Retrieve — `GET /telemetry/{category}?worker_id=…&from&to&page&per_page[&subcategory]` (admin)
`CollectionResponse<{ id, worker_id, ephemeral_public_key, encrypted_payload (b64), payload_size, client_timestamp, server_timestamp, batch_id, subcategory? }>`.

`GET /telemetry/{category}/:id`, `DELETE /telemetry/{category}/:id` (admin).

## 7. Health

`GET /health` → `{ status: "healthy", version, timestamp }`. Used by the agent's diagnostics tab and before draining the queue.
