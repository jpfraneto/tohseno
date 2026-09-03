#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd -P)"
app="${1:-$repository_root/dist/native/Tohseno.app}"
output="${2:-$repository_root/dist/native/Tohseno-1.2.0-rc.11.dmg}"
[ -d "$app/Contents" ] && [ ! -L "$app" ] || { printf '%s\n' 'create-dmg.sh: app bundle is missing or unsafe.' >&2; exit 1; }
case "$output" in "$repository_root"/dist/*.dmg|"$repository_root"/dist/*/*.dmg) ;; *) printf '%s\n' 'create-dmg.sh: output must be below repository dist/.' >&2; exit 2 ;; esac
stage="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-dmg.XXXXXX")"
payload="$stage/payload"
read_write="$stage/Tohseno-layout.dmg"
device=""
cleanup() {
  if [ -n "$device" ]; then
    hdiutil detach "$device" -quiet >/dev/null 2>&1 || true
  fi
  case "$stage" in "${TMPDIR:-/tmp}"/tohseno-dmg.*) rm -rf "$stage" ;; esac
}
trap cleanup EXIT HUP INT TERM
mkdir "$payload"
cp -R "$app" "$payload/Tohseno.app"
ln -s /Applications "$payload/Applications"
mkdir "$payload/.background"
sips -s format png "$repository_root/macos/Tohseno/Packaging/dmg-background.svg" \
  --out "$payload/.background/background.png" >/dev/null
mkdir -p "$(dirname -- "$output")"
hdiutil create -fs HFS+ -volname Tohseno -srcfolder "$payload" -format UDRW -ov "$read_write" >/dev/null
attach_output="$(hdiutil attach -readwrite -noverify -noautoopen "$read_write")"
device="$(printf '%s\n' "$attach_output" | awk '/^\/dev\// { print $1; exit }')"
mount_point="$(printf '%s\n' "$attach_output" | awk -F '\t' '/\/Volumes\// { print $NF; exit }')"
[ -n "$device" ] && [ "$mount_point" = "/Volumes/Tohseno" ] || {
  printf '%s\n' 'create-dmg.sh: the writable Finder layout volume did not mount safely.' >&2
  exit 1
}
osascript <<'APPLESCRIPT'
tell application "Finder"
  tell disk "Tohseno"
    open
    set current view of container window to icon view
    tell container window
      set toolbar visible to false
      set statusbar visible to false
      set pathbar visible to false
      set bounds to {120, 120, 760, 500}
    end tell
    set view_options to the icon view options of container window
    set icon size of view_options to 92
    set text size of view_options to 13
    set background picture of view_options to file ".background:background.png"
    set position of item "Tohseno.app" to {145, 214}
    set position of item "Applications" to {495, 214}
    update without registering applications
    delay 2
    close
  end tell
end tell
APPLESCRIPT
sync
hdiutil detach "$device" -quiet
device=""
hdiutil convert "$read_write" -format UDZO -imagekey zlib-level=9 -ov -o "$output" >/dev/null
shasum -a 256 "$output" >"$output.sha256"
printf 'assembled disk image: %s\n' "$output"
