# Handoff (session / machine change)

Context to resume work without losing what is not in the code. The orchestrator updates it whenever
work pauses.

Last update: 2026-10-11 — v3 docs written (attendance pivot, terms, Tauri worker app); implementation
of v3 tasks starting (`tasks/README.md` §6).

## 1. State

- Integration branch: **`develop-AGENTS`** (created 2026-10-11 from `dev-ecosystem/style-and-fixed-url`
  `e6b80d1`, which contained every other branch). `main`/`dev`/`develop` untouched.
- v2 (contract v2, design system pass) is complete except the known gaps listed in
  `implementation-status.md` §5; three of them are closed by v3 tasks (`tasks/README.md` §6.3).
- v3 tasks: see `tasks/README.md` §6 and `implementation-status.md` §6.

## 2. Owner decisions (2026-10-11)

1. Main use of the product = **attendance record** under Chilean law; telemetry stays as an optional
   module with the same guarantees. Don't change the admin palette or layout.
2. Port the worker app to **Tauri** with **separate processes** (closing the window keeps a background
   service running). The service keeps a visible tray icon (no invisible monitoring).
3. Robust **terms and conditions** in both apps, plus privacy policy (`docs/legal/`).
4. Order: `telemetry-apps` first (fully working), then `telemetry-api` (contract v3), then explain it
   on `telemetry-web` (low priority).
5. Working method: Claude documents and approves/rejects; agy implements; frequent commits; merge to
   `develop-AGENTS` when done.

## 3. Orchestrator decisions taken without the owner (review them)

| Decision | Why | Where |
|:--|:--|:--|
| Attendance is **not** zero-knowledge (server-readable) | R38 requires emailed receipts, hash verification page, reports and inspector access | `security-model.md` §9.1 |
| Marking never blocked, **even with the subscription suspended**; admin keeps read-only reports | R38 art. 38; employer must keep records available (art. 58 l) | ATT-02, BILL-04 |
| Identification = enrolled device + **PIN verified locally**; secondary = web portal login (future) | R38 art. 7 g requires two methods, one non-biometric; PIN works offline | `security-model.md` §9.2 |
| Worker status no longer depends on telemetry consent (`PENDING_TERMS`, `telemetry_status`) | A worker who rejects telemetry must still mark (ATT-07) | `domain.md` §4–5, `api-contract.md` §12 |
| Telemetry `location` requires a declared safety/legal cause; `productivity_screenshots` unavailable for remote/hybrid workers | R38 arts. 53 e, 55 a, 56 — apply the reading most protective of the worker | `marco-legal-cl.md` §6 |
| Reports are computed and exported (XLSX/PDF/DOCX) **by the server** | One implementation for admin app, portal and inspectors; R38 art. 28 | `api-contract.md` §9.4 |
| Release builds fail while legal texts contain owner placeholders | Never ship terms with `[RUT]` etc. | LEG-06 |
| The product never claims DT authorization until it exists | R38 arts. 3, 50, 58 b | LEG-07 |

## 4. Pending owner input

- Provider data for the legal texts (`docs/legal/README.md` "Pendientes del dueño").
- Legal review of `docs/legal/*` and of telemetry `location`/`productivity_screenshots` under R38.
- Whether/when to start DT certification + authorization (R38 arts. 64–69).
- System email account for receipts and alerts (API work).

## 5. How to resume on another machine

1. `git clone … telemetry-apps && git checkout develop-AGENTS`.
2. Rust (system or rustup) with clippy + rustfmt; Node ≥ 22; WebKitGTK 4.1 on Linux for Tauri.
3. `cargo test --workspace`, `cd admin-client && npm ci && npm test && npm run build`.
4. Orchestration: `tools/orchestration/README.md` (worktrees, agy, validation).

## 6. Not in the repo

| What | Where | Rebuild |
|:--|:--|:--|
| Official text of R38 (PDF) | Diario Oficial 09-05-2024, edition 43.846, CVE 2489163 | `https://www.diariooficial.interior.gob.cl/publicaciones/2024/05/09/43846/01/2489163.pdf` (needs a browser User-Agent) |
| agy logs | `~/.cache/telemetry-orchestration/logs` | not needed |
| `.env` files | local | copy `.env.example` |
