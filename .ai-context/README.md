# AI Context — telemetry-apps

Read this directory before touching code. It is the source of truth for *what the apps are supposed to be*; the code is the source of truth for *what exists today*. When they disagree, `implementation-status.md` says which gaps are known — anything else is a bug in either the code or these docs, and you should flag it.

## Reading order

| # | File | Read it when |
|:--|:-----|:-------------|
| 1 | [`domain.md`](domain.md) | Always. Ubiquitous language, actors, worker lifecycle, consent model, telemetry categories. |
| 2 | [`architecture.md`](architecture.md) | Always. Crates, processes, threads, data flow, where each responsibility lives. |
| 3 | [`constraints.md`](constraints.md) | Always. Hard rules (privacy, security, performance). Violating one is a blocking defect. |
| 4 | [`security-model.md`](security-model.md) | Touching crypto, keys, tokens, enrollment, local storage. |
| 5 | [`api-contract.md`](api-contract.md) | Touching any HTTP call. Exact request/response shapes the apps must use. |
| 6 | [`conventions.md`](conventions.md) | Writing code, tests, commits. |
| 7 | [`implementation-status.md`](implementation-status.md) | Planning work. What is real, stubbed, mocked or missing — and which task fixes it. |

## Related documents

| Document | Location |
|:---------|:---------|
| Functional spec — Admin app (screens, flows, states) | [`docs/admin-app-spec.md`](../docs/admin-app-spec.md) |
| Functional spec — Worker app (screens, flows, states) | [`docs/worker-app-spec.md`](../docs/worker-app-spec.md) |
| Plaintext telemetry payload schemas (agent ⇄ admin contract) | [`docs/telemetry-payloads.md`](../docs/telemetry-payloads.md) |
| Task roadmap | [`tasks/README.md`](../tasks/README.md) |
| Server: enrollment & consent lifecycle design (v1 + v2) | `telemetry-api/ENROLLMENT.md` |
| Server: design of contract v2 (hardening, enrollment v2, Google, devices, vault, subscriptions) | `telemetry-api/ARCHITECTURE-CHANGES.md` and `telemetry-api/tasks/task-13..17` |
| Server: API spec, DB schema, security model | `telemetry-api/docs/` (`api-specification.md` is the HTTP source of truth) |
| Server: interactive contract | Swagger UI `/swagger-ui/` and `/api-docs/openapi.json` of a running API (dev builds) |
| Server-side mirror of these apps' spec and endpoint mapping | `telemetry-api/telemetry-apps.md` |
| `.tkb` reference vector shared with `telemetry-web` | [`tasks/crypto-core/task-03-key-backup-v2-argon2id.md`](../tasks/crypto-core/task-03-key-backup-v2-argon2id.md) |

## Naming — read this, the old docs were inconsistent

| Concept | Directory | Cargo package | Binary / bundle | Old names you may still see |
|:--------|:----------|:--------------|:----------------|:----------------------------|
| Worker app | `agent/` | `agent` | `agent` | `telemetry-worker`, "Worker Desktop App" |
| Admin app (Rust side) | `admin-client/src-tauri/` | `admin-client-core` | `Telemetry Admin` (`com.telemetry.admin`) | `telemetry-admin`, `telemetry-admin-core` |
| Admin app (UI) | `admin-client/src/` | npm `telemetry-admin` | — | `telemetry-admin-ui` |
| Shared crypto | `telemetry-crypto-core/` | `telemetry-crypto-core` | library | — |

Use the directory/package names in code and docs. "Worker" is the **person/role**; "agent" is the **software** on their machine.
