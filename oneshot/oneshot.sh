#!/bin/sh
set -eu

repository="https://github.com/jpfraneto/tohseno"
version="v0.9.9"
start_studio="${TOHSENO_START_STUDIO:-}"
claim_token=""
claim_requested=0

fail() {
  printf 'TOHSENO installer: %s\n' "$1" >&2
  exit 1
}

usage() {
  printf '%s\n' \
    "usage: oneshot.sh [--claim TOKEN] [--no-studio]" \
    "" \
    "Without arguments, install or update TOHSENO and open Studio." \
    "--claim installs through the same verified release chain, imports one" \
    "encrypted pending intention, and opens it in local Studio." \
    "--no-studio imports without opening Studio."
}

valid_token_segment() {
  [ "${#1}" -eq "$2" ] || return 1
  case "$1" in
    *[!A-Za-z0-9_-]*) return 1 ;;
    *) return 0 ;;
  esac
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --claim)
      [ "$#" -ge 2 ] || fail "--claim requires a token."
      [ "$claim_requested" -eq 0 ] || fail "--claim may be supplied only once."
      claim_token="$2"
      claim_requested=1
      shift 2
      ;;
    --no-studio)
      start_studio=0
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *) fail "unknown argument; run with --help." ;;
  esac
done

if [ "$claim_requested" -eq 1 ]; then
  token_version="${claim_token%%.*}"
  token_tail="${claim_token#*.}"
  token_relay="${token_tail%%.*}"
  token_tail="${token_tail#*.}"
  token_capability="${token_tail%%.*}"
  token_key="${token_tail#*.}"
  if [ "$token_version" != "ti1" ] ||
    [ "$token_tail" = "$token_key" ] ||
    [ "${token_key#*.}" != "$token_key" ] ||
    ! valid_token_segment "$token_relay" 32 ||
    ! valid_token_segment "$token_capability" 43 ||
    ! valid_token_segment "$token_key" 43; then
    claim_token=""
    token_tail=""
    token_capability=""
    token_key=""
    fail "claim token is malformed or uses an unsupported version."
  fi
  token_tail=""
  token_capability=""
  token_key=""
fi

reject_symlink_components() {
  path_check_value="$1"
  path_check_label="$2"
  case "$path_check_value" in
    /*) ;;
    *) fail "$path_check_label must be an absolute path." ;;
  esac
  case "$path_check_value" in
    */../*|*/..|*/./*|*/.) fail "$path_check_label contains a dot path component." ;;
  esac
  while [ "$path_check_value" != "/" ]; do
    if [ -L "$path_check_value" ]; then
      fail "$path_check_label contains a symlinked path component."
    fi
    path_check_value="$(dirname -- "$path_check_value")"
  done
}

require_real_directory() {
  if [ -L "$1" ] || [ ! -d "$1" ]; then
    fail "$2 must be a real directory."
  fi
}

replace_symlink_atomically() {
  replace_source="$1"
  replace_destination="$2"
  if mv -h "$replace_source" "$replace_destination" 2>/dev/null; then
    return 0
  fi
  if mv -T "$replace_source" "$replace_destination" 2>/dev/null; then
    return 0
  fi
  fail "this system cannot atomically switch the installed release."
}

if [ -z "$start_studio" ]; then
  start_studio=1
fi
case "$start_studio" in
  0 | 1) ;;
  *) fail "TOHSENO_START_STUDIO must be 0 or 1." ;;
esac

printf 'installing TOHSENO %s - %s\n' "$version" "$repository"

if [ "$(uname -s)" != "Darwin" ]; then
  fail "Run this installer on a Mac."
fi
macos_version="$(sw_vers -productVersion 2>/dev/null)" ||
  fail "Could not determine the macOS version."
macos_major="${macos_version%%.*}"
case "$macos_major" in
  '' | *[!0-9]*) fail "Could not parse the macOS version." ;;
esac
if [ "$macos_major" -lt 13 ]; then
  fail "TOHSENO 0.9.9 requires macOS 13 or later."
fi

case "$(uname -m)" in
  arm64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *) fail "Use an Apple silicon or Intel Mac." ;;
esac

case "${HOME:-}" in
  /*) ;;
  *) fail "HOME must be an absolute path." ;;
esac
reject_symlink_components "$HOME" "HOME"
require_real_directory "$HOME" "HOME"
home_directory="$(CDPATH= cd -- "$HOME" && pwd -P)"
case "${SHELL:-}" in
  */zsh) shell_file="$home_directory/.zshrc" ;;
  */bash) shell_file="$home_directory/.bashrc" ;;
  *) shell_file="$home_directory/.profile" ;;
