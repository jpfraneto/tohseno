#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd -P)"
app="${1:-$repository_root/dist/native/Tohseno.app}"
output="${2:-$repository_root/dist/native/TOHSENO-1.0.2.dmg}"
[ -d "$app/Contents" ] && [ ! -L "$app" ] || { printf '%s\n' 'create-dmg.sh: app bundle is missing or unsafe.' >&2; exit 1; }
case "$output" in "$repository_root"/dist/*.dmg|"$repository_root"/dist/*/*.dmg) ;; *) printf '%s\n' 'create-dmg.sh: output must be below repository dist/.' >&2; exit 2 ;; esac
stage="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-dmg.XXXXXX")"
trap 'case "$stage" in "${TMPDIR:-/tmp}"/tohseno-dmg.*) rm -rf "$stage" ;; esac' EXIT HUP INT TERM
cp -R "$app" "$stage/TOHSENO.app"
ln -s /Applications "$stage/Applications"
mkdir -p "$(dirname -- "$output")"
hdiutil create -fs HFS+ -volname TOHSENO -srcfolder "$stage" -format UDZO -ov "$output"
shasum -a 256 "$output" >"$output.sha256"
printf 'assembled disk image: %s\n' "$output"
