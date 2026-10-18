#!/usr/bin/env bash
# Runs the REAL telemetry-api (origin/develop-AGENTS of ../telemetry-api) inside an agy container,
# against a fresh PostgreSQL database on the "pg" service, for the apps' live-contract tests.
# The API source is exported with `git archive` into $WORKSPACE/.e2e/telemetry-api (no write to the
# API repo). Reachable from both agy containers at http://agy-<host>:8080/api/v1.
# Usage: live-api.sh up|down|status   (LIVE_API_HOST=pro|plus, default plus)
set -eu
HOST_ACCT=${LIVE_API_HOST:-plus}; CTR="agy-$HOST_ACCT"
WS=$(cd "$(dirname "$0")/../../.." && pwd)
SRC="$WS/.e2e/telemetry-api"; DB=telemetry_e2e
URL="http://$CTR:8080/api/v1"
AGY_PATH="/locks/bin:/opt/tools/bin:/home/agy/.local/bin:/opt/rust/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
health() { docker exec "$CTR" curl -fsS -o /dev/null "http://127.0.0.1:8080/health" 2>/dev/null; }
case "${1:-}" in
  up)
    rev=$(git -C "$WS/telemetry-api" rev-parse origin/develop-AGENTS)
    if [ "$(cat "$SRC/.rev" 2>/dev/null)" != "$rev" ]; then
      mkdir -p "$SRC"; find "$SRC" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
      git -C "$WS/telemetry-api" archive "$rev" | tar -x -C "$SRC"; echo "$rev" > "$SRC/.rev"
    fi
    docker exec -w "$SRC" -e PATH="$AGY_PATH" -e CARGO_BUILD_JOBS=2 -e SQLX_OFFLINE=true \
      -e CARGO_PROFILE_DEV_DEBUG=line-tables-only "$CTR" cargo build --bin telemetry-api -q
    docker exec "$CTR" pkill -f "target/debug/telemetry-api" 2>/dev/null || true
    docker exec telemetry-pg dropdb -U postgres --if-exists "$DB"
    docker exec telemetry-pg createdb -U postgres "$DB"
    docker exec -d -w "$SRC" \
      -e SERVER_HOST=0.0.0.0 -e SERVER_PORT=8080 -e ENVIRONMENT=development -e RUST_LOG=info \
      -e DATABASE_URL="postgres://postgres:postgres@pg:5432/$DB" -e DATABASE_RUN_MIGRATIONS=true \
      -e JWT_SECRET=live_contract_tests_jwt_secret_at_least_32_chars_long \
      -e PUBLIC_API_URL="$URL" -e PORTAL_URL=http://localhost:3000 -e GOOGLE_AUTH_ENABLED=false \
      -e RATE_LIMIT_REQUESTS_PER_SECOND=1000 -e RATE_LIMIT_BURST_SIZE=2000 \
      "$CTR" sh -c "exec target/debug/telemetry-api > $SRC/server.log 2>&1"
    for _ in $(seq 1 60); do health && break; sleep 1; done
    health || { echo "API did not start; see $SRC/server.log"; tail -20 "$SRC/server.log"; exit 1; }
    echo "TELEMETRY_LIVE_API_URL=$URL  (rev ${rev:0:7}, db $DB, log $SRC/server.log)"
    ;;
  down) docker exec "$CTR" pkill -f "target/debug/telemetry-api" && echo stopped || echo "not running" ;;
  status) health && echo "up: $URL" || echo down ;;
  *) echo "usage: live-api.sh up|down|status"; exit 2 ;;
esac