esac
if [ -L "$shell_file" ] ||
  { [ -e "$shell_file" ] && [ ! -f "$shell_file" ]; }; then
  fail "Refusing an unsafe shell startup file."
fi

install_root="$home_directory/.tohseno"
command_name="tohseno"

reject_symlink_components "$install_root" "TOHSENO installation root"
if [ -e "$install_root" ] && [ ! -d "$install_root" ]; then
  fail "TOHSENO installation root is not a directory."
fi
mkdir -p "$install_root"
require_real_directory "$install_root" "TOHSENO installation root"

install_directory="$install_root/bin"
if [ -e "$install_directory" ] || [ -L "$install_directory" ]; then
  require_real_directory "$install_directory" "TOHSENO executable directory"
else
  mkdir "$install_directory"
fi

temporary_root="${TMPDIR:-/tmp}"
case "$temporary_root" in
  /*) ;;
  *) fail "temporary directory must be an absolute path." ;;
esac
case "$temporary_root" in
  */../*|*/..|*/./*|*/.) fail "temporary directory contains a dot path component." ;;
esac
if [ ! -d "$temporary_root" ]; then
  fail "temporary directory must exist."
fi
temporary_root="$(CDPATH= cd -- "$temporary_root" && pwd -P)"

install_temporary=""
release_stage=""
launcher_stage=""
helper_launcher_stage=""
current_stage=""
marker_stage=""
published_release=""
current_path=""
release_name=""
previous_current_target=""
current_changed=0
previous_binary=""
previous_helper=""
previous_materials=""
previous_materials_link=0
binary_launcher_changed=0
helper_launcher_changed=0
materials_link_changed=0
fresh_current_created=0
install_marker_created=0
release_committed=0

