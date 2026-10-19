#!/usr/bin/env bash
# Native mode (owner decision 2026-10-19): agy runs directly on the host with the owner's main Google
# account (the Plus account's quota is too small to be worth the containers). Idempotent.
# Installs into ~/.cache/telemetry-orchestration: bin/{cargo,heavy} (lock wrappers) and sqlx-cli 0.7.4.
# Needs on the host: agy (logged in), Rust stable with clippy/rustfmt, Node ≥ 22, Docker (only for the
# telemetry-api PostgreSQL: `docker compose -f ~/agy-multi/compose.yml up -d pg`, port 127.0.0.1:5441).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
C=$HOME/.cache/telemetry-orchestration
mkdir -p "$C/bin" "$C/logs"
install -m 755 "$HERE/cargo" "$HERE/heavy" "$C/bin/"
command -v agy >/dev/null || { echo "✗ agy not installed (curl -fsSL https://antigravity.google/cli/install.sh | bash)"; exit 1; }
for t in cargo node npm git; do command -v "$t" >/dev/null || { echo "✗ missing $t"; exit 1; }; done
cargo clippy --version >/dev/null 2>&1 || echo "⚠ clippy missing (rustup component add clippy, or brew rust includes it)"
if [ ! -x "$C/tools/bin/sqlx" ]; then
  echo "Installing sqlx-cli 0.7.4 into $C/tools (one time, a few minutes)…"
  cargo install sqlx-cli --version 0.7.4 --locked --no-default-features --features postgres,rustls --root "$C/tools"
fi
agy -p "responde solo: ok" --print-timeout 60s 2>/dev/null | grep -qi ok && echo "✓ agy logged in" || echo "✗ agy not logged in: run agy and log in"
echo "✓ native mode ready ($C/bin, $C/tools/bin)"
