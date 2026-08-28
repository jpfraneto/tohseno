#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd -P)"
app="${1:-$repository_root/dist/native/Tohseno.app}"
mode="${2:-sign-only}"
identity="${TOHSENO_DEVELOPER_ID_APPLICATION:-}"
team="${TOHSENO_DEVELOPER_TEAM_ID:-}"
[ -n "$identity" ] || { printf '%s\n' 'sign-and-notarize.sh: TOHSENO_DEVELOPER_ID_APPLICATION is required.' >&2; exit 2; }
case "$team" in [A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9][A-Z0-9]) ;;
  *) printf '%s\n' 'sign-and-notarize.sh: TOHSENO_DEVELOPER_TEAM_ID must be the exact 10-character Team ID.' >&2; exit 2 ;;
esac
[ -d "$app/Contents" ] && [ ! -L "$app" ] || { printf '%s\n' 'sign-and-notarize.sh: app bundle is missing or unsafe.' >&2; exit 1; }

printf '%s\n' "$team" \
  >"$app/Contents/Resources/native-client-requirement.txt"
sign() {
  entitlements="$1"
  target="$2"
  /usr/bin/codesign --force --sign "$identity" --options runtime --timestamp \
    --entitlements "$entitlements" "$target"
}
sign "$repository_root/macos/Tohseno/Packaging/Helper.entitlements" \
  "$app/Contents/Resources/FactoryRelease/bin/tohseno"
sign "$repository_root/macos/Tohseno/Packaging/Helper.entitlements" \
  "$app/Contents/Resources/FactoryRelease/bin/tohseno-apple-identity"
python3 "$repository_root/scripts/release-package-integrity.py" write-manifest \
  --root "$app/Contents/Resources/FactoryRelease" --manifest-name FILES.sha256
sign "$repository_root/macos/Tohseno/Packaging/Helper.entitlements" "$app/Contents/Helpers/tohseno"
sign "$repository_root/macos/Tohseno/Packaging/Tohseno.entitlements" "$app"
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app"

case "$mode" in
  sign-only) ;;
  notarize)
    profile="${TOHSENO_NOTARY_KEYCHAIN_PROFILE:-}"
    [ -n "$profile" ] || { printf '%s\n' 'sign-and-notarize.sh: TOHSENO_NOTARY_KEYCHAIN_PROFILE is required for notarize.' >&2; exit 2; }
    archive="$(mktemp "${TMPDIR:-/tmp}/Tohseno-notary.XXXXXX.zip")"
    trap 'rm -f "$archive"' EXIT HUP INT TERM
    /usr/bin/ditto -c -k --keepParent "$app" "$archive"
    xcrun notarytool submit "$archive" --keychain-profile "$profile" --wait
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
    ;;
  *) printf '%s\n' 'Usage: sign-and-notarize.sh [Tohseno.app] [sign-only|notarize]' >&2; exit 2 ;;
esac
printf 'native application signing step complete: %s (%s)\n' "$app" "$mode"
