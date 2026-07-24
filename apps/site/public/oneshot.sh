#!/usr/bin/env bash
#
# TOHSENO pinned legacy entry point
#
#   curl -fsSL https://tohseno.com/oneshot.sh | bash
#
# The original script created workspaces directly from a shared rails checkout.
# That competing creator is retired. This endpoint now downloads only the
# canonical managed installer from the exact released commit below, verifies
# the installer's bytes, and forwards all arguments to it.

set -euo pipefail
umask 077

PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

# This must remain the direct parent of the commit serving this script.
TOHSENO_PIN="48bada35f885216c8c2bf3ab4d51d0c935e2e01e"
PINNED_INSTALLER_SHA256="06efde2b0a9da6e2b7bac56119b84b0f5288d40e41dbe5a6d384246336be59fb"
PINNED_INSTALLER_URL="https://raw.githubusercontent.com/jpfraneto/tohseno/${TOHSENO_PIN}/apps/site/public/install.sh"
ONESHOT_VERSION="0.5.0"

temporary_directory=""
installer_path=""

say() { printf '%s\n' "$*"; }
die() { printf 'tohseno oneshot: %s\n' "$*" >&2; exit 1; }

cleanup() {
  if [ -n "$installer_path" ] && [ -f "$installer_path" ] &&
    [ ! -L "$installer_path" ]; then
    rm -f -- "$installer_path"
  fi
  if [ -n "$temporary_directory" ] && [ -d "$temporary_directory" ] &&
    [ ! -L "$temporary_directory" ]; then
    rmdir -- "$temporary_directory" 2>/dev/null || true
  fi
}

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

usage() {
  say "TOHSENO oneshot ${ONESHOT_VERSION}"
  say ""
  say "The legacy workspace creator is retired."
  say "This thin entry point runs the canonical TOHSENO 0.3.1 installer"
  say "from pinned released commit ${TOHSENO_PIN} after SHA-256 verification."
  say ""
  say "Usage: oneshot.sh [installer options]"
  say "Installer options include --help, --non-interactive, --no-modify-path,"
  say "--without-cloudflared, and --dry-run."
}

hash_file() {
  local path=$1
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  else
    die "shasum or sha256sum is required to verify the pinned installer"
  fi
}

main() {
  case "${1:-}" in
    --help|-h)
      usage
      return 0
      ;;
    --version)
      say "$ONESHOT_VERSION"
      return 0
      ;;
  esac

  command -v curl >/dev/null 2>&1 ||
    die "curl is required to download the pinned installer"
  command -v mktemp >/dev/null 2>&1 ||
    die "mktemp is required to stage the pinned installer"

  temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-oneshot.XXXXXX")"
  installer_path="${temporary_directory}/install.sh"
  curl --fail --silent --show-error --location \
    --proto '=https' --proto-redir '=https' --tlsv1.2 \
    --connect-timeout 15 --max-time 120 --max-filesize 262144 \
    "$PINNED_INSTALLER_URL" \
    --output "$installer_path"
  chmod 600 "$installer_path"
  [ "$(wc -c < "$installer_path" | tr -d '[:space:]')" -le 262144 ] ||
    die "the pinned installer exceeds its size limit"

  local actual_sha256
  actual_sha256="$(hash_file "$installer_path")"
  [ "$actual_sha256" = "$PINNED_INSTALLER_SHA256" ] ||
    die "checksum mismatch for the pinned installer"

  /bin/sh "$installer_path" "$@"
}

main "$@"
