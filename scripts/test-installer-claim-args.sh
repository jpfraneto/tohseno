#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
installer="$repository_root/oneshot/oneshot.sh"

help="$(sh "$installer" --help)"
printf '%s\n' "$help" | grep -Fq -- "--claim TOKEN"
printf '%s\n' "$help" | grep -Fq -- "--no-studio"

sensitive="ti1.not-a-valid-private-token"
log="$(mktemp "${TMPDIR:-/tmp}/tohseno-installer-claim.XXXXXX")"
cleanup() {
  case "$log" in
    "${TMPDIR:-/tmp}"/tohseno-installer-claim.*) rm -f "$log" ;;
  esac
}
trap cleanup EXIT HUP INT TERM
if sh "$installer" --claim "$sensitive" >"$log" 2>&1; then
  printf '%s\n' "claim parser accepted a malformed token" >&2
  exit 1
fi
grep -Fq "claim token is malformed or uses an unsupported version" "$log"
if grep -Fq "$sensitive" "$log"; then
  printf '%s\n' "claim parser echoed a sensitive token" >&2
  exit 1
fi

grep -Fq 'intent claim --stdin' "$installer"
if grep -Fq 'intent claim "$claim_token"' "$installer"; then
  printf '%s\n' "installer passes the claim token in a nested process argument" >&2
  exit 1
fi
grep -Fq 'version="v0.8.3"' "$installer"
grep -Fq 'version="v0.8.2"' "$repository_root/website/apps/site/public/oneshot.sh"
if grep -Fq -- '--claim)' "$repository_root/website/apps/site/public/oneshot.sh"; then
  printf '%s\n' "published thin installer was repointed before release" >&2
  exit 1
fi

printf '%s\n' "Claim installer argument and public-pin gates passed."
