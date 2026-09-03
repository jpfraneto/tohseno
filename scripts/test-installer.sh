#!/bin/sh
set -eu

if [ "$#" -ne 0 ]; then
  printf '%s\n' "usage: scripts/test-installer.sh" >&2
  exit 2
fi

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-installer-test.XXXXXX")"

cleanup() {
  case "$temporary_root" in
    "$temporary_parent"/tohseno-installer-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf "$temporary_root"
      fi
      ;;
    *) printf '%s\n' "test-installer.sh: refusing unsafe cleanup." >&2 ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

case "$(uname -m)" in
  arm64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *) printf '%s\n' "test-installer.sh: unsupported test architecture." >&2; exit 2 ;;
esac

fixture="$temporary_root/fixture"
package="$temporary_root/package/$target"
fake_bin="$temporary_root/fake-bin"
test_home="$temporary_root/home"
installer_tmp="$temporary_root/tmp"
curl_log="$temporary_root/curl.log"
service_log="$temporary_root/service.log"
mkdir -p \
  "$fixture" "$package/bin" "$package/share/studio" \
  "$package/share/apple-identity/Sources/TohsenoAppleIdentity" \
  "$package/share/sdk/apple/TohsenoCompanionKit" \
  "$package/share/sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit" \
  "$package/share/sdk/apple/TohsenoWorkshopKit" \
  "$package/share/sdk/apple/TohsenoWorkshopKit/Sources/TohsenoWorkshopKit" \
  "$package/share/companion/test-vectors" "$package/share/protocol/schemas" \
  "$package/share/fascia/apple" "$package/share/genesis" "$fake_bin" \
  "$test_home" "$installer_tmp"

write_cli() {
  destination="$1"
  behavior="$2"
  cat >"$destination" <<'FAKE_TOHSENO'
#!/bin/sh
set -eu
if [ "${1:-}" = "--version" ]; then
  printf '%s\n' "tohseno 1.1.0"
  exit 0
fi
if [ "${1:-}" = "--json" ]; then shift; fi
case "${1:-}:${2:-}" in
  service:install)
    if grep -Fqx fail-health "$(dirname -- "$0")/service-behavior"; then
      exit 23
    fi
    launch_agent="$HOME/Library/LaunchAgents/com.tohseno.workspace-service.plist"
    if [ -L "$launch_agent" ] ||
      { [ -e "$launch_agent" ] && [ ! -f "$launch_agent" ]; }; then
      exit 24
    fi
    if [ -f "$launch_agent" ] &&
      ! grep -Fq 'TOHSENO_WORKSPACE_SERVICE_PLIST_V1' "$launch_agent"; then
      exit 25
    fi
    mkdir -p "$HOME/Library/LaunchAgents" "$HOME/.tohseno/service/devices"
    printf '%s\n' '<!-- TOHSENO_WORKSPACE_SERVICE_PLIST_V1 -->' \
      >"$launch_agent"
    printf '%s\n' '{"schema":"tohseno.local-workspace-runtime/1"}' \
      >"$HOME/.tohseno/service/runtime.json"
    printf '%s\n' install >>"${TOHSENO_TEST_SERVICE_LOG:?}"
    printf '%s\n' '{"schema":"tohseno.service-status/1","healthy":true,"service_version":"1.1.0","origin":"http://127.0.0.1:19466","workspace_id":"workspace_fixture"}'
    ;;
  service:status)
    if grep -Fqx unhealthy-status "$(dirname -- "$0")/service-behavior"; then
      printf '%s\n' '{"schema":"tohseno.service-status/1","healthy":false,"service_version":"1.1.0","origin":"http://127.0.0.1:19466","workspace_id":"workspace_fixture"}'
    else
      printf '%s\n' '{"schema":"tohseno.service-status/1","healthy":true,"service_version":"1.1.0","origin":"http://127.0.0.1:19466","workspace_id":"workspace_fixture"}'
    fi
    ;;
  service:uninstall)
    launch_agent="$HOME/Library/LaunchAgents/com.tohseno.workspace-service.plist"
    if [ -L "$launch_agent" ] ||
      { [ -e "$launch_agent" ] && [ ! -f "$launch_agent" ]; }; then
      exit 24
    fi
    if [ -f "$launch_agent" ] &&
      ! grep -Fq 'TOHSENO_WORKSPACE_SERVICE_PLIST_V1' "$launch_agent"; then
      exit 25
    fi
    rm -f "$launch_agent"
    printf '%s\n' uninstall >>"${TOHSENO_TEST_SERVICE_LOG:?}"
    ;;
  studio:)
    printf '%s\n' studio >>"${TOHSENO_TEST_SERVICE_LOG:?}"
    ;;
  *) exit 2 ;;
