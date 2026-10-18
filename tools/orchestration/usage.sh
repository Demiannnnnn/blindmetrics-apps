#!/usr/bin/env bash
# Prints the remaining agy quota of both Google accounts (containers agy-pro and agy-plus).
# Usage: usage.sh [pro|plus]
set -u
for acct in ${1:-pro plus}; do
  echo "## $acct"
  docker exec "agy-$acct" timeout 90 agy -p "/usage" --print-timeout 60s 2>&1 | grep -E "Limit|%" || echo "(no answer; is agy-$acct running and logged in?)"
done
