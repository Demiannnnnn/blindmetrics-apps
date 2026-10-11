# Conventions

## Rust

- Edition 2021, MSRV 1.75. `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` must pass (the workspace already sets `-D warnings` in `.cargo/config.toml`).
- **No `unwrap()` / `expect()` / `panic!` in non-test code.** Release builds use `panic = "abort"`, so a panic kills the agent silently on an employee's machine. Existing offenders are listed in `implementation-status.md`.
- Errors: `thiserror` enums per crate (`CryptoError`, `AgentError`, `AdminError`). Tauri commands return `Result<T, CommandError>` where `CommandError` serializes to `{ code, message }` — not bare `String`.
- Logging: `tracing` only, never `println!`. Respect the logging rules in `security-model.md` §5.
- Public items get `///` docs stating purpose, errors, and which constraint IDs they enforce.
- Strings that are domain enums (`"OFFICE"`, `"system"`, `"ACTIVE"`) are Rust enums with `serde(rename_all = "SCREAMING_SNAKE_CASE")` / `lowercase`, never raw `String` fields.
- Time: `chrono::DateTime<Utc>` on the wire; work-hours checks use `chrono-tz` with the worker's IANA timezone.

## Async & threads

- `tokio` for I/O, `rayon` for CPU-bound bulk decryption, plain `std::thread` only for OS hook loops that require it.
- Never hold a `Mutex`/`RwLock` guard across `.await`.
- The egui thread communicates with the runtime via `tokio::sync::{mpsc, watch}`; it never calls `block_on`.

## Attendance code

- Mark types, sources, statuses, journey types, work modes are Rust enums with
  `serde(rename_all = "SCREAMING_SNAKE_CASE")`, mirrored 1:1 in TS.
- Times on the wire: RFC 3339 UTC, **second precision** for marks. Display in the worker's timezone
  with `dd/mm/aa` and `hh:mm:ss` (R38 formats) — helpers in one module per app, tested.
- Durations from the server (`"+00:40:00"`) are displayed verbatim; apps never recompute report
  values (ATT-09).
- RUT/RUN: always through `telemetry_crypto_core::rut` (Rust) or its TS mirror in `src/lib/rut.ts`
  (tested against the same vectors).
- Every user-facing attendance string is es-CL, `tú`, and uses the R38 names for marks and reports.

## TypeScript / React (admin UI and agent-ui)

- `agent-ui` follows the same rules as the admin UI: strict TS, `src/api/tauri-bridge.ts` as the only `invoke` caller, tokens of the design system, no mocks reachable without `VITE_DEMO_MODE=true`, `node --test` unit tests for pure helpers.
- Strict TS. Types in `src/api/types.ts` mirror the Rust command DTOs 1:1 (snake_case field names as serialized by serde — do not rename in TS).
- All backend access via `src/api/tauri-bridge.ts`. Demo/mock data lives behind `import.meta.env.VITE_DEMO_MODE === 'true'` and is stripped from production builds.
- Decrypted telemetry is held in component/store state only; the lock handler clears it.
- Tailwind for styling; ECharts for charts; Leaflet for maps; `lucide-react` for icons.

## HTTP

- Base URL ends in `/api/v1` with no trailing slash; join paths with a helper, never string concatenation with `format!("{}{}")` that can double slashes.
- Every success body is `{ "data": … }`; every error `{ "error": { code, message, details } }`. Deserialize both explicitly (`api-contract.md`).
- Send `X-App-Version: <crate version>` on every request.
- Timeouts: 10 s per request; the agent's queue drain applies backoff (base 2 s, ×2, cap 300 s, ±20 % jitter).

## Tests

- Unit tests in-module (`#[cfg(test)]`), integration tests in `<crate>/tests/`.
- Name: `test_<module>_<behaviour>_<expected>`.
- Tests **must not** touch the real user config dir or OS keystore: inject paths (`tempfile`) and use an in-memory credential store trait. (Fixed in INT-01: `ui::tests::test_traffic_light_states` uses `WorkerConfig::load_or_init_at` with a `tempfile` directory; admin tests use `keystore::init_test_vault()` and never the OS keystore.)
- HTTP code is tested against `wiremock` (or `httpmock`) servers using the JSON shapes from `api-contract.md`.
- Privacy constraints get dedicated tests (e.g. `test_location_collector_not_started_for_office_role`).

## Git

- Conventional commits: `feat(agent): …`, `feat(agent-ui): …`, `fix(admin): …`, `docs(ai-context): …`, `feat(crypto-core): …`, with `Refs: TASK-…` in the body.
- Branches (since 2026-10-11, AGENTS.md §5): `main`/`dev`/`develop` untouchable; `develop-AGENTS` = orchestrator integration; implementers work on `dev/<task-topic>` created from `develop-AGENTS`. Older branches (`dev-admin/*`, `dev-agent/*`, `dev-ecosystem/*`) are history.
- A PR that changes behaviour updates the relevant task checkbox, `implementation-status.md`, and — if it changes a contract — `api-contract.md` / `docs/telemetry-payloads.md`.

## Definition of Done (every task)

1. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green.
2. `cd admin-client && npm run build` green (if UI touched).
3. No new `unwrap`/`expect` outside tests; no mock data reachable in production builds.
4. Constraint IDs touched are referenced in code comments and covered by tests.
5. Task DoD checkboxes ticked, `implementation-status.md` updated.

## Task report (`## Reporte` of each task file)

```markdown
## Reporte
- **Resumen:** 2–5 lines of what was done.
- **Archivos:** main files created/changed.
- **Comandos:** each command of AGENTS.md §9 with its real result (✅/❌ + test counts).
- **Decisiones:** every ambiguity and the option taken.
- **Pendientes / bloqueos:** what is missing and why.
- **Desviaciones del contrato:** any difference from `api-contract.md` / specs (should be none).
```