esac
FAKE_TOHSENO
  chmod 0755 "$destination"
  printf '%s\n' "$behavior" >"$(dirname -- "$destination")/service-behavior"
}

write_helper() {
  destination="$1"
  cat >"$destination" <<'FAKE_HELPER'
#!/bin/sh
set -eu
[ "${1:-}" = "--version" ] || exit 2
printf '%s\n' "tohseno-apple-identity 1.1.0"
FAKE_HELPER
  chmod 0755 "$destination"
}

write_cli "$package/bin/tohseno" healthy
write_helper "$package/bin/tohseno-apple-identity"
printf '%s\n' '<html>Studio</html>' >"$package/share/studio/index.html"
printf '%s\n' '// Studio' >"$package/share/studio/app.js"
printf '%s\n' '/* Studio */' >"$package/share/studio/style.css"
printf '%s\n' '// CompanionKit' \
  >"$package/share/sdk/apple/TohsenoCompanionKit/Package.swift"
printf '%s\n' '1.1.0' \
  >"$package/share/sdk/apple/TohsenoCompanionKit/VERSION"
printf '%s\n' 'license' \
  >"$package/share/sdk/apple/TohsenoCompanionKit/LICENSE"
printf '%s\n' '// CompanionKit client fixture' \
  >"$package/share/sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift"
printf '%s\n' '// swift-tools-version: 6.0' \
  >"$package/share/sdk/apple/TohsenoWorkshopKit/Package.swift"
printf '%s\n' '// Workshop runtime fixture' \
  >"$package/share/sdk/apple/TohsenoWorkshopKit/Sources/TohsenoWorkshopKit/WorkshopRuntime.swift"
printf '%s\n' '// swift-tools-version: 6.0' \
  >"$package/share/apple-identity/Package.swift"
printf '%s\n' '// Apple identity fixture' \
  >"$package/share/apple-identity/Sources/TohsenoAppleIdentity/AppleIdentity.swift"
printf '%s\n' '{"schema":"tohseno.companion-test-vectors/1"}' \
  >"$package/share/companion/test-vectors/companion-v1.json"
printf '%s\n' '{}' >"$package/share/protocol/schemas/common.schema.json"
printf '%s\n' '{}' >"$package/share/fascia/apple/FASCIA.json"
printf '%s\n' '{}' >"$package/share/genesis/GENESIS.json"

write_release_manifest() {
  cat >"$package/RELEASE.json" <<EOF
{
  "schema": "tohseno.release/1",
  "version": "1.1.0",
  "codename": "COMPANION",
  "target": "$target",
  "source_commit": "1111111111111111111111111111111111111111",
  "source_state_sha256": "2222222222222222222222222222222222222222222222222222222222222222",
  "dirty": false,
  "channel": "stable",
  "prerelease": false
}
EOF
}

write_inner_manifest() {
  rm -f "$package/CHECKSUMS.sha256"
  (
    cd "$package"
    find . -type f ! -name CHECKSUMS.sha256 -print |
      sed 's#^\./##' |
      LC_ALL=C sort |
      while IFS= read -r file; do shasum -a 256 "$file"; done \
        >CHECKSUMS.sha256
  )
}

