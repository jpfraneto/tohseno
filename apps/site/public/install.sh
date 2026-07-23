#!/bin/sh
# TOHSENO managed installer. It installs only checksum-pinned release artifacts
# under ~/.tohseno and never asks for credentials.
set -eu
umask 022

INSTALLER_VERSION="0.2.0"
CLI_VERSION="0.2.0"
CLI_ARTIFACT="tohseno-cli-${CLI_VERSION}.tar.gz"
CLI_URL_DEFAULT="https://github.com/jpfraneto/tohseno/releases/download/cli-v${CLI_VERSION}/${CLI_ARTIFACT}"
# Filled from `bun run tohseno:release` before the prepared release is published.
CLI_SHA256_DEFAULT="b8ae8726b69dbd858149813566dc79755aacc77af56bfac6a214431783b9f5eb"

BUN_VERSION="1.2.18"
BUN_RELEASE_BASE="https://github.com/oven-sh/bun/releases/download/bun-v${BUN_VERSION}"
CLOUDFLARED_VERSION="2026.5.2"
CLOUDFLARED_RELEASE_BASE="https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}"

modify_path=1
install_cloudflared=1
dry_run=0

say() { printf '%s\n' "$*"; }
die() { printf 'tohseno installer: %s\n' "$*" >&2; exit 1; }

usage() {
  say "TOHSENO installer ${INSTALLER_VERSION}"
  say ""
  say "Usage: install.sh [--non-interactive] [--no-modify-path] [--without-cloudflared] [--dry-run]"
  say ""
  say "Installs a checksum-pinned TOHSENO source release, managed Bun runtime,"
  say "and (when missing) a checksum-pinned cloudflared binary under ~/.tohseno."
  say "No credentials are requested or collected. macOS and Linux on arm64/x86_64 are supported;"
  say "only macOS with Xcode can build and launch the current iOS platform."
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --version) say "$INSTALLER_VERSION"; exit 0 ;;
    --non-interactive|--yes) ;;
    --no-modify-path) modify_path=0 ;;
    --without-cloudflared) install_cloudflared=0 ;;
    --dry-run) dry_run=1 ;;
    *) die "unknown option: $1" ;;
  esac
  shift
done

[ -n "${HOME:-}" ] || die "HOME is required"
os="${TOHSENO_INSTALL_OS:-$(uname -s)}"
arch="${TOHSENO_INSTALL_ARCH:-$(uname -m)}"

case "$os/$arch" in
  Darwin/arm64|Darwin/aarch64)
    platform="darwin-arm64"
    bun_asset="bun-darwin-aarch64.zip"
    bun_sha="cca9eb52762bbd81eb894fc8275bba0a0654e81aad318d19869854a30f3769a2"
    cloudflared_asset="cloudflared-darwin-arm64.tgz"
    cloudflared_sha="cd9f764abfd06757b4def10ee5ba3d862381ed9fc02d6c1f06086c23d88695c6"
    cloudflared_kind="tgz"
    ;;
  Darwin/x86_64|Darwin/amd64)
    platform="darwin-amd64"
    bun_asset="bun-darwin-x64.zip"
    bun_sha="c83ea4ef2126cc942056ff1958518181a2a5b6723d6aa57c96b5d0fb34d4b7dc"
    cloudflared_asset="cloudflared-darwin-amd64.tgz"
    cloudflared_sha="c4fdc6021cd63003e32e70b577e17d47d493c6df4e24c7c97169ed74b67a715d"
    cloudflared_kind="tgz"
    ;;
  Linux/aarch64|Linux/arm64)
    platform="linux-arm64"
    bun_asset="bun-linux-aarch64.zip"
    bun_sha="1bad1671d05ba15696315ca7248ec043d29b595ff5fb15fa86b699c2255d8bc5"
    cloudflared_asset="cloudflared-linux-arm64"
    cloudflared_sha="5a4e8ce2701105271412059f44b6a0bf1ae4542b4d98ff3180c0c019443a5815"
    cloudflared_kind="binary"
    ;;
  Linux/x86_64|Linux/amd64)
    platform="linux-amd64"
    bun_asset="bun-linux-x64.zip"
    bun_sha="90e032a982ae299c62d645dac6caaa8eb00b69092bc8501bf13a590de8d099c8"
    cloudflared_asset="cloudflared-linux-amd64"
    cloudflared_sha="5286698547f03df745adb2355f04c12dde52ef425491e81f433642d695521886"
    cloudflared_kind="binary"
    ;;
  *) die "unsupported platform $os/$arch; supported: macOS or Linux on arm64/x86_64" ;;
