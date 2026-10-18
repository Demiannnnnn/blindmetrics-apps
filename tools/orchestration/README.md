# Orchestration (Claude Code + agy)

Orchestrator tooling to delegate tasks to `agy` (Antigravity CLI) and validate its work. Not part of
the product. Same method as the owner's other repos. Claude's role is defined in [`CLAUDE.md`](../../CLAUDE.md).

## Environment (owner decision 2026-10-18: permanent, on every PC)

This scheme is **the** way of working on all of the owner's machines (three PCs, macOS and Linux): Claude
orchestrates, two Google accounts run agy **in parallel**, each in its own Docker container. Everything
needed to rebuild it is versioned in [`agy-env/`](agy-env/); the live copy on each PC is `~/agy-multi/`.

| Account | Container | Worktree | Models | Use it for |
|:--|:--|:--|:--|:--|
| Google AI **Pro** | `agy-pro` | `ta-wt-pro` | Gemini 3.8 Flash, **Gemini 4 Argon** | Heavier or riskier tasks (crypto, security, contract, cross-crate) and **second reviews** with `gemini-4-argon-high`. Has a 5-hour limit besides the weekly one. |
| Google AI **Plus** | `agy-plus` | `ta-wt-plus` | Gemini 3.8 Flash | Bounded tasks (UI screens, helpers, tests, small fixes). Weekly limit only. |

- Image: Debian 12 + Rust stable (clippy, rustfmt), Node 24, Tauri/WebKitGTK and xcap libs,
  `sqlx-cli` 0.7.4 (for `telemetry-api`), `agy`. Compose also runs `pg` (PostgreSQL 16, container
  `telemetry-pg`, reachable as host `pg` from the agy containers only) for `telemetry-api` tests.
  Each container keeps its own Google login in a named volume (`pro-home` / `plus-home`).
- `WORKSPACE` (the folder holding `telemetry-apps`, its siblings and the `ta-wt-*` worktrees) is
  mounted **at the same path** in both containers: host and containers share paths and `.git`.
- Volume `locks` is mounted at `/locks` in both: compile lock `/locks/telemetry-heavy.lock` and the
  per-worktree agent lock are shared across accounts.
- Every build/test runs in a container (`validate.sh` re-runs itself there); the host needs no
  Rust/Node. Native `agy` on the host is never used for tasks (on macOS it can hold only one account).

### Setting up a PC (new machine, reformat, or containers missing)

1. Clone the repo into the workspace folder (e.g. `~/Documents/Github` on macOS, `~/repositorios` on
   Linux) and check out `develop-AGENTS` in any worktree, or just run from `telemetry-apps` after
   `git fetch`.
2. Run **`tools/orchestration/agy-env/setup.sh`** as the owner. It is idempotent and:
   1. Docker engine — macOS: installs `docker`, `docker-compose`, `colima` with Homebrew (needs
      [Homebrew](https://brew.sh)), starts Colima (half the CPUs, ¾ of RAM up to 12 GiB) and enables
      it at login. Linux: requires Docker Engine + compose plugin and prints the install commands per
      distro if missing (`sudo` is the owner's call; then re-run). Windows: WSL2 + Ubuntu.
   2. Copies `agy-env/` to `~/agy-multi/` and writes `~/agy-multi/.env` (`WORKSPACE`, host UID).
   3. Builds the image and starts `agy-pro` / `agy-plus` (`restart: unless-stopped`).
   4. Installs `agy-pro`, `agy-plus`, `agy-login` in `~/.local/bin`.
   5. Creates the worktrees `ta-wt-orch` (`develop-AGENTS`), `ta-wt-pro`, `ta-wt-plus` in `WORKSPACE`
      (and `tw-wt-*` / `tapi-wt-*` for `telemetry-web` / `telemetry-api` when cloned),
      plus a bootstrap `WORKSPACE/CLAUDE.md` pointing to `ta-wt-orch/CLAUDE.md` (the owner's
      checkouts are on `main`, where these docs are empty).
   6. Checks toolchain and login of each account; exit 4 = some login missing.
3. Google login, once per account per PC (needs a browser, run it in a normal terminal):
   `agy-login pro`, `agy-login plus`. It opens the OAuth page and asks for the code (copying the URL
   by hand from the TUI breaks it: `redirect_uri_mismatch`).
4. Repo git identity if missing: `git config user.name …` / `git config user.email …` in `telemetry-apps`.
5. Check: `tools/orchestration/usage.sh` (quota of both accounts).

After changing anything in `agy-env/`, re-run `setup.sh` on each PC (logins survive: they live in the
volumes). Other tools: `agy-pro` / `agy-plus` run agy in the current dir; a shell:
`docker exec -it -w "$PWD" agy-pro bash`.

## Flow

1. Claude (orchestrator) writes/updates the docs (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) on
   `develop-AGENTS` in `ta-wt-orch`.
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
4. Accepted ⇒ `git merge --no-ff` into `develop-AGENTS` from `ta-wt-orch`, update
   `tasks/README.md` and `.ai-context/implementation-status.md`, push.

## Rules

- `main`, `dev` and `develop` are never touched; `develop-AGENTS` only by the orchestrator.
- One worktree per agent; never two agents in the same directory (enforced by
  `/locks/agy-<worktree>.lock`). Heavy commands through `flock /locks/telemetry-heavy.lock`.
- Parallelism: each account may run **one task per repo at the same time** (up to 6 agy runs: `ta-`,
  `tw-`, `tapi-wt-<account>`). Memory is protected by the `cargo` wrapper (`agy-env/cargo-lock-wrapper`,
  first in PATH): build/check/clippy/test/run take `/locks/cargo.lock`, so Rust compiles are serialized
  across containers while Node work runs in parallel; `CARGO_BUILD_JOBS=2` and line-table debug info
  keep each link small. Until containers are recreated with the current image, `run_agy.sh` and
  `validate.sh` put `/locks/bin` (same wrapper + `sqlx`, staged in the shared volume) first in PATH.
- Recreating the containers (`docker compose up -d` after an image/compose change) kills running agy
  runs: do it only when no task is running.
- Worktrees in `WORKSPACE`: `ta-wt-orch` (orchestrator, `develop-AGENTS`), `ta-wt-pro` (agy-pro),
  `ta-wt-plus` (agy-plus). The owner's checkout `telemetry-apps` is left alone.
- Delete a worktree's `target/` and `node_modules/` once its task is merged (disk space).