package_name="tohseno-release-$target.tar.gz"
refresh_outer_checksum() {
  (
    cd "$fixture"
    shasum -a 256 "$package_name" >SHA256SUMS
  )
}

publish_raw_fixture() {
  rm -f "$fixture/$package_name" "$fixture/SHA256SUMS"
  COPYFILE_DISABLE=1 tar -czf "$fixture/$package_name" \
    -C "$(dirname -- "$package")" "$target"
  refresh_outer_checksum
}

publish_fixture() {
  rm -f "$fixture/$package_name" "$fixture/SHA256SUMS"
  python3 "$repository_root/scripts/build-normalized-tar.py" \
    --source "$package" \
    --output "$fixture/$package_name" \
    --root-name "$target" \
    --mtime 1 \
    --manifest-name CHECKSUMS.sha256
  refresh_outer_checksum
}

write_release_manifest
write_inner_manifest
publish_fixture

cat >"$fake_bin/curl" <<'FAKE_CURL'
#!/bin/sh
set -eu
url=""
destination=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -fsSL) shift ;;
    --proto|--proto-redir|--max-filesize) shift 2 ;;
    --tlsv1.2) shift ;;
    -o) destination="$2"; shift 2 ;;
    http://*|https://*) url="$1"; shift ;;
    *) exit 2 ;;
  esac
done
[ -n "$url" ] && [ -n "$destination" ] || exit 2
artifact="${url##*/}"
case "$artifact" in
  tohseno-release-aarch64-apple-darwin.tar.gz|\
  tohseno-release-x86_64-apple-darwin.tar.gz|SHA256SUMS) ;;
  *) exit 2 ;;
esac
[ "$url" = "https://github.com/jpfraneto/tohseno/releases/download/v1.1.0/$artifact" ] || exit 2
printf '%s\n' "$url" >>"${TOHSENO_INSTALLER_CURL_LOG:?}"
cp "${TOHSENO_INSTALLER_FIXTURE_DIR:?}/$artifact" "$destination"
FAKE_CURL
chmod 0755 "$fake_bin/curl"

printf '%s\n' 'export EXISTING_SETTING=yes' >"$test_home/.zshrc"
mkdir -p "$test_home/.tohseno" "$test_home/Desktop/Tohseno/fixture"
printf '%s\n' preserve >"$test_home/.tohseno/preexisting-state"
printf '%s\n' app-data >"$test_home/Desktop/Tohseno/fixture/source.txt"

run_installer_for_home() {
  requested_home="$1"
  requested_start="$2"
  env \
    HOME="$requested_home" \
    SHELL=/bin/zsh \
    TMPDIR="$installer_tmp" \
    PATH="$fake_bin:$PATH" \
    TOHSENO_START_STUDIO="$requested_start" \
    TOHSENO_INSTALLER_FIXTURE_DIR="$fixture" \
    TOHSENO_INSTALLER_CURL_LOG="$curl_log" \
    TOHSENO_TEST_SERVICE_LOG="$service_log" \
    sh "$repository_root/oneshot/oneshot.sh"
}

run_installer() {
  run_installer_for_home "$test_home" "${1:-0}"
}

# Input and installation-root validation happens before any release state is
# published or an installer-controlled service is touched.
if run_installer_for_home "$test_home" 2 \
  >"$temporary_root/invalid-start.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted an invalid Studio choice." >&2
  exit 1
fi
grep -Fq "TOHSENO_START_STUDIO must be 0 or 1" \
  "$temporary_root/invalid-start.log"
unsafe_home="$temporary_root/unsafe-home"
unsafe_install_target="$temporary_root/unsafe-install-target"
mkdir "$unsafe_home" "$unsafe_install_target"
printf '%s\n' sentinel >"$unsafe_install_target/sentinel"
ln -s "$unsafe_install_target" "$unsafe_home/.tohseno"
if run_installer_for_home "$unsafe_home" 0 \
  >"$temporary_root/unsafe-root.log" 2>&1; then
  printf '%s\n' "test-installer.sh: followed a symlinked install root." >&2
  exit 1
