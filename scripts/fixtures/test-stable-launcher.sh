#!/bin/sh
set -eu
umask 077

script_name="test-stable-launcher.sh"
launcher_directory="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
install_root="$(CDPATH= cd -- "$launcher_directory/.." && pwd -P)"

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

[ "${TOHSENO_INSTALL_ROOT:-}" = "$install_root" ] ||
  fail "the launcher escaped its isolated install root"
current="$install_root/current"
[ -L "$current" ] || fail "the current release pointer is not a symlink"
current_target="$(readlink "$current")"
case "$current_target" in
  releases/*)
    release_name="${current_target#releases/}"
    case "$release_name" in
      ""|*/*|.|..) fail "the current release target is non-canonical" ;;
    esac
    ;;
  *) fail "the current release target escaped releases" ;;
esac
release_root="$(CDPATH= cd -- "$current" && pwd -P)" ||
  fail "the current release is unavailable"
release_parent="$(CDPATH= cd -- "$release_root/.." && pwd -P)"
[ "$release_parent" = "$install_root/releases" ] ||
  fail "the current release resolved outside releases"
binary="$release_root/bin/tohseno"
[ -f "$binary" ] && [ ! -L "$binary" ] && [ -x "$binary" ] ||
  fail "the selected release binary is unsafe"

if [ -n "${TOHSENO_TEST_LAUNCHER_LOG:-}" ]; then
  launcher_log="$TOHSENO_TEST_LAUNCHER_LOG"
  case "$launcher_log" in
    "$install_root"/*) ;;
    *) fail "the launcher evidence path escaped the isolated install root" ;;
  esac
  if [ -L "$launcher_log" ] || { [ -e "$launcher_log" ] && [ ! -f "$launcher_log" ]; }; then
    fail "the launcher evidence path is unsafe"
  fi
  printf '%s\n' "$release_name" >>"$launcher_log"
fi

exec "$binary" "$@"
