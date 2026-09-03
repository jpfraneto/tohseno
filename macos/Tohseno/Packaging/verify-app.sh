#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd -P)"
app="${1:-$repository_root/dist/native/Tohseno.app}"
level="${2:-unsigned}"
[ -d "$app/Contents" ] && [ ! -L "$app" ] || { printf '%s\n' 'verify-app.sh: app bundle is missing or unsafe.' >&2; exit 1; }
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")" = com.tohseno.mac
test "$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$app/Contents/Info.plist")" = 14.0
team_id="$(sed -n '1p' "$app/Contents/Resources/native-client-requirement.txt")"
if [ "$team_id" != TEAMIDPLACEHOLDER ] && ! printf '%s\n' "$team_id" | grep -Eq '^[A-Z0-9]{10}$'; then
  printf '%s\n' 'verify-app.sh: native client Team ID is invalid.' >&2
  exit 1
fi
for binary in "$app/Contents/MacOS/TohsenoMacApp" "$app/Contents/Helpers/tohseno" "$app/Contents/Resources/FactoryRelease/bin/tohseno" "$app/Contents/Resources/FactoryRelease/bin/tohseno-apple-identity"; do
  test -x "$binary" && test ! -L "$binary"
  architectures="$(lipo -archs "$binary")"
  echo "$architectures" | grep -qw arm64
  echo "$architectures" | grep -qw x86_64
done
python3 "$repository_root/scripts/release-package-integrity.py" verify-manifest \
  --root "$app/Contents/Resources/FactoryRelease" --manifest-name FILES.sha256
test -f "$app/Contents/Resources/FactoryRelease/share/apple-identity/Package.swift"
test -f "$app/Contents/Resources/FactoryRelease/share/apple-identity/Sources/TohsenoAppleIdentity/AppleIdentity.swift"
test -f "$app/Contents/Resources/FactoryRelease/share/sdk/apple/TohsenoWorkshopKit/Package.swift"
test -f "$app/Contents/Resources/FactoryRelease/share/sdk/apple/TohsenoWorkshopKit/Sources/TohsenoWorkshopKit/WorkshopRuntime.swift"
if rg -a -n --glob '!FILES.sha256' 'sk_live_[A-Za-z0-9]|whsec_[A-Za-z0-9]|bk_[A-Za-z0-9]{8}|TOHSENO_OPERATOR_TOKEN=' "$app/Contents"; then
  printf '%s\n' 'verify-app.sh: a forbidden managed-service secret pattern is present.' >&2
  exit 1
fi
case "$level" in
  unsigned) ;;
  signed)
    /usr/bin/codesign --verify --deep --strict --verbose=2 "$app"
    /usr/bin/codesign -d --entitlements :- "$app" 2>&1 | grep -Fq '<dict>'
    /usr/sbin/spctl --assess --type execute --verbose=2 "$app"
    ;;
  notarized)
    /usr/bin/codesign --verify --deep --strict --verbose=2 "$app"
    /usr/sbin/spctl --assess --type execute --verbose=2 "$app"
    xcrun stapler validate "$app"
    ;;
  *) printf '%s\n' 'Usage: verify-app.sh [Tohseno.app] [unsigned|signed|notarized]' >&2; exit 2 ;;
esac
printf 'verified native application bundle: %s (%s)\n' "$app" "$level"
