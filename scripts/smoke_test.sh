#!/usr/bin/env bash
#
# End-to-end smoke test: boots the server on a scratch DB and drives the whole
# flow over HTTP (index -> form -> submit -> admin auth -> submissions).
#
#   bash scripts/smoke_test.sh
#
set -uo pipefail
cd "$(dirname "$0")/.."

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required for this script" >&2
  exit 1
fi

export HOST=127.0.0.1
export PORT=3111
export DB_PATH=/tmp/forms_smoke_$$.db
export FORMS_DIR=forms
export ADMIN_PASSWORD=testpass123
export RUST_LOG=info

BASE="http://127.0.0.1:$PORT"
UUID=8f14e45f-ceea-467f-8d3d-1890c9784b79
LOG=/tmp/forms_smoke_$$.log
rm -f "$DB_PATH" "$DB_PATH-wal" "$DB_PATH-shm"

./target/debug/forms >"$LOG" 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null' EXIT

# Wait until the server is accepting connections.
up=0
for _ in $(seq 1 50); do
  if curl -sf "$BASE/healthz" >/dev/null 2>&1; then up=1; break; fi
  sleep 0.2
done
if [ "$up" != "1" ]; then
  echo "!! server did not come up; log follows:"
  cat "$LOG"
  exit 1
fi

section() { printf '\n=== %s ===\n' "$1"; }

section "GET /healthz"
curl -s -o /dev/null -w 'HTTP %{http_code}\n' "$BASE/healthz"

section "GET / (redirects to GitHub)"
curl -s -o /dev/null -w 'HTTP %{http_code} -> %{redirect_url}\n' "$BASE/"

section "GET /$UUID (form renders its fields)"
curl -s "$BASE/$UUID" | grep -oE 'Full name|Email address|Your message|<form ' | sort -u

section "GET /$UUID?admin=true (not authed -> password prompt)"
curl -s "$BASE/$UUID?admin=true" | grep -oE 'Administrator access|admin password' | sort -u

section "POST /$UUID (valid submission -> 303 redirect)"
curl -s -X POST "$BASE/$UUID" \
  --data-urlencode "name=Ada Lovelace" \
  --data-urlencode "email=ada@example.com" \
  --data-urlencode "topic=Support" \
  --data-urlencode "message=Hello, this is a smoke test." \
  -o /dev/null -w 'HTTP %{http_code} -> %{redirect_url}\n'

section "POST /$UUID (missing required field -> 422 + error)"
curl -s -X POST "$BASE/$UUID" --data-urlencode "name=Nope" \
  -o /tmp/forms_smoke_422.html -w 'HTTP %{http_code}\n'
grep -oE 'is required' /tmp/forms_smoke_422.html | sort -u

section "POST /admin/auth (wrong password)"
curl -s -X POST "$BASE/$UUID/admin/auth" --data-urlencode "password=wrong" \
  -o /dev/null -w 'HTTP %{http_code}\n'

section "POST /admin/auth (correct password -> sets cookie)"
COOKIE_JAR=/tmp/forms_smoke_$$.cookies
curl -s -c "$COOKIE_JAR" -X POST "$BASE/$UUID/admin/auth" --data-urlencode "password=testpass123" \
  -o /dev/null -w 'HTTP %{http_code} -> %{redirect_url}\n'

section "GET /$UUID?admin=true (with cookie -> submissions table)"
curl -s -b "$COOKIE_JAR" "$BASE/$UUID?admin=true" \
  | grep -oE 'Submissions|Ada Lovelace|ada@example.com|Support|smoke test|[0-9]+ responses?' | sort -u

section "GET /unknown (404)"
curl -s -o /dev/null -w 'HTTP %{http_code}\n' "$BASE/00000000-0000-0000-0000-000000000000"

# --- second form: radio / checkbox (multi-value) / number ---
UUID2=3d9f5f2c-6a1e-4f0a-9b7c-2e5d8c1a4b06

section "GET /$UUID2 (radio/checkbox/number render)"
curl -s "$BASE/$UUID2" | grep -oE 'type="radio"|type="checkbox"|type="number"|How satisfied|Which features' | sort -u

section "POST /$UUID2 (radio + multi checkbox + number)"
curl -s -X POST "$BASE/$UUID2" \
  --data-urlencode "name=Grace" \
  --data-urlencode "email=grace@example.com" \
  --data-urlencode "satisfaction=Happy" \
  --data-urlencode "features=JSON form builder" \
  --data-urlencode "features=Dark theme" \
  --data-urlencode "nps=9" \
  -o /dev/null -w 'HTTP %{http_code} -> %{redirect_url}\n'

section "GET /$UUID2?admin=true (multi-value join + number)"
curl -s -b "$COOKIE_JAR" "$BASE/$UUID2?admin=true" \
  | grep -oE 'Grace|grace@example.com|Happy|JSON form builder, Dark theme|2 responses?' | sort -u

section "server log"
cat "$LOG"
