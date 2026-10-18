#!/usr/bin/env bash
# Launches agy (implementer) non-interactively on one task, in a dedicated worktree and dev/* branch.
# Runs on the host; agy runs inside the Docker container of the chosen Google account (agy-pro / agy-plus).
# Usage: AGY_ACCOUNT=<pro|plus> [REPO_DIR=<worktree>] run_agy.sh <tasks/...md> <dev/branch> [model] [extra-notes-file]
# REPO_DIR defaults to ../ta-wt-<account>. The branch is created from develop-AGENTS when it does not
# exist. Logs in $SCRATCH/logs.
set -u
ACCOUNT=${AGY_ACCOUNT:?AGY_ACCOUNT (pro|plus) is required}
case "$ACCOUNT" in pro|plus) ;; *) echo "AGY_ACCOUNT must be pro or plus"; exit 2;; esac
CTR="agy-$ACCOUNT"
REPO=${REPO_DIR:-$(cd "$(dirname "$0")/../.." && cd .. && pwd)/ta-wt-$ACCOUNT}
TASK="$1"; BRANCH="$2"; MODEL="${3:-gemini-3.8-flash-high}"; EXTRA="${4:-}"
LOGDIR=${SCRATCH:-$HOME/.cache/telemetry-orchestration}/logs
mkdir -p "$LOGDIR"
LOG="$LOGDIR/$(basename "$TASK" .md)-$ACCOUNT-$(date +%H%M%S).log"
cd "$REPO" || exit 2
case "$BRANCH" in dev/*) ;; *) echo "branch must start with dev/"; exit 2;; esac
if ! docker ps --format '{{.Names}}' | grep -qx "$CTR"; then
  if command -v colima >/dev/null 2>&1 && ! docker info >/dev/null 2>&1; then colima start >/dev/null; fi
  docker compose -f "${AGY_ENV_DIR:-$HOME/agy-multi}/compose.yml" up -d "$CTR" >/dev/null \
    || { echo "cannot start $CTR: run tools/orchestration/agy-env/setup.sh"; exit 2; }
fi
docker exec "$CTR" test -d "$PWD" || { echo "$PWD is not mounted in $CTR (outside WORKSPACE of ~/agy-multi/.env)"; exit 2; }
if git show-ref --verify --quiet "refs/heads/$BRANCH"; then git checkout -q "$BRANCH"; else git checkout -q -b "$BRANCH" develop-AGENTS; fi || exit 2
# Checked after the checkout: the worktree may still be on an older commit without the task file
[ -f "$TASK" ] || { echo "task file not found on $BRANCH: $TASK"; exit 2; }
EXTRA_TXT=""; [ -n "$EXTRA" ] && EXTRA_TXT="$(cat "$EXTRA")"
PROMPT="You are the implementing agent of this repository. Your assigned task is: $TASK
You are on git branch '$BRANCH' (already checked out; work and commit only here).

Instructions:
1. Follow AGENTS.md strictly (it overrides everything). Read AGENTS.md, ALL of .ai-context/, tasks/README.md, your task file and every doc it references BEFORE writing code.
2. Implement the WHOLE task. Do not ask for confirmation: no human is available. On ambiguity choose the most conservative option that satisfies .ai-context and write it in the task Report.
3. Run and make pass from the repo root (AGENTS.md section 9): cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace, and the npm build/test of every UI you touched. Fix errors for real (no cheats, no allow attributes, no skipped tests).
4. Commit on '$BRANCH' with Conventional Commits and 'Refs: <task id>'. At the end update the Estado row and the '## Reporte' section of your task file with REAL results, and commit.
5. NEVER touch main, dev, develop or develop-AGENTS. Never modify sibling repositories.
$EXTRA_TXT"
echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) START task=$TASK branch=$BRANCH account=$ACCOUNT model=$MODEL" > "$LOG"
# /locks/bin (shared volume) first: cargo wrapper that serializes heavy cargo commands across all agy
# containers (memory) and sqlx; the image has the same in /opt/tools/bin once containers are recreated.
AGY_PATH="/locks/bin:/opt/tools/bin:/home/agy/.local/bin:/opt/rust/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
# One agent per worktree, whatever the account: the lock lives in the volume shared by both containers.
docker exec -i -w "$PWD" -e PATH="$AGY_PATH" -e CARGO_BUILD_JOBS=2 -e CARGO_PROFILE_DEV_DEBUG=line-tables-only -e CARGO_PROFILE_TEST_DEBUG=line-tables-only "$CTR" flock -n -E 75 "/locks/agy-$(basename "$PWD").lock" \
  timeout "${AGY_TIMEOUT:-14400}" agy -p "$PROMPT" --dangerously-skip-permissions --model "$MODEL" >> "$LOG" 2>&1
RC=$?
[ "$RC" -eq 75 ] && echo "BUSY: another agent is already running in $PWD" | tee -a "$LOG"
echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) END rc=$RC" >> "$LOG"
echo "LOG=$LOG RC=$RC"