esac

install_root="${TOHSENO_INSTALL_HOME:-$HOME/.tohseno}"
cli_url="${TOHSENO_INSTALL_CLI_URL:-$CLI_URL_DEFAULT}"
cli_sha="${TOHSENO_INSTALL_CLI_SHA256:-$CLI_SHA256_DEFAULT}"
bun_url="${TOHSENO_INSTALL_BUN_URL:-$BUN_RELEASE_BASE/$bun_asset}"
bun_sha="${TOHSENO_INSTALL_BUN_SHA256:-$bun_sha}"
cloudflared_url="${TOHSENO_INSTALL_CLOUDFLARED_URL:-$CLOUDFLARED_RELEASE_BASE/$cloudflared_asset}"
cloudflared_sha="${TOHSENO_INSTALL_CLOUDFLARED_SHA256:-$cloudflared_sha}"

case "$cli_sha" in
  [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*)
    [ "${#cli_sha}" -eq 64 ] || die "CLI checksum is not a complete SHA-256 digest" ;;
  *) die "CLI release is prepared but not published; the pinned artifact checksum has not been finalized" ;;
esac

if [ "$dry_run" -eq 1 ]; then
  say "TOHSENO ${CLI_VERSION} install plan for ${platform}"
  say "  root: $install_root"
  say "  CLI: $cli_url"
  say "  Bun ${BUN_VERSION}: $bun_url"
  if [ "$install_cloudflared" -eq 1 ]; then say "  cloudflared ${CLOUDFLARED_VERSION}: $cloudflared_url"; fi
  say "No files changed."
  exit 0
fi

command -v curl >/dev/null 2>&1 || die "curl is required to download verified artifacts"
command -v tar >/dev/null 2>&1 || die "tar is required to unpack the TOHSENO release"
command -v unzip >/dev/null 2>&1 || die "unzip is required to unpack the managed Bun runtime"

if command -v shasum >/dev/null 2>&1; then
  hash_file() { shasum -a 256 "$1" | awk '{print $1}'; }
elif command -v sha256sum >/dev/null 2>&1; then
  hash_file() { sha256sum "$1" | awk '{print $1}'; }
else
  die "shasum or sha256sum is required for artifact verification"
fi

