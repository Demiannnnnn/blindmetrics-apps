#!/usr/bin/env bash
# Same as run_agy.sh but retries (AGY_RETRIES, default 4) on transient agy errors, adding a resume note.
# Quota exhaustion is not transient: after one retry it stops with exit 3 and "QUOTA_EXHAUSTED", so the
# orchestrator can relaunch on the other account (AGY_ACCOUNT=pro|plus) on the same branch.
# Usage: AGY_ACCOUNT=<pro|plus> [REPO_DIR=<worktree>] run_agy_retry.sh <task-file> <branch> [model] [extra-notes-file]
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
TASK="$1"; BRANCH="$2"; MODEL="${3:-gemini-3.8-flash-high}"; EXTRA="${4:-}"
RETRIES=${AGY_RETRIES:-4}
RESUME="RESUME: a previous attempt on this task was cut (network error or quota). Start with git status and git log, review what is already done (commits and uncommitted changes), do NOT discard it, and continue from there."
NOTE=$(mktemp); [ -n "$EXTRA" ] && cat "$EXTRA" > "$NOTE"
QUOTA_HITS=0
for i in $(seq 1 "$RETRIES"); do
  OUT=$(bash "$HERE/run_agy.sh" "$TASK" "$BRANCH" "$MODEL" "$NOTE")
  echo "$OUT"
  LOG=$(echo "$OUT" | sed -n 's/^LOG=\([^ ]*\).*/\1/p')
  if grep -qE "RESOURCE_EXHAUSTED|[Qq]uota exceeded|[Ll]imit reached" "$LOG" 2>/dev/null; then
    QUOTA_HITS=$((QUOTA_HITS + 1))
    if [ "$QUOTA_HITS" -ge 2 ] || [ "$i" -ge "$RETRIES" ]; then
      echo "QUOTA_EXHAUSTED account=${AGY_ACCOUNT:-?} (check tools/orchestration/usage.sh; relaunch with the other account)"
      rm -f "$NOTE"; exit 3
    fi
  elif ! grep -qE "network issue|UNAVAILABLE \(code 503\)|connection reset|read: connection" "$LOG" 2>/dev/null || [ "$i" -ge "$RETRIES" ]; then
    break
  fi
  echo "transient agy error (attempt $i), retrying in 60s"
  sleep 60
  { [ -n "$EXTRA" ] && cat "$EXTRA"; echo "$RESUME"; } > "$NOTE"
done
rm -f "$NOTE"