cleanup() {
  if [ -n "$install_temporary" ]; then
    case "$install_temporary" in
      "$temporary_root"/tohseno-install.*)
        if [ -d "$install_temporary" ] && [ ! -L "$install_temporary" ]; then
          rm -rf "$install_temporary"
        fi
        ;;
      *) printf '%s\n' "TOHSENO installer: refusing unsafe temporary cleanup." >&2 ;;
    esac
  fi
  if [ -n "$release_stage" ]; then
    case "$release_stage" in
      "$install_root"/releases/.release-stage.*)
        if [ -d "$release_stage" ] && [ ! -L "$release_stage" ]; then
          rm -rf "$release_stage"
        fi
        ;;
      *) printf '%s\n' "TOHSENO installer: refusing unsafe release cleanup." >&2 ;;
    esac
  fi
  for cleanup_file in \
    "$launcher_stage" "$helper_launcher_stage" "$current_stage" "$marker_stage"; do
    if [ -n "$cleanup_file" ] && { [ -f "$cleanup_file" ] || [ -L "$cleanup_file" ]; }; then
      case "$cleanup_file" in
        "$install_root"/*) rm -f "$cleanup_file" ;;
        *) printf '%s\n' "TOHSENO installer: refusing unsafe file cleanup." >&2 ;;
      esac
    fi
  done
  if [ "$release_committed" -eq 0 ]; then
    if [ "$binary_launcher_changed" -eq 1 ]; then
      rm -f "$install_directory/tohseno"
      if [ -n "$previous_binary" ] && [ -f "$previous_binary" ]; then
        mv "$previous_binary" "$install_directory/tohseno"
      fi
    fi
    if [ "$helper_launcher_changed" -eq 1 ]; then
      rm -f "$install_directory/tohseno-apple-identity"
      if [ -n "$previous_helper" ] && [ -f "$previous_helper" ]; then
        mv "$previous_helper" "$install_directory/tohseno-apple-identity"
      fi
    fi
    if [ "$materials_link_changed" -eq 1 ]; then
      rm -f "$install_root/share/genesis"
      if [ -n "$previous_materials" ] && [ -d "$previous_materials" ]; then
        mv "$previous_materials" "$install_root/share/genesis"
      elif [ "$previous_materials_link" -eq 1 ]; then
        ln -s "../current/share/genesis" "$install_root/share/genesis"
      fi
    fi
    if [ "$current_changed" -eq 1 ] && [ -n "$previous_current_target" ]; then
      if [ -L "$current_path" ] &&
        [ "$(readlink "$current_path")" = "releases/$release_name" ]; then
        rollback_stage="$install_root/.current.rollback.$$"
        ln -s "$previous_current_target" "$rollback_stage"
        replace_symlink_atomically "$rollback_stage" "$current_path"
      elif [ -e "$current_path" ] || [ -L "$current_path" ]; then
        printf '%s\n' \
          "TOHSENO installer: refusing to restore a changed release pointer." >&2
      fi
    elif [ "$fresh_current_created" -eq 1 ]; then
      if [ -L "$current_path" ] &&
        [ "$(readlink "$current_path")" = "releases/$release_name" ]; then
        rm -f "$current_path"
      elif [ -e "$current_path" ] || [ -L "$current_path" ]; then
        printf '%s\n' \
          "TOHSENO installer: refusing to remove a changed release pointer." >&2
      fi
    fi
    if [ "$install_marker_created" -eq 1 ]; then
      if [ -f "$install_marker" ] && [ ! -L "$install_marker" ] &&
        [ "$(cat "$install_marker")" = "tohseno-stable-install-v2" ]; then
        rm -f "$install_marker"
      elif [ -e "$install_marker" ] || [ -L "$install_marker" ]; then
        printf '%s\n' \
          "TOHSENO installer: refusing to remove a changed installation marker." >&2
      fi
    fi
    if [ -n "$published_release" ]; then
      published_release_in_use=0
      if [ -L "$current_path" ]; then
        current_release="$(
          CDPATH= cd -- "$current_path" 2>/dev/null && pwd -P
        )" || current_release=""
        if [ "$current_release" = "$published_release" ]; then
          published_release_in_use=1
        fi
      fi
      case "$published_release" in
        "$install_root"/releases/*)
          if [ "$published_release_in_use" -eq 0 ] &&
            [ -d "$published_release" ] && [ ! -L "$published_release" ]; then
            rm -rf "$published_release"
          elif [ "$published_release_in_use" -eq 1 ]; then
            printf '%s\n' \
              "TOHSENO installer: retaining the release referenced by current." >&2
          fi
          ;;
        *)
          printf '%s\n' \
            "TOHSENO installer: refusing unsafe published-release cleanup." >&2
          ;;
      esac
    fi
  fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

install_temporary="$(mktemp -d "$temporary_root/tohseno-install.XXXXXX")" ||
  fail "Could not create a private download directory."

package_name="tohseno-release-$target.tar.gz"
for artifact in SHA256SUMS "$package_name"; do
  case "$artifact" in
    "$package_name") maximum_download_bytes=536870912 ;;
    SHA256SUMS) maximum_download_bytes=1048576 ;;
  esac
  if ! curl -fsSL \
    --proto '=https' \
    --proto-redir '=https' \
    --tlsv1.2 \
    --max-filesize "$maximum_download_bytes" \
    "$repository/releases/download/$version/$artifact" \
    -o "$install_temporary/$artifact"; then
    fail "Could not download release artifact $artifact."
  fi
  if [ -L "$install_temporary/$artifact" ] ||
    [ ! -f "$install_temporary/$artifact" ]; then
    fail "Downloaded release artifact $artifact is not a regular file."
  fi
  downloaded_bytes="$(wc -c <"$install_temporary/$artifact" | tr -d ' ')"
  case "$downloaded_bytes" in
    ''|*[!0-9]*) fail "Could not measure release artifact $artifact." ;;
  esac
  if [ "$downloaded_bytes" -eq 0 ] ||
    [ "$downloaded_bytes" -gt "$maximum_download_bytes" ]; then
    fail "Downloaded release artifact $artifact has an unsafe size."
  fi
done

verify_artifact() {
    artifact="$1"
    expected="$(
      awk -v name="$artifact" '
        $2 == name {
          count += 1
          value = $1
        }
        END {
          if (count == 1 && length(value) == 64 &&
              value !~ /[^0123456789abcdefABCDEF]/) {
            print value
          }
        }
      ' \
        "$install_temporary/SHA256SUMS"
    )"
    if [ -z "$expected" ]; then
      fail "Release checksum is missing or malformed for $artifact."
    fi
    actual="$(shasum -a 256 "$install_temporary/$artifact" | awk '{print $1}')"
    if [ "$actual" != "$expected" ]; then
      fail "Release checksum failed for $artifact."
    fi
}

verify_artifact "$package_name"

  archive="$install_temporary/$package_name"
  archive_list="$install_temporary/package.list"
  archive_verbose="$install_temporary/package.verbose"
  tar -tzf "$archive" >"$archive_list" ||
    fail "The release package is not a readable gzip archive."
  tar -tvzf "$archive" >"$archive_verbose" ||
    fail "The release package metadata could not be inspected."
  if ! awk -F/ -v root="$target" '
    seen[$0]++ { exit 1 }
    $0 == root || $0 == root "/" { next }
    $1 != root { exit 1 }
    {
      for (part = 1; part <= NF; part += 1) {
        if ($part == "" && part == NF && substr($0, length($0), 1) == "/") {
          continue
        }
        if ($part == "" || $part == "." || $part == "..") {
          exit 1
        }
      }
    }
    END { if (NR == 0) exit 1 }
  ' "$archive_list"; then
    fail "The release package contains an unsafe archive path."
  fi
  if ! awk '
    substr($1, 1, 1) != "-" && substr($1, 1, 1) != "d" { exit 1 }
  ' "$archive_verbose"; then
    fail "The release package contains a link or unsupported archive entry."
  fi
  if ! awk '
    $5 !~ /^[0-9]+$/ { exit 1 }
    {
      entries += 1
      total += $5
      if (entries > 50000 || $5 > 536870912 || total > 2147483648) {
        exit 1
      }
    }
    END { if (entries == 0) exit 1 }
  ' "$archive_verbose"; then
    fail "The release package exceeds its extracted size or entry bound."
  fi

  releases_directory="$install_root/releases"
  if [ -e "$releases_directory" ] || [ -L "$releases_directory" ]; then
    require_real_directory "$releases_directory" "release directory"
  else
    mkdir "$releases_directory"
  fi
  release_stage="$(mktemp -d "$releases_directory/.release-stage.XXXXXX")" ||
    fail "Could not stage the release on its destination filesystem."
  tar -xzf "$archive" --strip-components 1 -C "$release_stage" ||
    fail "The verified release package could not be extracted."
  if find "$release_stage" -type l -print -quit | grep -q . ||
    find "$release_stage" ! -type f ! -type d -print -quit | grep -q .; then
    fail "The staged release contains a link or unsupported file."
  fi
  if [ ! -f "$release_stage/bin/tohseno" ] ||
    [ -L "$release_stage/bin/tohseno" ] ||
    [ ! -f "$release_stage/bin/tohseno-apple-identity" ] ||
    [ -L "$release_stage/bin/tohseno-apple-identity" ] ||
    [ ! -f "$release_stage/RELEASE.json" ] ||
    [ -L "$release_stage/RELEASE.json" ] ||
    [ ! -f "$release_stage/CHECKSUMS.sha256" ] ||
    [ -L "$release_stage/CHECKSUMS.sha256" ] ||
    [ ! -d "$release_stage/share" ] ||
    [ -L "$release_stage/share" ] ||
    [ ! -d "$release_stage/share/studio" ] ||
    [ -L "$release_stage/share/studio" ] ||
    [ ! -f "$release_stage/share/studio/index.html" ] ||
    [ -L "$release_stage/share/studio/index.html" ] ||
    [ ! -f "$release_stage/share/studio/app.js" ] ||
    [ -L "$release_stage/share/studio/app.js" ] ||
    [ ! -f "$release_stage/share/studio/style.css" ] ||
    [ -L "$release_stage/share/studio/style.css" ] ||
    [ ! -f "$release_stage/share/studio/pairing-seal.png" ] ||
    [ -L "$release_stage/share/studio/pairing-seal.png" ] ||
    [ ! -d "$release_stage/share/sdk/apple/TohsenoCompanionKit" ] ||
    [ -L "$release_stage/share/sdk/apple/TohsenoCompanionKit" ] ||
    [ ! -f "$release_stage/share/sdk/apple/TohsenoCompanionKit/Package.swift" ] ||
    [ -L "$release_stage/share/sdk/apple/TohsenoCompanionKit/Package.swift" ] ||
    [ ! -f "$release_stage/share/sdk/apple/TohsenoCompanionKit/VERSION" ] ||
    [ -L "$release_stage/share/sdk/apple/TohsenoCompanionKit/VERSION" ] ||
    [ ! -f "$release_stage/share/sdk/apple/TohsenoCompanionKit/LICENSE" ] ||
    [ -L "$release_stage/share/sdk/apple/TohsenoCompanionKit/LICENSE" ] ||
    [ ! -f "$release_stage/share/sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift" ] ||
    [ -L "$release_stage/share/sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift" ] ||
    [ ! -f "$release_stage/share/companion/test-vectors/companion-v1.json" ] ||
    [ -L "$release_stage/share/companion/test-vectors/companion-v1.json" ] ||
    [ ! -f "$release_stage/share/protocol/schemas/common.schema.json" ] ||
    [ -L "$release_stage/share/protocol/schemas/common.schema.json" ] ||
    [ ! -f "$release_stage/share/fascia/apple/FASCIA.json" ] ||
    [ -L "$release_stage/share/fascia/apple/FASCIA.json" ] ||
    [ ! -f "$release_stage/share/genesis/GENESIS.json" ] ||
    [ -L "$release_stage/share/genesis/GENESIS.json" ]; then
    fail "The staged 0.9.9 release is incomplete or unsafe."
  fi
  chmod 0755 \
    "$release_stage/bin/tohseno" \
    "$release_stage/bin/tohseno-apple-identity"

  package_manifest="$release_stage/CHECKSUMS.sha256"
  manifest_paths="$install_temporary/package.manifest.paths"
  if ! awk '
    {
      digest = substr($0, 1, 64)
      separator = substr($0, 65, 2)
      relative_path = substr($0, 67)
      if (length(digest) != 64 ||
          digest ~ /[^0123456789abcdefABCDEF]/ ||
          separator != "  " ||
          relative_path == "" ||
          relative_path ~ /[^A-Za-z0-9._\/-]/ ||
          relative_path == "CHECKSUMS.sha256" ||
          seen[relative_path]++) {
        exit 1
      }
      component_count = split(relative_path, components, "/")
      for (component = 1; component <= component_count; component += 1) {
        if (components[component] == "" ||
            components[component] == "." ||
            components[component] == "..") {
          exit 1
        }
      }
      print relative_path
    }
    END { if (NR == 0) exit 1 }
  ' "$package_manifest" >"$manifest_paths"; then
    fail "The release CHECKSUMS.sha256 contains a malformed or unsafe entry."
  fi
  actual_package_paths="$install_temporary/package.actual.paths"
  raw_package_paths="$install_temporary/package.actual.raw"
  (
    cd "$release_stage"
    find . -type f -print
  ) >"$raw_package_paths" ||
    fail "Could not enumerate the staged release."
  awk '
    {
      sub(/^\.\//, "")
      if ($0 != "CHECKSUMS.sha256") print
    }
  ' "$raw_package_paths" |
    LC_ALL=C sort >"$actual_package_paths" ||
    fail "Could not normalize the staged release file list."
  LC_ALL=C sort "$manifest_paths" >"$manifest_paths.sorted" ||
    fail "Could not normalize the release manifest paths."
  if ! cmp -s "$manifest_paths.sorted" "$actual_package_paths"; then
    fail "Release CHECKSUMS.sha256 does not cover exactly the staged files."
  fi
  (
    cd "$release_stage"
    shasum -a 256 -c CHECKSUMS.sha256 >/dev/null
  ) || fail "Release CHECKSUMS.sha256 verification failed."

  release_manifest="$release_stage/RELEASE.json"
  manifest_value() {
    /usr/bin/plutil -extract "$1" raw -o - "$release_manifest" 2>/dev/null
  }
  if [ "$(manifest_value schema)" != "tohseno.release/1" ] ||
    [ "$(manifest_value version)" != "0.9.9" ] ||
    [ "$(manifest_value codename)" != "COMPANION" ] ||
    [ "$(manifest_value target)" != "$target" ] ||
    [ "$(manifest_value channel)" != "stable" ] ||
    [ "$(manifest_value dirty)" != "false" ] ||
    [ "$(manifest_value prerelease)" != "false" ]; then
    fail "The immutable release manifest is not an authorized clean 0.9.9 package."
  fi
  manifest_source_commit="$(manifest_value source_commit)"
  manifest_source_state="$(manifest_value source_state_sha256)"
  if [ "${#manifest_source_commit}" -ne 40 ] ||
    [ "${#manifest_source_state}" -ne 64 ]; then
    fail "The immutable release manifest has malformed source provenance."
  fi
  case "$manifest_source_commit:$manifest_source_state" in
    *[!0123456789abcdef:]*|*:*:*)
      fail "The immutable release manifest has malformed source provenance."
      ;;
  esac
  if [ "$manifest_source_commit" = "0000000000000000000000000000000000000000" ] ||
    [ "$manifest_source_state" = "0000000000000000000000000000000000000000000000000000000000000000" ]; then
    fail "The immutable release manifest has empty source provenance."
  fi

  for release_material in protocol fascia studio sdk companion genesis; do
    if [ ! -d "$release_stage/share/$release_material" ] ||
      [ -L "$release_stage/share/$release_material" ]; then
      fail "The release is missing safe $release_material material."
    fi
  done

  installed_version="$("$release_stage/bin/tohseno" --version 2>/dev/null)" ||
    fail "The TOHSENO executable did not start from its private stage."
  if [ "$installed_version" != "tohseno 0.9.9" ]; then
    fail "The downloaded executable is not the pinned 0.9.9 release."
  fi
  helper_version="$("$release_stage/bin/tohseno-apple-identity" --version 2>/dev/null)" ||
    fail "The Apple identity helper did not start from its private stage."
  if [ "$helper_version" != "tohseno-apple-identity 0.9.9" ]; then
    fail "The downloaded Apple identity helper is not the pinned 0.9.9 release."
  fi

  release_nonce="${release_stage##*.release-stage.}"
  case "$release_nonce" in
    ''|*[!A-Za-z0-9]*) fail "The release stage has an unsafe identifier." ;;
  esac
  release_name="${version#v}-$target-$(date +%s)-$$-$release_nonce"
  release_directory="$releases_directory/$release_name"
  if [ -e "$release_directory" ] || [ -L "$release_directory" ]; then
    fail "Refusing to replace an existing release directory."
  fi
  mv "$release_stage" "$release_directory"
  published_release="$release_directory"
  release_stage=""

  current_path="$install_root/current"
  had_current=0
  if [ -L "$current_path" ]; then
    had_current=1
    previous_current_target="$(readlink "$current_path")"
    case "$previous_current_target" in
      releases/*) ;;
      *) fail "The existing release link has an unsafe target." ;;
    esac
    previous_release_name="${previous_current_target#releases/}"
    case "$previous_release_name" in
      ''|.|..|*/*)
        fail "The existing release link has a non-canonical target."
        ;;
    esac
    current_physical="$(CDPATH= cd -- "$current_path" && pwd -P)" ||
      fail "The existing release link is broken."
    if [ "$current_physical" != "$releases_directory/$previous_release_name" ]; then
      fail "The existing release link escapes its releases directory."
    fi
  elif [ -e "$current_path" ]; then
    fail "Refusing a non-symlink release pointer."
  fi

  if [ -L "$install_directory/tohseno" ] ||
    { [ -e "$install_directory/tohseno" ] &&
      [ ! -f "$install_directory/tohseno" ]; } ||
    [ -L "$install_directory/tohseno-apple-identity" ] ||
    { [ -e "$install_directory/tohseno-apple-identity" ] &&
      [ ! -f "$install_directory/tohseno-apple-identity" ]; }; then
    fail "Refusing unsafe existing launcher paths."
  fi

  launcher_stage="$(mktemp "$install_directory/.tohseno.XXXXXX")" ||
    fail "Could not stage the TOHSENO launcher."
  cat >"$launcher_stage" <<'TOHSENO_LAUNCHER'
