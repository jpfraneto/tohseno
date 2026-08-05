#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  printf '%s\n' \
    "usage: scripts/test-installer.sh PACKAGE_DIRECTORY GENESIS_ARCHIVE" >&2
  exit 2
fi

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
package_directory="$(CDPATH= cd -- "$1" && pwd -P)"
archive_directory="$(CDPATH= cd -- "$(dirname -- "$2")" && pwd -P)"
archive="$archive_directory/$(basename -- "$2")"

if [ -L "$package_directory/bin/tohseno" ] ||
  [ -L "$package_directory/bin/tohseno-apple-identity" ] ||
  [ ! -x "$package_directory/bin/tohseno" ] ||
  [ ! -x "$package_directory/bin/tohseno-apple-identity" ] ||
  [ ! -f "$archive" ] ||
  [ -L "$archive" ]; then
  printf '%s\n' \
    "test-installer.sh: package or archive fixture is incomplete." >&2
  exit 2
fi

temporary_parent="$(CDPATH= cd -- "${TMPDIR:-/tmp}" && pwd -P)"
temporary_root="$(mktemp -d "$temporary_parent/tohseno-installer-test.XXXXXX")"

cleanup() {
  case "$temporary_root" in
    "$temporary_parent"/tohseno-installer-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf "$temporary_root"
      fi
      ;;
    *)
      printf '%s\n' \
        "test-installer.sh: refusing unsafe cleanup." >&2
      ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

fixture="$temporary_root/fixture"
fake_bin="$temporary_root/fake-bin"
test_home="$temporary_root/home"
installer_tmp="$temporary_root/tmp"
mkdir -p "$fixture" "$fake_bin" "$test_home" "$installer_tmp"

case "$(uname -m)" in
  arm64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *)
    printf '%s\n' \
      "test-installer.sh: unsupported test architecture." >&2
    exit 2
    ;;
esac

binary_name="tohseno-$target"
helper_name="tohseno-apple-identity-$target"
materials_name="tohseno-genesis-materials.tar.gz"
cp "$package_directory/bin/tohseno" "$fixture/$binary_name"
cp "$package_directory/bin/tohseno-apple-identity" "$fixture/$helper_name"
cp "$archive" "$fixture/$materials_name"

refresh_outer_manifest() {
  (
    cd "$fixture"
    shasum -a 256 \
      "$binary_name" \
      "$helper_name" \
      "$materials_name" >SHA256SUMS
  )
}
refresh_outer_manifest
if [ ! -f "$package_directory/CHECKSUMS.sha256" ] ||
  [ -L "$package_directory/CHECKSUMS.sha256" ]; then
  printf '%s\n' \
    "test-installer.sh: package has no safe checksum manifest." >&2
  exit 2
fi
(
  cd "$package_directory"
  shasum -a 256 -c CHECKSUMS.sha256 >/dev/null
)

cat >"$fake_bin/curl" <<'FAKE_CURL'
#!/bin/sh
set -eu

url=""
destination=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -fsSL) shift ;;
    -o)
      [ "$#" -ge 2 ] || exit 2
      destination="$2"
      shift 2
      ;;
    http://* | https://*)
      url="$1"
      shift
      ;;
    *) exit 2 ;;
  esac
done
[ -n "$url" ] && [ -n "$destination" ] || exit 2
artifact="${url##*/}"
case "$artifact" in
  tohseno-aarch64-apple-darwin|\
    tohseno-x86_64-apple-darwin|\
    tohseno-apple-identity-aarch64-apple-darwin|\
    tohseno-apple-identity-x86_64-apple-darwin|\
    tohseno-genesis-materials.tar.gz|\
    SHA256SUMS) ;;
  *) exit 2 ;;
esac
expected_url="https://github.com/jpfraneto/tohseno/releases/download/v0.8.5/$artifact"
[ "$url" = "$expected_url" ] || exit 2
printf '%s\n' "$url" >>"${TOHSENO_INSTALLER_CURL_LOG:?}"
cp "${TOHSENO_INSTALLER_FIXTURE_DIR:?}/$artifact" "$destination"
FAKE_CURL
chmod 0755 "$fake_bin/curl"

