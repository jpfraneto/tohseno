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
  ""|*/*|*\?*|*\#*|*@*|*\[*|*\]*) fail "use a bare DNS origin without credentials, path, query, or fragment" ;;
esac

case "$authority" in
  *:*)
    host=${authority%:*}
    port=${authority##*:}
    case "$host" in *:*) fail "use a fully qualified DNS hostname, not an IP literal" ;; esac
    case "$port" in ""|*[!0-9]*) fail "the optional port must be numeric" ;; esac
    [ "$port" -ge 1 ] 2>/dev/null && [ "$port" -le 65535 ] 2>/dev/null ||
      fail "the optional port must be between 1 and 65535"
    ;;
  *) host=$authority ;;
esac
lower_host=$(printf '%s' "$host" | tr '[:upper:]' '[:lower:]')
case "$lower_host" in
  localhost|localhost.|*.localhost|*.localhost.|127.*|0.*) fail "localhost and loopback endpoints are development-only" ;;
  trycloudflare.com|trycloudflare.com.|*.trycloudflare.com|*.trycloudflare.com.) fail "Cloudflare Quick Tunnels are development-only" ;;
esac

case "$lower_host" in
  ""|.*|*.|*..*|*[!a-z0-9.-]*) fail "use a stable fully qualified DNS hostname" ;;
esac

old_ifs=$IFS
IFS=.
set -- $lower_host
IFS=$old_ifs
[ "$#" -ge 2 ] || fail "use a stable fully qualified DNS hostname"
for label in "$@"; do
  case "$label" in ""|-*|*-) fail "use a stable fully qualified DNS hostname" ;; esac
  [ "${#label}" -le 63 ] || fail "DNS labels must be at most 63 characters"
done
top_level=
for label in "$@"; do top_level=$label; done
case "$top_level" in
  *[a-z]*) ;;
  *) fail "use a stable fully qualified DNS hostname, not an IP literal" ;;
esac