#!/bin/sh
set -eu

launcher_fail() {
  printf 'tohseno: %s\n' "$1" >&2
  exit 1
}

launcher_directory="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
launcher_root="$(dirname -- "$launcher_directory")"
release_root="$(CDPATH= cd -- "$launcher_root/current" && pwd -P)" ||
  launcher_fail "release pointer is missing or broken"
release_parent="$(dirname -- "$release_root")"
release_name="${release_root##*/}"
if [ "$release_parent" != "$launcher_root/releases" ]; then
  launcher_fail "release pointer escapes its installation root"
fi
case "$release_name" in
  ''|.|..) launcher_fail "release pointer has an unsafe release name" ;;
esac
tohseno_binary="$release_root/bin/tohseno"
if [ -L "$tohseno_binary" ] || [ ! -f "$tohseno_binary" ] ||
  [ ! -x "$tohseno_binary" ]; then
  launcher_fail "installed executable is missing or unsafe"
fi

exec "$tohseno_binary" "$@"
TOHSENO_LAUNCHER
  chmod 0755 "$launcher_stage"

  helper_launcher_stage="$(
    mktemp "$install_directory/.tohseno-apple-identity.XXXXXX"
  )" || fail "Could not stage the Apple identity launcher."
  cat >"$helper_launcher_stage" <<'HELPER_LAUNCHER'