mkdir "$test_home/.tohseno"
printf '%s\n' "stable-state-sentinel" >"$test_home/.tohseno/stable-state"
printf '%s\n' "export EXISTING_SETTING=yes" >"$test_home/.zshrc"
stable_digest="$(
  shasum -a 256 "$test_home/.tohseno/stable-state" | awk '{print $1}'
)"
curl_log="$temporary_root/curl.log"
expected_urls="$temporary_root/expected-urls"
printf '%s\n' \
  "https://github.com/jpfraneto/tohseno/releases/download/v0.8.5/$binary_name" \
  "https://github.com/jpfraneto/tohseno/releases/download/v0.8.5/$helper_name" \
  "https://github.com/jpfraneto/tohseno/releases/download/v0.8.5/$materials_name" \
  "https://github.com/jpfraneto/tohseno/releases/download/v0.8.5/SHA256SUMS" |
  LC_ALL=C sort >"$expected_urls"

invalid_start_log="$temporary_root/invalid-start.log"
if env \
  HOME="$test_home" \
  SHELL=/bin/zsh \
  TMPDIR="$installer_tmp" \
  PATH="$fake_bin:$PATH" \
  TOHSENO_START_STUDIO=2 \
  sh "$repository_root/oneshot/oneshot.sh" \
  >"$invalid_start_log" 2>&1; then
  printf '%s\n' \
    "test-installer.sh: accepted an invalid Studio launch choice." >&2
  exit 1
fi
grep -Fqx \
  "TOHSENO installer: TOHSENO_START_STUDIO must be 0 or 1." \
  "$invalid_start_log"
test ! -e "$test_home/.tohseno/.tohseno-install-root"

run_installer() {
  env \
    HOME="$test_home" \
    SHELL=/bin/zsh \
    TMPDIR="$installer_tmp" \
    PATH="$fake_bin:$PATH" \
    TOHSENO_START_STUDIO=0 \
    TOHSENO_INSTALLER_FIXTURE_DIR="$fixture" \
    TOHSENO_INSTALLER_CURL_LOG="$curl_log" \
    sh "$repository_root/oneshot/oneshot.sh"
}

: >"$curl_log"
run_installer >/dev/null
install_root="$test_home/.tohseno"
first_current="$(readlink "$install_root/current")"
case "$first_current" in
  releases/*) first_release_name="${first_current#releases/}" ;;
  *)
    printf '%s\n' \
      "test-installer.sh: current escaped the release directory." >&2
    exit 1
    ;;
esac
case "$first_release_name" in
  "" | "." | ".." | */*)
    printf '%s\n' \
      "test-installer.sh: current has a non-canonical target." >&2
    exit 1
    ;;
esac
first_physical="$(CDPATH= cd -- "$install_root/current" && pwd -P)"
case "$first_physical/" in
  "$install_root"/releases/*/) ;;
  *)
    printf '%s\n' \
      "test-installer.sh: current escaped its physical release root." >&2
    exit 1
    ;;
esac
"$install_root/bin/tohseno" --version |
  grep -Fqx 'tohseno 0.8.5'
"$install_root/bin/tohseno-apple-identity" --version |
  grep -Fqx 'tohseno-apple-identity 0.8.5'
test -f "$install_root/share/genesis/GENESIS.json"
test -f "$install_root/share/genesis/FILES.sha256"
test -L "$install_root/share/genesis"
test "$(readlink "$install_root/share/genesis")" = "../current/share/genesis"
test -f "$install_root/.tohseno-install-root"
test ! -L "$install_root/.tohseno-install-root"
test "$(cat "$install_root/.tohseno-install-root")" = "tohseno-stable-install-v2"
test ! -L "$install_root/bin/tohseno"
test ! -L "$install_root/bin/tohseno-apple-identity"
cmp "$first_physical/bin/tohseno" "$package_directory/bin/tohseno"
cmp \
  "$first_physical/bin/tohseno-apple-identity" \
  "$package_directory/bin/tohseno-apple-identity"
if find "$first_physical" ! -type f ! -type d -print -quit | grep -q .; then
  printf '%s\n' \
    "test-installer.sh: physical release contains a special entry." >&2
  exit 1
fi
if ! LC_ALL=C sort "$curl_log" | cmp -s - "$expected_urls"; then
  printf '%s\n' \
    "test-installer.sh: installer did not fetch the exact release set." >&2
  exit 1
fi
reference_materials="$temporary_root/reference-materials"
mkdir "$reference_materials"
COPYFILE_DISABLE=1 tar -xzf "$archive" -C "$reference_materials"
diff -qr "$reference_materials/genesis" "$first_physical/share/genesis" >/dev/null

: >"$curl_log"
run_installer >/dev/null
second_current="$(readlink "$install_root/current")"
if [ "$first_current" = "$second_current" ]; then
  printf '%s\n' \
    "test-installer.sh: reinstall did not switch releases." >&2
  exit 1
fi
test -d "$first_physical"
second_physical="$(CDPATH= cd -- "$install_root/current" && pwd -P)"
second_release_name="${second_current#releases/}"
case "$second_current:$second_release_name" in
  releases/*:?*) ;;
  *)
    printf '%s\n' \
      "test-installer.sh: reinstalled current has an invalid target." >&2
    exit 1
    ;;
esac
case "$second_release_name" in
  "." | ".." | */*)
    printf '%s\n' \
      "test-installer.sh: reinstalled current is non-canonical." >&2
    exit 1
    ;;
