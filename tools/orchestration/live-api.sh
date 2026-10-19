#!/usr/bin/env bash
# Runs the REAL telemetry-api (origin/develop-AGENTS of ../telemetry-api) against a fresh PostgreSQL
# database, for the apps' live-contract tests (TASK-INT-05). The API source is exported with
# `git archive` into $WORKSPACE/.e2e/telemetry-api (no write to the API repo).
# AGY_MODE=native (default): builds and runs on the host, http://127.0.0.1:18080/api/v1, database on
#   the telemetry-pg container published at 127.0.0.1:5441.
# AGY_MODE=container: runs inside agy-${LIVE_API_HOST:-plus}, http://agy-<host>:8080/api/v1.
# Usage: live-api.sh up|down|status
main() {
set -eu
MODE=${AGY_MODE:-native}
WS=$(cd "$(dirname "$0")/../../.." && pwd)
SRC="$WS/.e2e/telemetry-api"; DB=telemetry_e2e
JWT=live_contract_tests_jwt_secret_at_least_32_chars_long
if [ "$MODE" = container ]; then
  CTR="agy-${LIVE_API_HOST:-plus}"; URL="http://$CTR:8080/api/v1"
  health() { docker exec "$CTR" curl -fsS -o /dev/null "http://127.0.0.1:8080/health" 2>/dev/null; }
else
  PORT=${LIVE_API_PORT:-18080}; URL="http://127.0.0.1:$PORT/api/v1"
  health() { curl -fsS -o /dev/null "http://127.0.0.1:$PORT/health" 2>/dev/null; }
fi
case "${1:-}" in
  up)
    rev=$(git -C "$WS/telemetry-api" rev-parse origin/develop-AGENTS)
    if [ "$(cat "$SRC/.rev" 2>/dev/null)" != "$rev" ]; then
      mkdir -p "$SRC"; find "$SRC" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
      git -C "$WS/telemetry-api" archive "$rev" | tar -x -C "$SRC"; echo "$rev" > "$SRC/.rev"
    fi
    docker inspect -f '{{.State.Running}}' telemetry-pg 2>/dev/null | grep -q true \
      || docker compose -f "${AGY_ENV_DIR:-$HOME/agy-multi}/compose.yml" up -d pg >/dev/null
    for _ in $(seq 1 30); do docker exec telemetry-pg pg_isready -U postgres >/dev/null 2>&1 && break; sleep 1; done
    docker exec telemetry-pg dropdb -U postgres --if-exists "$DB"
    docker exec telemetry-pg createdb -U postgres "$DB"
    ENVV=(SERVER_PORT="${PORT:-8080}" ENVIRONMENT=development RUST_LOG=info DATABASE_RUN_MIGRATIONS=true
          JWT_SECRET="$JWT" PUBLIC_API_URL="$URL" PORTAL_URL=http://localhost:3000 GOOGLE_AUTH_ENABLED=false
          RATE_LIMIT_REQUESTS_PER_SECOND=1000 RATE_LIMIT_BURST_SIZE=2000)
    if [ "$MODE" = container ]; then
      AGY_PATH="/locks/bin:/opt/tools/bin:/home/agy/.local/bin:/opt/rust/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
      docker exec -w "$SRC" -e PATH="$AGY_PATH" -e CARGO_BUILD_JOBS=2 -e SQLX_OFFLINE=true \
        -e CARGO_PROFILE_DEV_DEBUG=line-tables-only "$CTR" cargo build --bin telemetry-api -q
      docker exec "$CTR" pkill -f "target/debug/telemetry-api" 2>/dev/null || true
      E=(); for v in "${ENVV[@]}" SERVER_HOST=0.0.0.0 DATABASE_URL="postgres://postgres:postgres@pg:5432/$DB"; do E+=(-e "$v"); done
      docker exec -d -w "$SRC" "${E[@]}" "$CTR" sh -c "exec target/debug/telemetry-api > $SRC/server.log 2>&1"
    else
      export PATH="$HOME/.cache/telemetry-orchestration/bin:$PATH"
      (cd "$SRC" && SQLX_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=line-tables-only \
        cargo build --bin telemetry-api -q)
      pkill -f "$SRC/target/debug/telemetry-api" 2>/dev/null || true
      (cd "$SRC" && env "${ENVV[@]}" SERVER_HOST=127.0.0.1 \
        DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5441/$DB" \
        nohup "$SRC/target/debug/telemetry-api" > "$SRC/server.log" 2>&1 &)
    fi
    for _ in $(seq 1 60); do health && break; sleep 1; done
    health || { echo "API did not start; see $SRC/server.log"; tail -20 "$SRC/server.log"; exit 1; }
    echo "TELEMETRY_LIVE_API_URL=$URL  (rev ${rev:0:7}, db $DB, log $SRC/server.log)"
    ;;
  down)
    if [ "$MODE" = container ]; then docker exec "$CTR" pkill -f "target/debug/telemetry-api" && echo stopped || echo "not running"
    else pkill -f "$SRC/target/debug/telemetry-api" && echo stopped || echo "not running"; fi ;;
  status) health && echo "up: $URL" || echo down ;;
  *) echo "usage: live-api.sh up|down|status"; exit 2 ;;
esac
}
main "$@"
exit $?
