#!/usr/bin/env bash
# Validates the current branch of REPO_DIR against develop-AGENTS. Compact output for the orchestrator.
set -u
# The host (macOS) has no Rust/Node toolchain: re-run inside an agy container, same image as the implementers.
# VALIDATE_IN picks the container (default: AGY_ACCOUNT, else pro); use the one that built the worktree.
if ! command -v cargo >/dev/null 2>&1; then
  SELF=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")
  R=$(cd "${REPO_DIR:?REPO_DIR required}" && pwd) || exit 2
  AGY_PATH="/locks/bin:/opt/tools/bin:/home/agy/.local/bin:/opt/rust/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
  exec docker exec -e PATH="$AGY_PATH" -e CARGO_BUILD_JOBS=2 -e CARGO_PROFILE_DEV_DEBUG=line-tables-only -e CARGO_PROFILE_TEST_DEBUG=line-tables-only -e REPO_DIR="$R" -e LOCK -e TAILN "agy-${VALIDATE_IN:-${AGY_ACCOUNT:-pro}}" bash "$SELF"
fi
cd ${REPO_DIR:?REPO_DIR required} || exit 2
B=$(git branch --show-current); echo "branch=$B"
git log --oneline develop-AGENTS..HEAD | head -20
echo "-- uncommitted:"; git status --short | head -10
echo "-- protected files touched:"; git diff --name-only develop-AGENTS...HEAD | grep -E "^(AGENTS\.md|CLAUDE\.md|GEMINI\.md|\.ai-context/|docs/|\.claude/|\.agents/|tasks/README\.md|tools/orchestration/)" || echo "(none)"
echo "-- task files touched:"; git diff --name-only develop-AGENTS...HEAD | grep -E "^tasks/" || echo "(none)"
echo "-- cheats:"; git diff develop-AGENTS...HEAD -- . ':!*.lock' ':!package-lock.json' ':!*.md' | grep -nE "^\+" | grep -E "(#!?\[allow\(|#\[ignore|\.unwrap\(\)|\.expect\(|panic!\(|: any\b|<any>|as any|@ts-ignore|@ts-expect-error|@ts-nocheck|eslint-disable|\.skip\(|\.only\(|as unknown as|dangerouslySetInnerHTML)" | head -40 || true
echo "-- stat:"; git diff --stat develop-AGENTS...HEAD | tail -1
run(){ local n=$1; shift; if out=$("$@" 2>&1); then echo "$n: OK"; else echo "$n: FAIL"; echo "$out" | grep -vE "^\s*$" | tail -${TAILN:-30}; fi; }
L=${LOCK:-/locks/telemetry-heavy.lock}  # volume shared by agy-pro and agy-plus
run fmt cargo fmt --all --check
run clippy flock "$L" cargo clippy --workspace --all-targets -- -D warnings
run test flock "$L" cargo test --workspace
echo "-- rust test counts:"; flock "$L" cargo test --workspace 2>&1 | grep -E "^test result" | awk '{print $4}' | paste -sd+ | bc 2>/dev/null
for ui in admin-client agent-ui; do
  [ -f "$ui/package.json" ] || continue
  run "$ui:install" bash -c "cd $ui && npm ci --silent"
  run "$ui:build" flock "$L" bash -c "cd $ui && npm run build"
  run "$ui:test" bash -c "cd $ui && npm test"
done
