#!/usr/bin/env bash
# Launches agy (implementer) non-interactively on one task, in a dedicated worktree and dev/* branch.
# Usage: REPO_DIR=<worktree> run_agy.sh <tasks/...md> <dev/branch> [model] [extra-notes-file]
# The branch is created from develop-AGENTS when it does not exist. Logs in $SCRATCH/logs.
set -u
REPO=${REPO_DIR:?REPO_DIR (agy worktree) is required}
TASK="$1"; BRANCH="$2"; MODEL="${3:-gemini-3.8-flash-high}"; EXTRA="${4:-}"
LOGDIR=${SCRATCH:-$HOME/.cache/telemetry-orchestration}/logs
mkdir -p "$LOGDIR"
LOG="$LOGDIR/$(basename "$TASK" .md)-$(date +%H%M%S).log"
cd "$REPO" || exit 2
[ -f "$TASK" ] || { echo "task file not found: $TASK"; exit 2; }
case "$BRANCH" in dev/*) ;; *) echo "branch must start with dev/"; exit 2;; esac
if git show-ref --verify --quiet "refs/heads/$BRANCH"; then git checkout -q "$BRANCH"; else git checkout -q -b "$BRANCH" develop-AGENTS; fi
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
echo "=== $(date -Is) START task=$TASK branch=$BRANCH model=$MODEL" > "$LOG"
timeout ${AGY_TIMEOUT:-7200} agy -p "$PROMPT" --dangerously-skip-permissions --model "$MODEL" >> "$LOG" 2>&1
RC=$?
echo "=== $(date -Is) END rc=$RC" >> "$LOG"
echo "LOG=$LOG RC=$RC"