esac
case "$second_physical/" in
  "$install_root"/releases/*/) ;;
  *)
    printf '%s\n' \
      "test-installer.sh: reinstalled current escaped its release root." >&2
    exit 1
    ;;
esac
cmp "$second_physical/bin/tohseno" "$package_directory/bin/tohseno"
cmp \
  "$second_physical/bin/tohseno-apple-identity" \
  "$package_directory/bin/tohseno-apple-identity"
diff -qr "$reference_materials/genesis" "$second_physical/share/genesis" >/dev/null
if find "$second_physical" ! -type f ! -type d -print -quit | grep -q .; then
  printf '%s\n' \
    "test-installer.sh: reinstalled release contains a special entry." >&2
  exit 1
fi
if [ "$(
  find "$install_root/releases" -mindepth 1 -maxdepth 1 -type d |
    wc -l | tr -d ' '
)" -ne 2 ]; then
  printf '%s\n' \
    "test-installer.sh: reinstall leaked or omitted a release." >&2
  exit 1
fi
if ! LC_ALL=C sort "$curl_log" | cmp -s - "$expected_urls"; then
  printf '%s\n' \
    "test-installer.sh: reinstall did not fetch the exact release set." >&2
  exit 1
fi
first_hold="$first_physical.routing-check"
mv "$first_physical" "$first_hold"
"$install_root/bin/tohseno" --version |
  grep -Fqx 'tohseno 0.8.5'
"$install_root/bin/tohseno-apple-identity" --version |
  grep -Fqx 'tohseno-apple-identity 0.8.5'
mv "$first_hold" "$first_physical"
if [ "$(
  grep -Fxc 'export PATH="$HOME/.tohseno/bin:$PATH"' \
    "$test_home/.zshrc"
)" -ne 1 ]; then
  printf '%s\n' \
    "test-installer.sh: stable PATH entry is not idempotent." >&2
  exit 1
fi
expected_zshrc="$temporary_root/expected.zshrc"
printf '%s\n\n%s\n' \
  "export EXISTING_SETTING=yes" \
  'export PATH="$HOME/.tohseno/bin:$PATH"' >"$expected_zshrc"
if ! cmp -s "$expected_zshrc" "$test_home/.zshrc"; then
  printf '%s\n' \
    "test-installer.sh: installation clobbered shell configuration." >&2
  exit 1
fi

installed_state() {
  find "$install_root" -type d -print |
    LC_ALL=C sort |
    while IFS= read -r path; do
      stat -f '%Lp %N' "$path"
    done
  find "$install_root" -type l -print |
    LC_ALL=C sort |
    while IFS= read -r path; do
      printf '%s -> %s\n' "$path" "$(readlink "$path")"
    done
  find "$install_root" -type f -print |
    LC_ALL=C sort |
    while IFS= read -r path; do
      stat -f '%Lp %N' "$path"
      shasum -a 256 "$path"
    done
  stat -f '%Lp %N' "$test_home/.zshrc"
  shasum -a 256 "$test_home/.zshrc"
}
state_before_failure="$(installed_state)"

printf '%s\n' "corruption" >>"$fixture/$binary_name"
outer_failure="$temporary_root/outer-failure.log"
if run_installer >"$outer_failure" 2>&1; then
  printf '%s\n' \
    "test-installer.sh: accepted an outer checksum mismatch." >&2
  exit 1
fi
expected_outer_failure="$temporary_root/expected-outer-failure.log"
printf '%s\n' \
  "installing TOHSENO v0.8.5 - https://github.com/jpfraneto/tohseno" \
  "TOHSENO installer: Release checksum failed for $binary_name." \
  >"$expected_outer_failure"
if ! cmp -s "$expected_outer_failure" "$outer_failure"; then
  printf '%s\n' \
    "test-installer.sh: checksum test failed for an unrelated reason." >&2
  exit 1
fi
if [ "$(installed_state)" != "$state_before_failure" ]; then
  printf '%s\n' \
    "test-installer.sh: checksum failure changed installed state." >&2
  exit 1
fi

cp "$package_directory/bin/tohseno" "$fixture/$binary_name"
invalid_materials="$temporary_root/invalid-materials"
mkdir "$invalid_materials"
tar -xzf "$archive" -C "$invalid_materials"
printf '%s\n' "not covered by FILES.sha256" \
  >"$invalid_materials/genesis/unmanifested.txt"
(
  cd "$invalid_materials"
  COPYFILE_DISABLE=1 tar -czf "$fixture/$materials_name" genesis
)
refresh_outer_manifest
inner_failure="$temporary_root/inner-failure.log"
if run_installer >"$inner_failure" 2>&1; then
  printf '%s\n' \
    "test-installer.sh: accepted an incomplete inner manifest." >&2
  exit 1
fi
expected_inner_failure="$temporary_root/expected-inner-failure.log"
printf '%s\n' \
  "installing TOHSENO v0.8.5 - https://github.com/jpfraneto/tohseno" \
  "TOHSENO installer: Genesis FILES.sha256 does not cover exactly the staged files." \
  >"$expected_inner_failure"
if ! cmp -s "$expected_inner_failure" "$inner_failure"; then
  printf '%s\n' \
    "test-installer.sh: inner-manifest test failed for an unrelated reason." >&2
  exit 1
fi
if [ "$(installed_state)" != "$state_before_failure" ]; then
  printf '%s\n' \
    "test-installer.sh: inner-manifest failure changed installed state." >&2
  exit 1
fi

cp "$archive" "$fixture/$materials_name"
refresh_outer_manifest
printf '%s\n' "unrecognized-marker" >"$install_root/.tohseno-install-root"
late_state_before="$(installed_state)"
late_failure="$temporary_root/late-failure.log"
if run_installer >"$late_failure" 2>&1; then
  printf '%s\n' \
    "test-installer.sh: accepted an unrecognized late marker." >&2
  exit 1
fi
if ! grep -Fqx \
  "TOHSENO installer: Existing installation marker is unrecognized." \
  "$late_failure"; then
  printf '%s\n' \
    "test-installer.sh: late rollback failed for an unrelated reason." >&2
  exit 1
fi
if [ "$(installed_state)" != "$late_state_before" ]; then
  printf '%s\n' \
    "test-installer.sh: late failure did not roll back installed state." >&2
  exit 1
fi
printf '%s\n' "tohseno-stable-install-v2" \
  >"$install_root/.tohseno-install-root"

observed_stable_digest="$(
  shasum -a 256 "$test_home/.tohseno/stable-state" | awk '{print $1}'
)"
if [ "$observed_stable_digest" != "$stable_digest" ]; then
  printf '%s\n' \
    "test-installer.sh: installation changed pre-existing stable state." >&2
  exit 1
fi

bootstrap_home="$temporary_root/bootstrap-home"
bootstrap_log="$temporary_root/bootstrap.log"
mkdir "$bootstrap_home"
cat >"$fixture/$binary_name" <<'FAKE_TOHSENO'
#!/bin/sh
set -eu
case "${1:-}" in
  --version) printf '%s\n' "tohseno 0.8.5" ;;
  studio) printf '%s\n' "studio" >>"${TOHSENO_BOOTSTRAP_LOG:?}" ;;
  *) exit 2 ;;
esac
FAKE_TOHSENO
cat >"$fixture/$helper_name" <<'FAKE_IDENTITY'
#!/bin/sh
set -eu
[ "${1:-}" = "--version" ] || exit 2
printf '%s\n' "tohseno-apple-identity 0.8.5"
FAKE_IDENTITY
chmod 0755 "$fixture/$binary_name" "$fixture/$helper_name"
refresh_outer_manifest
env \
  HOME="$bootstrap_home" \
  SHELL=/bin/zsh \
  TMPDIR="$installer_tmp" \
  PATH="$fake_bin:$PATH" \
  TOHSENO_BOOTSTRAP_LOG="$bootstrap_log" \
  TOHSENO_INSTALLER_FIXTURE_DIR="$fixture" \
  TOHSENO_INSTALLER_CURL_LOG="$curl_log" \
  sh "$repository_root/oneshot/oneshot.sh" >/dev/null
grep -Fqx "studio" "$bootstrap_log"

printf '%s\n' "Stable 0.8.5 installer regressions passed."
