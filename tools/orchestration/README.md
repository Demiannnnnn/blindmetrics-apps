# Orchestration (Claude Code + agy)

Orchestrator tooling to delegate tasks to `agy` (Antigravity CLI) and validate its work. Not part of
the product. Same method as the owner's other repos.

## Flow

1. Claude (orchestrator) writes/updates the docs (`AGENTS.md`, `.ai-context/`, `docs/`, `tasks/`) on
   `develop-AGENTS`.
2. `REPO_DIR=<agy worktree> run_agy_retry.sh <tasks/...md> <dev/branch> [model] agy-note.txt` launches
   agy non-interactively on a `dev/*` branch created from `develop-AGENTS`. Logs in
   `$SCRATCH/logs` (default `~/.cache/telemetry-orchestration/logs`).
   - Default model `gemini-3.8-flash-high`. Quota: `agy -p "/usage"`.
3. Claude validates: `REPO_DIR=<worktree> validate.sh` (diff vs `develop-AGENTS`, protected files,
   cheats, fmt/clippy/test, UI build/test), reads the diff and the task Report, and runs extra
   checks. Rejected ⇒ notes file + re-run on the same branch.
4. Accepted ⇒ `git merge --no-ff` into `develop-AGENTS` from the orchestrator worktree, update
   `tasks/README.md` and `.ai-context/implementation-status.md`, push.

## Rules

- `main`, `dev` and `develop` are never touched; `develop-AGENTS` only by the orchestrator.
- One worktree per agent; never two agents in the same directory. Heavy commands through
  `flock /tmp/telemetry-heavy.lock`.
- Worktrees on this machine: `../ta-wt-orch` (orchestrator, `develop-AGENTS`), `../ta-wt-agy-a`
  (agent track), `../ta-wt-agy-b` (admin track). The owner's checkout `../telemetry-apps` is left alone.