fi
grep -Fq "contains a symlinked path component" \
  "$temporary_root/unsafe-root.log"
grep -Fqx sentinel "$unsafe_install_target/sentinel"

# Fresh install publishes a complete release, installs the service, and leaves
# all pre-existing app/private state untouched.
: >"$curl_log"
: >"$service_log"
run_installer 0 >/dev/null
install_root="$test_home/.tohseno"
first_current="$(readlink "$install_root/current")"
case "$first_current" in releases/*) ;; *) exit 1 ;; esac
first_release="$(CDPATH= cd -- "$install_root/current" && pwd -P)"
test -x "$first_release/bin/tohseno"
test -f "$first_release/share/studio/index.html"
test -f "$first_release/share/apple-identity/Package.swift"
test -f "$first_release/share/sdk/apple/TohsenoCompanionKit/Package.swift"
test -f "$first_release/share/sdk/apple/TohsenoWorkshopKit/Package.swift"
test -f "$first_release/share/companion/test-vectors/companion-v1.json"
(
  cd "$first_release"
  shasum -a 256 -c CHECKSUMS.sha256 >/dev/null
)
test -L "$install_root/share/genesis"
test "$(readlink "$install_root/share/genesis")" = "../current/share/genesis"
test -f "$test_home/Library/LaunchAgents/com.tohseno.workspace-service.plist"
test -f "$test_home/.tohseno/service/runtime.json"
grep -Fqx install "$service_log"
grep -Fqx preserve "$test_home/.tohseno/preexisting-state"
grep -Fqx app-data "$test_home/Desktop/Tohseno/fixture/source.txt"

# Reinstall switches current while retaining both immutable releases and
# preserving command/pairing state.
printf '%s\n' paired >"$test_home/.tohseno/service/devices/phone"
run_installer 0 >/dev/null
second_current="$(readlink "$install_root/current")"
test "$second_current" != "$first_current"
test -d "$first_release"
test -f "$test_home/.tohseno/service/devices/phone"
test "$(find "$install_root/releases" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq 2
test "$(grep -Fxc 'export PATH="$HOME/.tohseno/bin:$PATH"' "$test_home/.zshrc")" -eq 1

# Studio is a short-lived client invocation: the installer returns while the
# service artifact remains installed.
run_installer 1 >/dev/null
grep -Fqx studio "$service_log"
test -f "$test_home/Library/LaunchAgents/com.tohseno.workspace-service.plist"

stable_current="$(readlink "$install_root/current")"
stable_release_count="$(find "$install_root/releases" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')"

# A corrupt outer package never changes installed state.
printf '%s\n' corruption >>"$fixture/$package_name"
if run_installer 0 >"$temporary_root/outer.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted an outer checksum mismatch." >&2
  exit 1
fi
grep -Fq "Release checksum failed for $package_name" "$temporary_root/outer.log"
test "$(readlink "$install_root/current")" = "$stable_current"
publish_fixture

# An extra unmanifested inner file is rejected before publication.
printf '%s\n' extra >"$package/unmanifested"
publish_raw_fixture
if run_installer 0 >"$temporary_root/inner.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted an incomplete inner manifest." >&2
  exit 1
fi
grep -Fq "CHECKSUMS.sha256 does not cover exactly" "$temporary_root/inner.log"
test "$(readlink "$install_root/current")" = "$stable_current"
rm -f "$package/unmanifested"
write_inner_manifest

# A package containing a symlink is rejected even when its outer checksum is
# valid; no user-controlled archive link is ever extracted as release state.
ln -s common.schema.json "$package/share/protocol/schemas/unsafe-link"
publish_raw_fixture
if run_installer 0 >"$temporary_root/symlink.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted a release archive symlink." >&2
  exit 1
fi
grep -Fq "link or unsupported archive entry" "$temporary_root/symlink.log"
rm -f "$package/share/protocol/schemas/unsafe-link"

# Duplicate archive paths are rejected before extraction, even if the final
# extracted tree would otherwise collapse them to one manifest-covered file.
COPYFILE_DISABLE=1 tar -czf "$fixture/$package_name" \
  -C "$(dirname -- "$package")" "$target" "$target"
(
  refresh_outer_checksum
)
if run_installer 0 >"$temporary_root/duplicate.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted duplicate archive paths." >&2
  exit 1
fi
grep -Fq "unsafe archive path" "$temporary_root/duplicate.log"
publish_fixture

# A nested release pointer is non-canonical even when it still resolves under
# the releases directory. The rejected update leaves no additional release.
stable_release_name="${stable_current#releases/}"
rm -f "$install_root/current"
ln -s "releases/$stable_release_name/share" "$install_root/current"
publish_fixture
if run_installer 0 >"$temporary_root/nested-current.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted a nested release pointer." >&2
  exit 1
fi
grep -Fq "non-canonical target" "$temporary_root/nested-current.log"
test "$(find "$install_root/releases" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq "$stable_release_count"
rm -f "$install_root/current"
ln -s "$stable_current" "$install_root/current"

# A symlinked LaunchAgent is never overwritten or removed during an update or
# its rollback attempt.
write_cli "$package/bin/tohseno" healthy
write_inner_manifest
publish_fixture
launch_agent="$test_home/Library/LaunchAgents/com.tohseno.workspace-service.plist"
launch_agent_target="$temporary_root/unowned-launch-agent"
rm -f "$launch_agent"
printf '%s\n' unowned >"$launch_agent_target"
ln -s "$launch_agent_target" "$launch_agent"
if run_installer 0 >"$temporary_root/launch-agent.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted a symlinked LaunchAgent." >&2
  exit 1
fi
test -L "$launch_agent"
grep -Fqx unowned "$launch_agent_target"
test "$(readlink "$install_root/current")" = "$stable_current"
test "$(find "$install_root/releases" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq "$stable_release_count"
rm -f "$launch_agent"
printf '%s\n' '<!-- TOHSENO_WORKSPACE_SERVICE_PLIST_V1 -->' >"$launch_agent"

# Health failure after the pointer switch automatically restores the prior
# release and starts its service. Pairing and app state survive the rollback.
write_cli "$package/bin/tohseno" unhealthy-status
write_inner_manifest
publish_fixture
if run_installer 0 >"$temporary_root/health.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted an unhealthy service update." >&2
  exit 1
fi
grep -Fq "restored the previous release and service" "$temporary_root/health.log"
test "$(readlink "$install_root/current")" = "$stable_current"
test "$(find "$install_root/releases" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq "$stable_release_count"
test -f "$test_home/.tohseno/service/devices/phone"
grep -Fqx app-data "$test_home/Desktop/Tohseno/fixture/source.txt"
test -f "$test_home/Library/LaunchAgents/com.tohseno.workspace-service.plist"

# A failed first install removes program artifacts again while preserving
# user app data. Service state remains available for a future healthy retry.
failed_home="$temporary_root/failed-home"
mkdir -p "$failed_home/Desktop/Tohseno/existing"
printf '%s\n' app-data >"$failed_home/Desktop/Tohseno/existing/source.txt"
if run_installer_for_home "$failed_home" 0 \
  >"$temporary_root/fresh-health.log" 2>&1; then
  printf '%s\n' "test-installer.sh: accepted an unhealthy fresh service." >&2
  exit 1
fi
test ! -e "$failed_home/.tohseno/current"
test ! -e "$failed_home/.tohseno/bin/tohseno"
test ! -e "$failed_home/Library/LaunchAgents/com.tohseno.workspace-service.plist"
grep -Fqx app-data "$failed_home/Desktop/Tohseno/existing/source.txt"

printf '%s\n' "TOHSENO 1.1.0 installer lifecycle regressions passed."
