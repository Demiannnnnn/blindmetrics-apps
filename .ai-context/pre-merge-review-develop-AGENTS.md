# Pre-merge review — `develop-AGENTS` (telemetry-apps)

Date: 2026-11-12. Reviewer: orchestrator (Claude). Scope: everything on `develop-AGENTS` that is not on `main`
(224 commits at the time of the review). This repo has **no `develop` branch**: there is no PR. The owner decides
how and where `develop-AGENTS` lands (`main` or a new `develop`).

## 1. Summary

The desktop apps (worker service + window, admin console, `telemetry-crypto-core`) were reviewed for security,
correctness and their contract with `telemetry-api`. The review was done by reading the code. The planned second
opinion from Gemini 4 Argon did not run because its weekly quota was used up (it resets 2026-11-15).
The only change was a stale status note. No security or contract defect was confirmed in this repo.

## 2. Findings fixed

| ID | Severity | What | Fix |
|:--|:--|:--|:--|
| APPS-AUD-01 | info (docs) | `implementation-status.md` still said that removing an absence qualification needs `absence_id` in `DaySummary`. The API already sends `absence_id` and `absence_abbreviation`, and admin-client already uses them (`AttendanceDays.tsx` `currentAbsence`, Rust `DaySummary` in `commands/attendance.rs`). | Row updated (commit `cff0682`). |

## 3. Checked and OK (no change needed)

- **Worker service IPC** (`worker-client/ipc`, `service/src/ipc_server.rs`):
  - The Unix socket lives in a `0700` runtime directory. The code refuses symlinks and foreign owners, and the socket file is `0600`.
  - A `Hello` handshake is required within 5 s, with the protocol version checked and a 32-byte random token compared in constant time. The token file is `0600` and symlinks are refused.
  - Frames are limited to 1 MiB, and there are at most 16 concurrent connections (`BUSY` beyond that).
  - Quitting needs a one-shot nonce from the tray with a TTL.
- **Tauri windows**: strict CSP (`script-src 'self'`) in both apps. The worker window can only listen to events. It has no shell or opener permissions.
- **Contract apps → API**: every path called by the agent (`worker-client/service/src/api/client.rs`, `background/worker.rs`, `io/transport.rs`, `enrollment.rs`, `session.rs`) and by admin-client (`src-tauri/src/client.rs`, `commands/*.rs`) exists in the API router with the same method. Examples include `/workers/me/*`, `/workers/:id/*`, `/attendance/{me/*,days,absences/:id,corrections,board,marks,reports/:report[/export]}`, `/attendance/incidents`, `/attendance/settings`, `/admins/me/*` and `/subscriptions/me`.
  - Request bodies were checked against the API DTOs for the attendance and portal calls.
  - `GET /workers/:id/consent-records` from the worker agent is allowed by the API only for the worker's own id.
- **Absence qualification removal**: the API returns `DaySummary.absence_id`, which is covered by an HTTP test in the API (`tests/attendance_corrections_tests.rs`). The admin console enables *Quitar calificación* with it and uses the id saved in the session as a fallback.

## 4. Accepted risks

- On Windows the IPC uses a named pipe without a custom ACL. Access control relies on the token file in the user profile. This is the same model as on Unix minus the directory mode.
- The admin console opens external URLs only after `validate_external_url` (https, no credentials).

## 5. Open decisions for the owner (not decided here)

- Inspector temporary lockout policy (API TASK-37; see `handoff.md` §4).
- Legal text placeholders and provider data (`docs/legal/README.md`).
- Google OAuth client configuration, Flow credentials, release asset names and code signing.
- Where `develop-AGENTS` is merged, since there is no `develop` branch here.

## 6. Manual verification still pending

- Run the worker service and window on a real desktop (tray, quit confirmation, attendance PIN, offline queue), on Linux and Windows.
- Admin console against a real API: enrol a worker, qualify an absence and remove the qualification, export legal reports.
- Release build with the final legal texts (it fails on purpose while placeholders remain).
