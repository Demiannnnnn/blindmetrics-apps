#!/usr/bin/env bash
# Same as run_agy.sh but retries (AGY_RETRIES, default 4) on transient agy errors, adding a resume note.
# Usage: REPO_DIR=<worktree> run_agy_retry.sh <task-file> <branch> [model] [extra-notes-file]
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
TASK="$1"; BRANCH="$2"; MODEL="${3:-gemini-3.8-flash-high}"; EXTRA="${4:-}"
RETRIES=${AGY_RETRIES:-4}
NOTE=$(mktemp); [ -n "$EXTRA" ] && cat "$EXTRA" > "$NOTE"
for i in $(seq 1 "$RETRIES"); do
  OUT=$(bash "$HERE/run_agy.sh" "$TASK" "$BRANCH" "$MODEL" "$NOTE")
  echo "$OUT"
  LOG=$(echo "$OUT" | sed -n 's/^LOG=\([^ ]*\).*/\1/p')
  if grep -qE "network issue|UNAVAILABLE \(code 503\)|connection reset|read: connection|RESOURCE_EXHAUSTED" "$LOG" 2>/dev/null && [ "$i" -lt "$RETRIES" ]; then
    echo "transient agy error (attempt $i), retrying in 60s"
    sleep 60
    { [ -n "$EXTRA" ] && cat "$EXTRA"; echo "RESUME: a previous attempt of yours on this task was cut by a network error. Start with git status and git log, review what is already done (commits and uncommitted changes), do NOT discard it, and continue from there."; } > "$NOTE"
    continue
  fi
  break
done
rm -f "$NOTE"
