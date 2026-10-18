# Orchestration (Claude Code + agy)

Orchestrator tooling to delegate tasks to `agy` (Antigravity CLI) and validate its work. Not part of
the product. Same method as the owner's other repos. Claude's role is defined in [`CLAUDE.md`](../../CLAUDE.md).

## Environment (since 2026-10-18, macOS host)

Two Google accounts run agy **in parallel**, each in its own Docker container (Colima engine). Setup
lives outside the repo in `~/agy-multi/` (Dockerfile, `compose.yml`, `agy-in`, `agy-login`).

| Account | Container | Worktree | Models | Use it for |
|:--|:--|:--|:--|:--|
| Google AI **Pro** | `agy-pro` | `../ta-wt-pro` | Gemini 3.8 Flash, **Gemini 4 Argon** | Heavier or riskier tasks (crypto, security, contract, cross-crate) and **second reviews** with `gemini-4-argon-high`. Has a 5-hour limit besides the weekly one. |
| Google AI **Plus** | `agy-plus` | `../ta-wt-plus` | Gemini 3.8 Flash | Bounded tasks (UI screens, helpers, tests, small fixes). Weekly limit only. |

- Image: Debian 12 + Rust stable (clippy, rustfmt), Node 24, Tauri/WebKitGTK libs, `agy`. Each
  container keeps its own login in a named volume (`pro-home` / `plus-home`).
- `~/Documents/Github` is mounted **at the same path** in both containers: host and containers share
  paths, worktrees and `.git`.
- Volume `locks` is mounted at `/locks` in both: the compile lock (`/locks/telemetry-heavy.lock`) and
  the per-worktree agent lock are shared across accounts.
- The macOS host has no Rust/Node: every build/test runs in a container (`validate.sh` does it by itself).
- Login (owner, once per account, browser needed): `agy-login pro` / `agy-login plus`.
- Interactive shell or one-off command: `agy-pro` / `agy-plus` (agy in the current dir), or
  `docker exec -it -w "$PWD" agy-pro bash`.

## Flow

1. Claude (orchestrator) writes/updates the docs (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) on
   `develop-AGENTS` in `../ta-wt-orch`.
2. Check quota: `usage.sh` (both accounts). Pick the account per the table above; with two
   independent tasks, launch both at once (one per account and worktree), in the background:
   `AGY_ACCOUNT=pro run_agy_retry.sh <tasks/...md> <dev/branch> [model] agy-note.txt`
   (`REPO_DIR` defaults to `../ta-wt-<account>`). agy runs non-interactively in its container on a
   `dev/*` branch created from `develop-AGENTS`. Logs in `$SCRATCH/logs` (default
   `~/.cache/telemetry-orchestration/logs`).
   - Default model `gemini-3.8-flash-high` (both accounts).
   - Exit 3 + `QUOTA_EXHAUSTED` ⇒ relaunch the same task and branch with the other account
     (`AGY_ACCOUNT=<other> REPO_DIR=<same worktree> …`); the worktree lock prevents overlaps.
   - `BUSY` ⇒ another agent is already working in that worktree.
3. Claude validates: `REPO_DIR=<worktree> validate.sh` (diff vs `develop-AGENTS`, protected files,
   cheats, fmt/clippy/test, UI build/test; runs in `agy-${VALIDATE_IN:-$AGY_ACCOUNT}` — use the
   container that built the worktree to reuse its build cache), reads the diff and the task Report,
   and runs extra checks. Optional second review on Pro with `gemini-4-argon-high` (read-only prompt,
   report to a file, never commits). Rejected ⇒ notes file + re-run on the same branch.
4. Accepted ⇒ `git merge --no-ff` into `develop-AGENTS` from `../ta-wt-orch`, update
   `tasks/README.md` and `.ai-context/implementation-status.md`, push.

## Rules

- `main`, `dev` and `develop` are never touched; `develop-AGENTS` only by the orchestrator.
- One worktree per agent; never two agents in the same directory (enforced by
  `/locks/agy-<worktree>.lock`). Heavy commands through `flock /locks/telemetry-heavy.lock`.
- Worktrees on this machine (`~/Documents/Github`): `ta-wt-orch` (orchestrator, `develop-AGENTS`),
  `ta-wt-pro` (agy-pro), `ta-wt-plus` (agy-plus). The owner's checkout `telemetry-apps` is left alone.
- Delete a worktree's `target/` and `node_modules/` once its task is merged (disk space).
