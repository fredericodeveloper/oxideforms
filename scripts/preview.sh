#!/usr/bin/env bash
#
# Boots the server, seeds a couple of submissions, and writes the rendered HTML
# of a few pages into `.preview/` so you can eyeball the design without a browser:
#
#   bash scripts/preview.sh
#   open .preview/index.html        (or `cat .preview/submissions_contact.html`)
#
set -uo pipefail
cd "$(dirname "$0")/.."

export HOST=127.0.0.1
export PORT=3113
export DB_PATH=/tmp/forms_preview_$$.db
export FORMS_DIR=forms
export ADMIN_PASSWORD=preview
export RUST_LOG=info

BASE="http://127.0.0.1:$PORT"
C=8f14e45f-ceea-467f-8d3d-1890c9784b79
F=3d9f5f2c-6a1e-4f0a-9b7c-2e5d8c1a4b06
LOG=/tmp/forms_preview_$$.log
OUT=.preview
rm -f "$DB_PATH"
mkdir -p "$OUT"

./target/debug/forms >"$LOG" 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null' EXIT
for _ in $(seq 1 50); do curl -sf "$BASE/healthz" >/dev/null 2>&1 && break; sleep 0.2; done

# seed a submission for each form
curl -s -X POST "$BASE/$C" \
  --data-urlencode "name=Ada Lovelace" --data-urlencode "email=ada@example.com" \
  --data-urlencode "topic=Support" --data-urlencode "message=The dark theme looks great!" >/dev/null
curl -s -X POST "$BASE/$F" \
  --data-urlencode "name=Grace Hopper" --data-urlencode "email=grace@example.com" \
  --data-urlencode "satisfaction=Very happy" \
  --data-urlencode "features=JSON form builder" --data-urlencode "features=Dark theme" \
  --data-urlencode "nps=10" --data-urlencode "comments=Ship it." >/dev/null

jar=/tmp/forms_preview_$$.cookies
curl -s -c "$jar" -X POST "$BASE/$C/admin/auth" --data-urlencode "password=preview" >/dev/null

save() { curl -s -b "$jar" "$1" > "$OUT/$2"; }
save "$BASE/"                                index.html
save "$BASE/$C"                              form_contact.html
save "$BASE/$C?admin=true"                   submissions_contact.html
save "$BASE/$F"                              form_feedback.html
save "$BASE/$F?admin=true"                   submissions_feedback.html
save "$BASE/00000000-0000-0000-0000-000000000000" 404.html

echo "wrote:"; ls -1 "$OUT"