#!/bin/sh
set -eu

launcher_directory="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
launcher_root="$(dirname -- "$launcher_directory")"
release_root="$(CDPATH= cd -- "$launcher_root/current" && pwd -P)" || {
  printf '%s\n' "tohseno-apple-identity: release pointer is missing or broken" >&2
  exit 1
}
release_parent="$(dirname -- "$release_root")"
release_name="${release_root##*/}"
if [ "$release_parent" != "$launcher_root/releases" ]; then
  printf '%s\n' "tohseno-apple-identity: release pointer is unsafe" >&2
  exit 1
fi
case "$release_name" in
  ''|.|..)
    printf '%s\n' "tohseno-apple-identity: release pointer is unsafe" >&2
    exit 1
    ;;
esac
helper="$release_root/bin/tohseno-apple-identity"
if [ -L "$helper" ] || [ ! -f "$helper" ] || [ ! -x "$helper" ]; then
  printf '%s\n' "tohseno-apple-identity: installed helper is missing or unsafe" >&2
  exit 1
fi
exec "$helper" "$@"
HELPER_LAUNCHER
  chmod 0755 "$helper_launcher_stage"

  if [ "$had_current" -eq 0 ]; then
    current_stage="$install_root/.current.$$"
    ln -s "releases/$release_name" "$current_stage"
    replace_symlink_atomically "$current_stage" "$current_path"
    current_stage=""
    fresh_current_created=1
  fi

  if [ -f "$install_directory/tohseno" ]; then
    previous_binary="$install_directory/.tohseno.previous.$$"
    mv "$install_directory/tohseno" "$previous_binary"
  fi
  binary_launcher_changed=1
  mv "$launcher_stage" "$install_directory/tohseno"
  launcher_stage=""
  if [ -f "$install_directory/tohseno-apple-identity" ]; then
    previous_helper="$install_directory/.tohseno-apple-identity.previous.$$"
    mv "$install_directory/tohseno-apple-identity" "$previous_helper"
  fi
  helper_launcher_changed=1
  mv "$helper_launcher_stage" "$install_directory/tohseno-apple-identity"
  helper_launcher_stage=""

  share_directory="$install_root/share"
  if [ -e "$share_directory" ] || [ -L "$share_directory" ]; then
    require_real_directory "$share_directory" "Genesis materials directory"
  else
    mkdir "$share_directory"
  fi
  if [ -L "$share_directory/genesis" ]; then
    if [ "$(readlink "$share_directory/genesis")" != "../current/share/genesis" ]; then
      fail "Existing Genesis materials link is not installer-controlled."
    fi
    previous_materials_link=1
    materials_link_changed=1
    rm -f "$share_directory/genesis"
  elif [ -e "$share_directory/genesis" ]; then
    if [ ! -d "$share_directory/genesis" ]; then
      fail "Refusing an unsafe existing Genesis materials path."
    fi
    previous_materials="$share_directory/.genesis.previous.$$"
    mv "$share_directory/genesis" "$previous_materials"
    materials_link_changed=1
  else
    materials_link_changed=1
  fi
  ln -s "../current/share/genesis" "$share_directory/.genesis.current.$$"
  mv "$share_directory/.genesis.current.$$" "$share_directory/genesis"

  install_marker="$install_root/.tohseno-install-root"
  if [ -L "$install_marker" ] ||
    { [ -e "$install_marker" ] && [ ! -f "$install_marker" ]; }; then
    fail "Refusing an unsafe installation marker."
  fi
  if [ -f "$install_marker" ]; then
    if [ "$(cat "$install_marker")" != "tohseno-stable-install-v2" ]; then
      fail "Existing installation marker is unrecognized."
    fi
  else
    marker_stage="$install_root/.tohseno-install-root.$$"
    (umask 077 && printf '%s\n' "tohseno-stable-install-v2" >"$marker_stage")
    mv "$marker_stage" "$install_marker"
    marker_stage=""
    install_marker_created=1
  fi

  if [ "$had_current" -eq 1 ]; then
    current_stage="$install_root/.current.$$"
    ln -s "releases/$release_name" "$current_stage"
    replace_symlink_atomically "$current_stage" "$current_path"
    current_stage=""
    current_changed=1
  elif [ ! -L "$current_path" ] ||
    [ "$(readlink "$current_path")" != "releases/$release_name" ]; then
    fail "The fresh release pointer changed before commit."
  fi

  validate_service_health() {
    health_file="$1"
    expected_service_version="$2"
    service_health_value() {
      /usr/bin/plutil -extract "$1" raw -o - "$health_file" 2>/dev/null
    }
    [ "$(service_health_value schema)" = "tohseno.service-status/1" ] &&
      [ "$(service_health_value healthy)" = "true" ] &&
      [ -n "$(service_health_value service_version)" ] &&
      { [ -z "$expected_service_version" ] ||
        [ "$(service_health_value service_version)" = "$expected_service_version" ]; } &&
      case "$(service_health_value origin)" in
        http://127.0.0.1:*) true ;;
        *) false ;;
      esac &&
      case "$(service_health_value workspace_id)" in
        workspace_*) true ;;
        *) false ;;
      esac
  }

  install_and_verify_service() {
    expected_service_version="$1"
    health_prefix="$2"
    "$install_directory/$command_name" --json service install \
      >"$install_temporary/$health_prefix-install.json" &&
      "$install_directory/$command_name" --json service status \
        >"$install_temporary/$health_prefix-status.json" &&
      validate_service_health \
        "$install_temporary/$health_prefix-status.json" \
        "$expected_service_version"
  }

  service_install_status=0
  install_and_verify_service "0.9.9" service || service_install_status=$?
  if [ "$service_install_status" -ne 0 ]; then
    "$install_directory/$command_name" service uninstall >/dev/null 2>&1 || true
    if [ "$had_current" -eq 1 ]; then
      rollback_stage="$install_root/.current.rollback.$$"
      ln -s "$previous_current_target" "$rollback_stage"
      replace_symlink_atomically "$rollback_stage" "$current_path"
      current_changed=0
      if install_and_verify_service "" rollback 2>/dev/null; then
        printf '%s\n' \
          "TOHSENO installer: update health failed; restored the previous release and service." >&2
      else
        printf '%s\n' \
          "TOHSENO installer: update health failed; restored the previous release pointer, but its service could not be started." >&2
      fi
    fi
    fail "The 0.9.9 Local Workspace Service did not pass its verified health check."
  fi
  release_committed=1

  if [ -n "$previous_binary" ] && [ -f "$previous_binary" ]; then
    rm -f "$previous_binary"
    previous_binary=""
  fi
  if [ -n "$previous_helper" ] && [ -f "$previous_helper" ]; then
    rm -f "$previous_helper"
    previous_helper=""
  fi
  if [ -n "$previous_materials" ] && [ -d "$previous_materials" ]; then
    rm -rf "$previous_materials"
    previous_materials=""
  fi

