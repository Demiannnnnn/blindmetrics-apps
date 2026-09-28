# AGENTS.md — telemetry-apps

Instructions for AI assistants and developers. Read fully before changing code.

## 1. What this repo is

A Cargo workspace with the two desktop clients of the Telemetry system plus their shared crypto crate. The server lives in the sibling repo `telemetry-api` and is **zero-knowledge**: it stores ciphertext and never decrypts.

| Directory | Package | Role |
|:----------|:--------|:-----|
| `telemetry-crypto-core/` | `telemetry-crypto-core` | X25519 + HKDF-SHA256 + AES-256-GCM envelopes; enrollment commitment. Pure library. |
| `agent/` | `agent` | **Worker app** on the employee's machine (egui). Enrolls, shows the consent manifest, collects only accepted categories, encrypts to the pinned admin key, sends/queues. |
| `admin-client/` | `admin-client-core` (Rust) + React UI | **Admin app** (Tauri 2). Holds `K_admin_priv` in the OS keystore, manages workers, issues enrollment QR codes, decrypts telemetry locally. |

"Worker" = the employee. "Agent" = the software on their machine. Old docs say `telemetry-worker` / `telemetry-admin`; those names do not exist in code.

## 2. Mandatory reading

1. [`.ai-context/README.md`](.ai-context/README.md) — index and naming table.
2. [`.ai-context/domain.md`](.ai-context/domain.md) — actors, worker lifecycle, consent model, categories.
3. [`.ai-context/constraints.md`](.ai-context/constraints.md) — hard rules with IDs (`DP-*`, `SEC-*`, `PERF-*`, `PLAT-*`, `LEG-*`).
4. [`.ai-context/implementation-status.md`](.ai-context/implementation-status.md) — **what is real vs. stub/mock**. Do not assume a feature works because a file for it exists.

Then, as relevant: `architecture.md`, `security-model.md`, `api-contract.md`, `conventions.md`, and the functional specs in `docs/`.

## 3. Non-negotiables (short version — full list in `constraints.md`)

- **Blind server:** never send plaintext telemetry, private keys, DEKs or shared secrets to the API (SEC-01). All crypto in `telemetry-crypto-core` (SEC-02).
- **Admin defines, worker accepts:** the worker accepts or rejects the whole consent package; no per-category worker toggles. Defaults are all `false` (DP-08). Withdraw is always ≤ 2 clicks (DP-10).
- **No collection unless `ACTIVE` and accepted on this device** (DP-06, DP-07).
- **Minimization:** keystroke counts only (DP-01), FQDN only (DP-02), file metadata only in designated folders (DP-03), GPS only FIELD + consent + work hours (DP-04).
- **Pinned key:** the agent encrypts only to the `K_admin_pub` verified at enrollment and never accepts a replacement from the server (SEC-07, SEC-08).
- **Tokens and keys never reach the WebView or plaintext files** (SEC-03..06).
- **Hooks are O(1) atomics; polling for idle/window; no locks across await** (PERF-01..06).
- **No fake data in production paths** (PLAT-02). Unimplemented ⇒ return "unsupported", don't return `"Finder"`.
- **No `unwrap`/`expect` outside tests** — release builds abort on panic.

## 4. How to work here

- Pick work from [`tasks/README.md`](tasks/README.md). Each task lists affected files, requirements, DoD and tests. If code and task disagree, check `implementation-status.md`; if still unclear, the `.ai-context` docs win over task pseudocode.
- Any HTTP change must match [`.ai-context/api-contract.md`](.ai-context/api-contract.md). Endpoints tagged **[T12]** depend on `telemetry-api` TASK-12 — code against them behind a clear error until the server ships them.
- Any change to plaintext payloads must update [`docs/telemetry-payloads.md`](docs/telemetry-payloads.md) and bump `schema_version`.
- Keep `implementation-status.md` truthful in the same PR that changes a capability.

## 5. Commands

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p agent                      # worker app (GUI)
cargo run -p agent -- --headless        # background only
cd admin-client && npm install && npm run tauri dev
cd admin-client && npm run build        # tsc + vite
```

The server: `cd ../telemetry-api && cargo run` (needs PostgreSQL; see its README).

## 6. Branches & commits

`dev` (protected) · `dev-architecture` · `dev-features` · `dev-admin/<topic>` · `dev-agent/<topic>`. Conventional commits (`feat(agent): …`, `fix(admin): …`, `docs(ai-context): …`).
