#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
export HOST=127.0.0.1 PORT=3112 DB_PATH=/tmp/forms_dbg_$$.db FORMS_DIR=forms ADMIN_PASSWORD=testpass123 RUST_LOG=info
rm -f "$DB_PATH"
./target/debug/forms >/tmp/forms_dbg_$$.log 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null' EXIT
for _ in $(seq 1 50); do curl -sf http://127.0.0.1:3112/healthz >/dev/null 2>&1 && break; sleep 0.2; done

echo "--- index: code/size ---"
curl -s -o /tmp/forms_dbg_idx.html -w 'code=%{http_code} size=%{size_download}\n' http://127.0.0.1:3112/
echo "--- first 500 bytes of index ---"
head -c 500 /tmp/forms_dbg_idx.html
echo
echo "--- grep -c Contact (index) ---"
grep -c "Contact" /tmp/forms_dbg_idx.html
echo "--- tail log ---"
tail -6 /tmp/forms_dbg_$$.log