download() {
  source=$1
  destination=$2
  case "$source" in
    file://*) cp "${source#file://}" "$destination" ;;
    /*|./*|../*) cp "$source" "$destination" ;;
    https://*) curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 "$source" -o "$destination" ;;
    http://127.0.0.1:*|http://localhost:*)
      [ "${TOHSENO_INSTALL_ALLOW_HTTP:-0}" = "1" ] || die "HTTP artifact overrides require TOHSENO_INSTALL_ALLOW_HTTP=1 and localhost"
      curl --fail --silent --show-error --location "$source" -o "$destination"
      ;;
    *) die "artifact URL must use HTTPS, a local file, or an explicit localhost test override" ;;
  esac
}

verify() {
  file=$1
  expected=$2
  actual=$(hash_file "$file")
  [ "$actual" = "$expected" ] || die "checksum mismatch for $(basename "$file"): expected $expected, got $actual"
}

temporary=$(mktemp -d "${TMPDIR:-/tmp}/tohseno-install.XXXXXX")
cleanup() { rm -rf "$temporary"; }
trap cleanup EXIT HUP INT TERM

mkdir -p "$install_root/versions" "$install_root/runtime" "$install_root/tools" "$install_root/bin"

cli_destination="$install_root/versions/$CLI_VERSION"
if [ -e "$cli_destination" ] || [ -L "$cli_destination" ]; then
  [ -d "$cli_destination" ] && [ ! -L "$cli_destination" ] || die "managed CLI version is not a real directory: $cli_destination"
  [ -f "$cli_destination/.artifact.sha256" ] || die "existing managed CLI version is incomplete: $cli_destination"
  [ "$(sed -n '1p' "$cli_destination/.artifact.sha256")" = "$cli_sha" ] || die "existing CLI version has a different checksum: $cli_destination"
  say "TOHSENO ${CLI_VERSION} already verified."
else
  cli_archive="$temporary/$CLI_ARTIFACT"
  download "$cli_url" "$cli_archive"
  verify "$cli_archive" "$cli_sha"
  mkdir -p "$temporary/cli"
  tar -xzf "$cli_archive" -C "$temporary/cli"
  extracted_cli="$temporary/cli/tohseno-cli-$CLI_VERSION"
  [ -f "$extracted_cli/factory-source/packages/cli/src/bin.ts" ] || die "verified CLI artifact has an unexpected layout"
  cli_staging="$install_root/versions/.${CLI_VERSION}.installing-$$"
  rm -rf "$cli_staging"
  mv "$extracted_cli" "$cli_staging"
  printf '%s\n' "$cli_sha" > "$cli_staging/.artifact.sha256"
  mv "$cli_staging" "$cli_destination"
  say "Installed TOHSENO ${CLI_VERSION}."
fi
if [ -e "$install_root/versions/current" ] && [ ! -L "$install_root/versions/current" ]; then
  die "managed CLI current pointer is not a symlink: $install_root/versions/current"
fi
ln -sfn "$CLI_VERSION" "$install_root/versions/current"

bun_destination="$install_root/runtime/bun-$BUN_VERSION"
if [ -e "$bun_destination" ] || [ -L "$bun_destination" ]; then
  [ -d "$bun_destination" ] && [ ! -L "$bun_destination" ] || die "managed Bun runtime is not a real directory: $bun_destination"
  [ -f "$bun_destination/.artifact.sha256" ] || die "existing managed Bun runtime is incomplete: $bun_destination"
  [ "$(sed -n '1p' "$bun_destination/.artifact.sha256")" = "$bun_sha" ] || die "existing managed Bun runtime has a different checksum"
  say "Managed Bun ${BUN_VERSION} already verified."
else
  bun_archive="$temporary/$bun_asset"
  download "$bun_url" "$bun_archive"
  verify "$bun_archive" "$bun_sha"
  mkdir -p "$temporary/bun"
  unzip -q "$bun_archive" -d "$temporary/bun"
  bun_binary=$(find "$temporary/bun" -type f -name bun -print | head -n 1)
  [ -n "$bun_binary" ] || die "verified Bun artifact has an unexpected layout"
  bun_staging="$install_root/runtime/.bun-$BUN_VERSION.installing-$$"
  rm -rf "$bun_staging"
  mkdir -p "$bun_staging/bin"
  cp "$bun_binary" "$bun_staging/bin/bun"
  chmod 755 "$bun_staging/bin/bun"
  printf '%s\n' "$bun_sha" > "$bun_staging/.artifact.sha256"
  mv "$bun_staging" "$bun_destination"
  say "Installed managed Bun ${BUN_VERSION}."
fi
if [ -e "$install_root/runtime/current" ] && [ ! -L "$install_root/runtime/current" ]; then
  die "managed Bun current pointer is not a symlink: $install_root/runtime/current"
fi
ln -sfn "bun-$BUN_VERSION" "$install_root/runtime/current"

if [ "$install_cloudflared" -eq 1 ]; then
  existing_cloudflared=$(command -v cloudflared 2>/dev/null || true)
  if [ -n "$existing_cloudflared" ]; then
    say "Using existing cloudflared at $existing_cloudflared."
  else
    cloudflared_destination="$install_root/tools/cloudflared-$CLOUDFLARED_VERSION"
    cloudflared_marker="$cloudflared_destination.sha256"
    if [ -e "$cloudflared_destination" ] || [ -L "$cloudflared_destination" ]; then
      [ -f "$cloudflared_destination" ] && [ ! -L "$cloudflared_destination" ] || die "managed cloudflared is not a regular file"
      [ -f "$cloudflared_marker" ] || die "existing managed cloudflared is missing its checksum marker"
      [ "$(sed -n '1p' "$cloudflared_marker")" = "$cloudflared_sha" ] || die "existing managed cloudflared has a different checksum"
      say "Managed cloudflared ${CLOUDFLARED_VERSION} already verified."
    else
      cloudflared_download="$temporary/$cloudflared_asset"
      download "$cloudflared_url" "$cloudflared_download"
      verify "$cloudflared_download" "$cloudflared_sha"
      if [ "$cloudflared_kind" = "tgz" ]; then
        mkdir -p "$temporary/cloudflared"
        tar -xzf "$cloudflared_download" -C "$temporary/cloudflared"
        cloudflared_binary=$(find "$temporary/cloudflared" -type f -name cloudflared -print | head -n 1)
        [ -n "$cloudflared_binary" ] || die "verified cloudflared artifact has an unexpected layout"
      else
        cloudflared_binary="$cloudflared_download"
      fi
      cloudflared_staging="$install_root/tools/.cloudflared-$CLOUDFLARED_VERSION.installing-$$"
      cp "$cloudflared_binary" "$cloudflared_staging"
      chmod 755 "$cloudflared_staging"
      mv "$cloudflared_staging" "$cloudflared_destination"
      printf '%s\n' "$cloudflared_sha" > "$cloudflared_marker"
      say "Installed managed cloudflared ${CLOUDFLARED_VERSION}."
    fi
    ln -sfn "../tools/cloudflared-$CLOUDFLARED_VERSION" "$install_root/bin/cloudflared"
  fi
fi

wrapper="$install_root/bin/tohseno"
cat > "$wrapper" <<'WRAPPER'
#!/bin/sh
set -eu
bin_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
managed_home=$(dirname "$bin_directory")
: "${TOHSENO_HOME:=$managed_home}"
TOHSENO_BUN="$managed_home/runtime/current/bin/bun"
TOHSENO_SOURCE_ROOT="$managed_home/versions/current/factory-source"
PATH="$managed_home/bin:${PATH:-/usr/bin:/bin}"
export PATH TOHSENO_HOME TOHSENO_BUN TOHSENO_SOURCE_ROOT
exec "$TOHSENO_BUN" "$TOHSENO_SOURCE_ROOT/packages/cli/src/bin.ts" "$@"
WRAPPER
chmod 755 "$wrapper"

installed_version=$("$wrapper" --version) || die "installed TOHSENO executable did not start"
[ "$installed_version" = "$CLI_VERSION" ] || die "installed executable reported unexpected version $installed_version"

path_note=""
case ":${PATH:-}:" in
  *":$install_root/bin:"*) ;;
  *)
    if [ "$modify_path" -eq 1 ]; then
      shell_name=$(basename "${SHELL:-sh}")
      case "$shell_name" in
        zsh) shell_file="$HOME/.zshrc" ;;
        bash) shell_file="$HOME/.bashrc" ;;
        *) shell_file="$HOME/.profile" ;;
      esac
      marker="# >>> tohseno managed path >>>"
      if ! grep -F "$marker" "$shell_file" >/dev/null 2>&1; then
        {
          printf '\n%s\n' "$marker"
          printf 'export PATH="%s/bin:$PATH"\n' "$install_root"
          printf '%s\n' "# <<< tohseno managed path <<<"
        } >> "$shell_file"
      fi
      path_note="Restart your shell, or run: export PATH=\"$install_root/bin:\$PATH\""
    else
      path_note="Add TOHSENO to this shell: export PATH=\"$install_root/bin:\$PATH\""
    fi
    ;;
esac

say ""
say "TOHSENO ${CLI_VERSION} is installed at $wrapper"
[ -z "$path_note" ] || say "$path_note"
if command -v git >/dev/null 2>&1; then say "Git: found"; else say "Git: missing (required to create shots)"; fi
if command -v xcodebuild >/dev/null 2>&1; then say "Xcode: found"; else say "Xcode: missing (shots can be created; iOS cannot launch here)"; fi
if command -v codex >/dev/null 2>&1; then say "Codex: found"; else say "Codex: not found"; fi
if command -v claude >/dev/null 2>&1; then say "Claude Code: found"; else say "Claude Code: not found"; fi
if command -v cloudflared >/dev/null 2>&1 || [ -x "$install_root/bin/cloudflared" ]; then say "cloudflared: found"; else say "cloudflared: not installed (Quick Tunnels unavailable)"; fi
say ""
say "Next: tohseno"
