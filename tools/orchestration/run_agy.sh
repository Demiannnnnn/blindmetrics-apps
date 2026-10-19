#!/usr/bin/env bash
# Launches agy (implementer) non-interactively on one task, in a dedicated worktree and dev/* branch.
# Usage: [AGY_SLOT=<pro|plus|…>] [AGY_MODE=native|container] [REPO_DIR=<worktree>] \
#        run_agy.sh <tasks/...md> <dev/branch> [model] [extra-notes-file]
# AGY_MODE=native (default, owner decision 2026-10-19): agy runs on the host with the owner's main
#   account; parallel runs use different worktrees ("slots"): REPO_DIR defaults to ../ta-wt-<slot>.
# AGY_MODE=container: agy runs in agy-<AGY_ACCOUNT> (pro|plus), see agy-env/ in telemetry-apps.
# Body wrapped in main(): bash parses the whole file first, so editing it never breaks a running launch.
main() {
set -u
MODE=${AGY_MODE:-native}
SLOT=${AGY_SLOT:-${AGY_ACCOUNT:-pro}}
REPO=${REPO_DIR:-$(cd "$(dirname "$0")/../.." && cd .. && pwd)/ta-wt-$SLOT}
TASK="$1"; BRANCH="$2"; MODEL="${3:-gemini-3.8-flash-high}"; EXTRA="${4:-}"
HERE=$(cd "$(dirname "$0")" && pwd)
C=$HOME/.cache/telemetry-orchestration
LOGDIR=${SCRATCH:-$C}/logs
mkdir -p "$LOGDIR"
LOG="$LOGDIR/$(basename "$TASK" .md)-$SLOT-$(date +%H%M%S).log"
cd "$REPO" || exit 2
case "$BRANCH" in dev/*) ;; *) echo "branch must start with dev/"; exit 2;; esac
if [ "$MODE" = container ]; then
  CTR="agy-${AGY_ACCOUNT:-pro}"
  docker ps --format '{{.Names}}' | grep -qx "$CTR" || { echo "$CTR not running (container mode)"; exit 2; }
fi
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

echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) START task=$TASK branch=$BRANCH mode=$MODE slot=$SLOT model=$MODEL" > "$LOG"
T=${AGY_TIMEOUT:-14400}
if [ "$MODE" = container ]; then
  AGY_PATH="/locks/bin:/opt/tools/bin:/home/agy/.local/bin:/opt/rust/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
  docker exec -i -w "$PWD" -e PATH="$AGY_PATH" -e CARGO_BUILD_JOBS=2 -e CARGO_PROFILE_DEV_DEBUG=line-tables-only \
    -e CARGO_PROFILE_TEST_DEBUG=line-tables-only "$CTR" flock -n -E 75 "/locks/agy-$(basename "$PWD").lock" \
    timeout "$T" agy -p "$PROMPT" --dangerously-skip-permissions --model "$MODEL" >> "$LOG" 2>&1
  RC=$?
else
  [ -x "$C/bin/cargo" ] || { echo "native mode not set up: run telemetry-apps tools/orchestration/native/setup-native.sh"; exit 2; }
  export PATH="$C/bin:$C/tools/bin:$PATH" CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only
  # One agent per worktree (non-blocking lock, exit 75 = busy); timeout via perl (macOS has no timeout(1)).
  WL="/tmp/agy-$(basename "$PWD").lock"
  if command -v flock >/dev/null 2>&1; then LOCK=(flock -n -E 75 "$WL"); else LOCK=(lockf -k -t 0 "$WL"); fi
  "${LOCK[@]}" perl -e '$t=shift; alarm $t; exec @ARGV or die "exec: $!"' "$T" \
    agy -p "$PROMPT" --dangerously-skip-permissions --model "$MODEL" >> "$LOG" 2>&1
  RC=$?
fi
[ "$RC" -eq 75 ] && echo "BUSY: another agent is already running in $PWD" | tee -a "$LOG"
echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) END rc=$RC" >> "$LOG"
echo "LOG=$LOG RC=$RC"
}
main "$@"
exit $?
