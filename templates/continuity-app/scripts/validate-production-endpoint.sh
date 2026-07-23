#!/bin/sh
set -eu

configuration="${CONFIGURATION:-${1:-}}"
endpoint="${TOHSENO_API_BASE_URL:-${API_BASE_URL:-${2:-}}}"

if [ "$configuration" != "Release" ]; then
  exit 0
fi

fail() {
  printf 'error: TOHSENO production API endpoint: %s\n' "$1" >&2
  exit 1
}

[ -n "$endpoint" ] || fail "Config/Production.xcconfig must define a stable HTTPS bare origin"
case "$endpoint" in
  https://*) ;;
  *) fail "Release builds require https://" ;;
esac

authority=${endpoint#https://}
case "$authority" in
  ""|*/*|*\?*|*\#*|*@*) fail "use a bare origin without credentials, path, query, or fragment" ;;
esac

host=${authority%%:*}
lower_host=$(printf '%s' "$host" | tr '[:upper:]' '[:lower:]')
case "$lower_host" in
  localhost|*.localhost|127.*|0.0.0.0|::1|\[::1\]) fail "localhost and loopback endpoints are development-only" ;;
  trycloudflare.com|*.trycloudflare.com) fail "Cloudflare Quick Tunnels are development-only" ;;
esac

case "$lower_host" in
  *.*) ;;
  *) fail "use a stable fully qualified HTTPS hostname" ;;
esac
