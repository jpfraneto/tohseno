#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd -P)"
package_root="$repository_root/macos/Tohseno"
output="${1:-$repository_root/dist/native/Tohseno.app}"
case "$output" in
  "$repository_root"/dist/*/Tohseno.app | "$repository_root"/dist/Tohseno.app) ;;
  *) printf '%s\n' "build-app.sh: output must be a Tohseno.app below repository dist/." >&2; exit 2 ;;
esac

for tool in rustup swift lipo sips iconutil python3; do
  command -v "$tool" >/dev/null 2>&1 || { printf 'build-app.sh: missing %s\n' "$tool" >&2; exit 1; }
done
release_toolchain="${TOHSENO_RUST_TOOLCHAIN:-1.88.0}"
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed --toolchain "$release_toolchain" | grep -Fx "$target" >/dev/null || {
    printf 'build-app.sh: Rust target %s is not installed.\n' "$target" >&2
    exit 1
  }
done

release_rustc="$(rustup which --toolchain "$release_toolchain" rustc)"
release_rustdoc="$(rustup which --toolchain "$release_toolchain" rustdoc)"
RUSTC="$release_rustc" RUSTDOC="$release_rustdoc" \
  rustup run "$release_toolchain" cargo build --manifest-path "$repository_root/Cargo.toml" --release --locked \
  --target aarch64-apple-darwin --target x86_64-apple-darwin --bin tohseno
swift build --package-path "$package_root" -c release --arch arm64 --arch x86_64
swift_bin="$(swift build --package-path "$package_root" -c release --arch arm64 --arch x86_64 --show-bin-path)/TohsenoMacApp"
[ -x "$swift_bin" ] || { printf '%s\n' 'build-app.sh: Swift app executable was not produced.' >&2; exit 1; }
swift build --package-path "$repository_root/apple-identity" -c release --arch arm64 --arch x86_64
identity_bin="$(swift build --package-path "$repository_root/apple-identity" -c release --arch arm64 --arch x86_64 --show-bin-path)/tohseno-apple-identity"
[ -x "$identity_bin" ] || { printf '%s\n' 'build-app.sh: Apple identity helper was not produced.' >&2; exit 1; }

stage="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-native-app.XXXXXX")"
cleanup() {
  case "$stage" in "${TMPDIR:-/tmp}"/tohseno-native-app.*) rm -rf "$stage" ;; esac
}
trap cleanup EXIT HUP INT TERM
app="$stage/Tohseno.app"
contents="$app/Contents"
release="$contents/Resources/FactoryRelease"
mkdir -p "$contents/MacOS" "$contents/Helpers" "$contents/Resources" \
  "$release/bin" "$release/share/billing" "$release/share/protocol" \
  "$release/share/fascia" "$release/share/readiness/apple" "$release/share/studio"
cp "$package_root/Packaging/Info.plist" "$contents/Info.plist"
cp "$repository_root/website/apps/site/public/logo.svg" "$contents/Resources/TohsenoLogo.svg"
cp "$swift_bin" "$contents/MacOS/TohsenoMacApp"
lipo -create \
  "$repository_root/target/aarch64-apple-darwin/release/tohseno" \
  "$repository_root/target/x86_64-apple-darwin/release/tohseno" \
  -output "$contents/Helpers/tohseno"
cp "$contents/Helpers/tohseno" "$release/bin/tohseno"
cp "$identity_bin" "$release/bin/tohseno-apple-identity"
chmod 0755 "$contents/MacOS/TohsenoMacApp" "$contents/Helpers/tohseno" \
  "$release/bin/tohseno" "$release/bin/tohseno-apple-identity"

cp -R "$repository_root/protocol/schemas" "$release/share/protocol/schemas"
cp -R "$repository_root/protocol/test-vectors" "$release/share/protocol/test-vectors"
cp -R "$repository_root/fascia/apple" "$release/share/fascia/apple"
find "$release/share/fascia/apple" -type d \( -name .build -o -name .swiftpm \) -prune -exec rm -rf {} +
cp -R "$repository_root/studio/." "$release/share/studio/"
cp -R "$repository_root/engine/fixtures/hello-world/." "$release/share/readiness/apple/"
mkdir -p "$release/share/sdk/apple" "$release/share/companion/apple" "$release/share/companion/test-vectors"
cp -R "$repository_root/sdk/apple/TohsenoCompanionKit" "$release/share/sdk/apple/TohsenoCompanionKit"
cp -R "$repository_root/companion/apple/TohsenoCompanion" "$release/share/companion/apple/TohsenoCompanion"
cp -R "$repository_root/companion/test-vectors/." "$release/share/companion/test-vectors/"
find "$release/share/sdk" "$release/share/companion" -type d \( -name .build -o -name .swiftpm \) -prune -exec rm -rf {} +
if [ -f "$repository_root/billing/verification-key-p256.txt" ]; then
  cp "$repository_root/billing/verification-key-p256.txt" "$release/share/billing/verification-key-p256.txt"
fi
python3 "$repository_root/scripts/release-package-integrity.py" write-manifest \
  --root "$release" --manifest-name FILES.sha256

iconset="$stage/AppIcon.iconset"
mkdir "$iconset"
for specification in '16 16x16' '32 16x16@2x' '32 32x32' '64 32x32@2x' '128 128x128' '256 128x128@2x' '256 256x256' '512 256x256@2x' '512 512x512' '1024 512x512@2x'; do
  pixels="${specification%% *}"
  name="${specification#* }"
  sips -z "$pixels" "$pixels" "$repository_root/brand/logos/tohseno-app-icon-1024.png" \
    --out "$iconset/icon_${name}.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$contents/Resources/AppIcon.icns"

team="${TOHSENO_DEVELOPER_TEAM_ID:-TEAMIDPLACEHOLDER}"
if [ "$team" != TEAMIDPLACEHOLDER ] && ! printf '%s\n' "$team" | grep -Eq '^[A-Z0-9]{10}$'; then
  printf '%s\n' 'build-app.sh: TOHSENO_DEVELOPER_TEAM_ID must be the exact 10-character Team ID.' >&2
  exit 1
fi
printf '%s\n' "$team" \
  >"$contents/Resources/native-client-requirement.txt"
chmod 0644 "$contents/Resources/native-client-requirement.txt"

mkdir -p "$(dirname -- "$output")"
if [ -e "$output" ] || [ -L "$output" ]; then
  rm -rf "$output"
fi
mv "$app" "$output"
printf 'assembled unsigned native application: %s\n' "$output"
