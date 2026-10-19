#!/usr/bin/env bash
# Remaining agy quota. Native mode (default): the host agy (owner's main account).
# Container mode: AGY_MODE=container usage.sh [pro|plus].
set -u
if [ "${AGY_MODE:-native}" = container ]; then
  for acct in ${1:-pro plus}; do
    echo "## $acct (container)"
    docker exec "agy-$acct" timeout 90 agy -p "/usage" --print-timeout 60s 2>&1 | grep -E "Limit|%" || echo "(no answer)"
  done
else
  echo "## host agy"
  agy -p "/usage" --print-timeout 60s 2>&1 | grep -E "Limit|%" || echo "(no answer; is agy logged in?)"
fi
