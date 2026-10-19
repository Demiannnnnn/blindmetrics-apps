#!/usr/bin/env bash
# Read-only review of a task branch by agy (so reviews burn agy quota, not the orchestrator's).
# Usage: review_agy.sh <slot-worktree> <tasks/...md> [model] → prints REVIEW_FILE=<path> VERDICT=<PASS|FAIL|UNKNOWN>
# The review runs in a throwaway detached worktree of the branch HEAD: agy cannot touch the slot.
main() {
set -u
SLOT=$(cd "${1:?slot worktree}" && pwd); TASK="${2:?task file}"; MODEL="${3:-gemini-4-argon-high}"
C=$HOME/.cache/telemetry-orchestration; mkdir -p "$C/reviews"
BR=$(git -C "$SLOT" branch --show-current); HEAD=$(git -C "$SLOT" rev-parse HEAD)
OUT="$C/reviews/$(basename "$SLOT")-$(basename "$TASK" .md)-$(date +%H%M%S).md"
TMP=$(mktemp -d /tmp/agy-review-XXXX); git -C "$SLOT" worktree add -q --detach "$TMP/wt" "$HEAD" || exit 2
RULES=AGENTS.md; [ -f "$TMP/wt/AGENT.md" ] && RULES=AGENT.md
PROMPT="You are a strict code REVIEWER (read-only). Do NOT modify, create or delete any file, do NOT run git commands that change state, do NOT commit.
Review branch '$BR' (HEAD $HEAD) against base develop-AGENTS in this directory: run 'git diff develop-AGENTS...HEAD' and read the changed files.
Check, in this order: (1) every requirement (RF-*) and test listed in $TASK is implemented and the tests really exercise the behaviour (not trivial asserts); (2) the rules of $RULES (non-negotiables, forbidden cheats: no skipped/weakened tests, no lint suppressions, no unwrap/expect in production Rust, no 'any'/ts-ignore, no protected files modified, no secrets, no real network in tests); (3) correctness and security bugs in the diff (auth, crypto, legal/attendance rules, timezones, injection); (4) the task Report is truthful versus the code.
Write your result to stdout in exactly this format and nothing else after it:
VERDICT: PASS or VERDICT: FAIL
FINDINGS:
- [blocker|major|minor] <file>:<line> — <problem> — <precise fix>
(Only blocker/major findings justify FAIL. If there are none, write 'FINDINGS: none'.)"
( cd "$TMP/wt" && perl -e '$t=shift; alarm $t; exec @ARGV' 3600 agy -p "$PROMPT" --dangerously-skip-permissions --model "$MODEL" ) > "$OUT" 2>&1
DIRTY=$(git -C "$TMP/wt" status --porcelain | head -1)
git -C "$SLOT" worktree remove --force "$TMP/wt" >/dev/null 2>&1; rm -rf "$TMP"
V=$(grep -oE 'VERDICT: (PASS|FAIL)' "$OUT" | tail -1 | awk '{print $2}'); V=${V:-UNKNOWN}
[ -n "$DIRTY" ] && { echo "NOTE: reviewer modified files in its throwaway copy (discarded)" >> "$OUT"; }
echo "REVIEW_FILE=$OUT VERDICT=$V"
}
main "$@"
exit $?
