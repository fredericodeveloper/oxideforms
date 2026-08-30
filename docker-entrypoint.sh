#!/bin/sh
set -e

# When started as root (the image default), make the mounted database
# directory owned by the app user, then drop privileges. If the container was
# explicitly started as a non-root user (e.g. -u 1000:1000), run as-is.
#
# /forms is only read by the container, so its host-owned files need no
# ownership change (they just need to be readable, which 644 already is).
if [ "$(id -u)" = "0" ]; then
  chown -R oxideforms:oxideforms /data
  exec setpriv --reuid=oxideforms --regid=oxideforms --clear-groups "$@"
fi

exec "$@"
