#!/bin/sh
set -eu

# A mounted volume hides image-layer ownership. Initialize the only writable
# application path as root, then permanently drop privileges before Bun starts.
if [ "$(id -u)" = "0" ]; then
  mkdir -p /data
  chown -R bun:bun /data
  export HOME=/home/bun
  exec su-exec bun "$@"
fi

exec "$@"