path_line='export PATH="$HOME/.tohseno/bin:$PATH"'
if [ -L "$shell_file" ] ||
  { [ -e "$shell_file" ] && [ ! -f "$shell_file" ]; }; then
  fail "Shell startup file became unsafe during installation."
fi
if ! touch "$shell_file"; then
  fail "TOHSENO was installed, but the shell startup file is not writable."
fi
if ! grep -Fqx "$path_line" "$shell_file"; then
  if ! printf '\n%s\n' "$path_line" >>"$shell_file"; then
    fail "TOHSENO was installed, but its PATH entry could not be saved."
  fi
  printf 'Add TOHSENO to this shell now with: source %s\n' "$shell_file"
else
  printf '%s\n' "TOHSENO is already on your saved PATH."
fi

"$install_directory/$command_name" --version
"$install_directory/tohseno-apple-identity" --version
if [ -d "$install_root/apps" ]; then
  printf '%s\n' \
    "Your v0.6 apps remain untouched in $install_root/apps." \
    "After onboarding, run: tohseno migrate-legacy"
fi
if [ "$claim_requested" -eq 1 ]; then
  printf '%s\n' \
    "Importing the encrypted intention. The one-time token is not written by the installer."
  claim_status=0
  if [ "$start_studio" -eq 1 ]; then
    printf '%s\n' "$claim_token" |
      "$install_directory/$command_name" intent claim --stdin &
  else
    printf '%s\n' "$claim_token" |
      "$install_directory/$command_name" intent claim --stdin --no-open &
  fi
  claim_process=$!
  claim_token=""
  wait "$claim_process" || claim_status=$?
  claim_process=""
  if [ "$claim_status" -ne 0 ]; then
    fail "encrypted intention import did not complete. Rerun the copied claim command; local import is idempotent."
  fi
elif [ "$start_studio" -eq 1 ]; then
  printf '%s\n' \
    "Opening Studio. The Local Workspace Service remains healthy after this Terminal exits."
  "$install_directory/$command_name" studio
fi
printf 'Start Studio with: %s studio\n' "$install_directory/$command_name"
