#!/usr/bin/env bash
# Runs a queue of tasks on ONE slot, fully in shell (no orchestrator tokens):
# implement (run_agy_retry) → validate → agy review → if anything fails, one automatic fix round with the
# validate/review findings as notes → summary file. The orchestrator then reads only the summaries.
# Usage: pipeline.sh <orch-dir> <slot> <queue-file>
#   queue line: <tasks/...md> <dev/branch> [model] [resume|-] [extra-note-file]   ('resume' adds the RESUME note)
# REVIEW_MODEL=<model> overrides the reviewer (default gemini-4-argon-high; claude-opus-5-5-high for security-heavy tasks).
# Summaries: ~/.cache/telemetry-orchestration/summary/<repo>-<task>.md  (STATUS: READY | NEEDS_ORCHESTRATOR)
main() {
set -u
ORCH=$(cd "${1:?orch dir}" && pwd); SLOT="${2:?slot}"; Q="${3:?queue file}"
OT=$ORCH/tools/orchestration; HERE=$(cd "$(dirname "$0")" && pwd)
C=$HOME/.cache/telemetry-orchestration; mkdir -p "$C/summary"
WT="$(dirname "$ORCH")/$(basename "$ORCH" | sed 's/-orch$//')-$SLOT"
REPO=$(basename "$ORCH" | sed 's/-wt-orch$//')
RESUME="RESUME: a previous attempt on this task was cut. Start with git status and git log, review what is already done (commits and uncommitted changes), do NOT discard it, finish the task, verify and write the Report."
while read -r TASK BRANCH MODEL FLAG XNOTE; do
  [ -z "${TASK:-}" ] && continue; case "$TASK" in \#*) continue;; esac
  MODEL=${MODEL:-gemini-3.8-flash-high}; S="$C/summary/$REPO-$(basename "$TASK" .md).md"
  NOTES=$(mktemp); cat "$OT/agy-note.txt" > "$NOTES"; [ "${FLAG:-}" = resume ] && echo "$RESUME" >> "$NOTES"
  [ -n "${XNOTE:-}" ] && [ -f "$XNOTE" ] && cat "$XNOTE" >> "$NOTES"
  echo "# $REPO $TASK ($BRANCH, slot $SLOT)" > "$S"; echo "STATUS: RUNNING" >> "$S"
  for round in 1 2; do
    R=$(cd "$ORCH" && AGY_SLOT=$SLOT bash "$OT/run_agy_retry.sh" "$TASK" "$BRANCH" "$MODEL" "$NOTES" 2>&1 | tail -3)
    echo "## round $round run: $(echo "$R" | tr '\n' ' ')" >> "$S"
    echo "$R" | grep -q QUOTA_EXHAUSTED && { echo "STATUS: NEEDS_ORCHESTRATOR (quota)" >> "$S"; break; }
    V=$(cd "$ORCH" && REPO_DIR=$WT TAILN=20 bash "$OT/validate.sh" 2>&1)
    VFAIL=$(echo "$V" | grep -E ': FAIL' ; echo "$V" | awk '/-- protected files touched:/{f=1;next} /^--/{f=0} f && !/\(none\)/')   # cheat patterns are judged by the reviewer (unwrap/expect are legal in tests)
    RV=$(bash "$HERE/review_agy.sh" "$WT" "$TASK" ${REVIEW_MODEL:-} 2>&1 | tail -1)
    RF=$(echo "$RV" | sed -n 's/^REVIEW_FILE=\([^ ]*\).*/\1/p'); VERD=$(echo "$RV" | sed -n 's/.*VERDICT=\(.*\)$/\1/p')
    { echo "## round $round validate"; echo "$V" | grep -E ': (OK|FAIL)|^branch=|Tests +[0-9]|^[0-9]+$' ; echo "## round $round review: $VERD ($RF)"; sed -n '/^FINDINGS:/,$p' "$RF" 2>/dev/null | head -25; } >> "$S"
    if [ -z "$(echo "$VFAIL" | tr -d '[:space:]')" ] && [ "$VERD" = PASS ]; then echo "STATUS: READY" >> "$S"; break; fi
    if [ "$round" = 2 ]; then echo "STATUS: NEEDS_ORCHESTRATOR" >> "$S"; break; fi
    { cat "$OT/agy-note.txt"; [ -n "${XNOTE:-}" ] && [ -f "$XNOTE" ] && cat "$XNOTE"; echo "FIX ROUND: your previous attempt on this task did not pass. Fix ALL of the following for real, re-run the checks, update the Report, commit."; echo "Validation failures:"; echo "$VFAIL" | head -40; echo "Reviewer findings:"; sed -n '/^FINDINGS:/,$p' "$RF" 2>/dev/null | head -40; } > "$NOTES"
  done
  rm -f "$NOTES"
done < "$Q"
}
main "$@"
exit $?
