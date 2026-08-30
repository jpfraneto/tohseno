#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
integrity_tool="$repository_root/scripts/release-package-integrity.py"
dirty_override="${TOHSENO_ALLOW_DIRTY_RELEASE:-0}"
signing_identity="${TOHSENO_RELEASE_SIGNING_IDENTITY:-}"
signing_keychain="${TOHSENO_RELEASE_SIGNING_KEYCHAIN:-}"
case "$dirty_override" in
  0 | 1) ;;
  *)
    printf '%s\n' \
      "release.sh: TOHSENO_ALLOW_DIRTY_RELEASE must be 0 or 1." >&2
    exit 2
    ;;
esac

source_commit="$(git -C "$repository_root" rev-parse --verify 'HEAD^{commit}')"
if ! source_status="$(
  git -C "$repository_root" status \
    --porcelain=v1 \
    --untracked-files=all \
    --ignore-submodules=none
)"; then
  printf '%s\n' \
    "release.sh: could not determine full-worktree source state." >&2
  exit 1
fi

source_state_sha256="$(
  python3 "$integrity_tool" source-state --repository-root "$repository_root"
)"
source_dirty=false
bundle_dirty_override=0
if [ -n "$source_status" ]; then
  source_dirty=true
  if [ "$dirty_override" != "1" ]; then
    printf '%s\n' \
      "release.sh: the full worktree is dirty; commit or stash every tracked and untracked change." \
      "For explicit local-only inspection, set TOHSENO_ALLOW_DIRTY_RELEASE=1; RELEASE.json will record dirty=true." >&2
    exit 1
  fi
  bundle_dirty_override=1
  printf '%s\n' \
    "release.sh: WARNING: assembling dirty sources; RELEASE.json will record dirty=true." >&2
fi

target="${TOHSENO_RELEASE_TARGET:-$(rustc -vV | awk '/^host:/ {print $2}')}"
output="$repository_root/dist/release-candidate/$target"
build_root="$repository_root"
snapshot_parent=""
snapshot_root=""
staging_root=""
stage_name=""

cleanup() {
  if [ -n "$staging_root" ]; then
    python3 "$integrity_tool" cleanup-stage \
      --repository-root "$repository_root" \
      --stage-name "$stage_name" ||
      printf '%s\n' \
        "release.sh: could not safely remove the package stage." >&2
  fi
  if [ -n "$snapshot_root" ]; then
    case "$snapshot_root" in
      "$snapshot_parent"/source)
        git -C "$repository_root" worktree remove --force "$snapshot_root" \
          >/dev/null 2>&1 || true
        ;;
      *)
        printf '%s\n' \
          "release.sh: refusing unsafe source-snapshot cleanup." >&2
        ;;
    esac
  fi
  if [ -n "$snapshot_parent" ]; then
    case "$snapshot_parent" in
      "${TMPDIR:-/tmp}"/tohseno-release-source.*)
        if [ -d "$snapshot_parent" ] && [ ! -L "$snapshot_parent" ]; then
          rm -rf "$snapshot_parent"
        fi
        ;;
      *)
        printf '%s\n' \
          "release.sh: refusing unsafe snapshot-root cleanup." >&2
        ;;
    esac
  fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

case "$target" in
  aarch64-apple-darwin)
    swift_triple="arm64-apple-macosx13.0"
    macho_architecture="arm64"
    ;;
  x86_64-apple-darwin)
    swift_triple="x86_64-apple-macosx13.0"
    macho_architecture="x86_64"
    ;;
  *)
    printf 'unsupported Genesis release target: %s\n' "$target" >&2
    exit 1
    ;;
esac

verify_macho() {
  binary="$1"
  actual_architectures="$(lipo -archs "$binary")"
  if [ "$actual_architectures" != "$macho_architecture" ]; then
    printf 'release.sh: unexpected architectures for %s: %s\n' \
      "$binary" "$actual_architectures" >&2
    return 1
  fi
  build_versions="$(
    vtool -show-build "$binary" |
      awk '
        $1 == "platform" { platform = $2 }
        $1 == "minos" { print platform ":" $2 }
      '
  )"
  if [ "$build_versions" != "MACOS:13.0" ]; then
    printf 'release.sh: unexpected build versions for %s: %s\n' \
      "$binary" "$build_versions" >&2
    return 1
  fi
}

sign_release_binary() {
  binary="$1"
  if [ -z "$signing_identity" ]; then
    return 0
  fi
  if [ -n "$signing_keychain" ]; then
    /usr/bin/codesign \
      --force \
      --options runtime \
      --timestamp \
      --keychain "$signing_keychain" \
      --sign "$signing_identity" \
      "$binary"
  else
    /usr/bin/codesign \
      --force \
      --options runtime \
      --timestamp \
      --sign "$signing_identity" \
      "$binary"
  fi
  /usr/bin/codesign --verify --strict --verbose=2 "$binary"
}

if [ "$source_dirty" = false ]; then
  # Build clean releases from an immutable view of the captured commit. A
  # detached worktree prevents a concurrent edit or HEAD movement in the
  # developer checkout from producing bytes labeled `dirty:false`.
  snapshot_parent="$(
    mktemp -d "${TMPDIR:-/tmp}/tohseno-release-source.XXXXXX"
  )" || {
    printf '%s\n' \
      "release.sh: could not create a private source snapshot." >&2
    exit 1
  }
  chmod 0700 "$snapshot_parent"
  snapshot_root="$snapshot_parent/source"
  if ! git -C "$repository_root" worktree add \
    --detach \
    --quiet \
    "$snapshot_root" \
    "$source_commit"; then
    printf '%s\n' \
      "release.sh: could not materialize the captured source commit." >&2
    exit 1
  fi
  build_root="$snapshot_root"
  if [ "$(git -C "$build_root" rev-parse --verify 'HEAD^{commit}')" != "$source_commit" ] ||
    [ -n "$(
      git -C "$build_root" status \
        --porcelain=v1 \
        --untracked-files=all \
        --ignore-submodules=none
    )" ]; then
    printf '%s\n' \
      "release.sh: private source snapshot is not the exact clean commit." >&2
    exit 1
  fi
  # Bundle timestamps must follow the pinned commit, not caller state.
  unset SOURCE_DATE_EPOCH
fi

MACOSX_DEPLOYMENT_TARGET=13.0 \
  TOHSENO_RELEASE_SOURCE_STATE="$source_commit:$source_dirty" cargo build \
  --manifest-path "$build_root/Cargo.toml" \
  --release \
  --locked \
  --target "$target" \
  --bin tohseno

swift build \
  --package-path "$build_root/apple-identity" \
  -c release \
  --triple "$swift_triple"

TOHSENO_ALLOW_DIRTY_BUNDLE="$bundle_dirty_override" \
  "$build_root/scripts/build-genesis-bundle.sh"

rust_binary="$build_root/target/$target/release/tohseno"
python3 "$integrity_tool" validate-file --path "$rust_binary"
verify_macho "$rust_binary"

helper_directory="$(
  swift build \
    --package-path "$build_root/apple-identity" \
    -c release \
    --triple "$swift_triple" \
    --show-bin-path
)"
helper="$helper_directory/tohseno-apple-identity"
if [ -z "$helper" ] || [ ! -x "$helper" ]; then
  printf '%s\n' "the Apple identity helper was not produced" >&2
  exit 1
fi
python3 "$integrity_tool" validate-file --path "$helper"
verify_macho "$helper"

python3 "$integrity_tool" validate-tree \
  --root "$build_root/protocol/schemas"
python3 "$integrity_tool" validate-tree \
  --root "$build_root/protocol/test-vectors"
python3 "$integrity_tool" validate-tree \
  --root "$build_root/fascia/apple" \
  --exclude-dir-name .build \
  --exclude-dir-name .swiftpm \
  --exclude-name Package.resolved
python3 "$integrity_tool" validate-tree --root "$build_root/studio"
python3 "$integrity_tool" validate-tree \
  --root "$build_root/sdk/apple/TohsenoCompanionKit" \
  --exclude-dir-name .build \
  --exclude-dir-name .swiftpm
python3 "$integrity_tool" validate-tree --root "$build_root/companion/test-vectors"
python3 "$integrity_tool" validate-tree --root "$build_root/dist/genesis"

genesis_source_commit="$(sed -n '1p' "$build_root/dist/genesis/SOURCE_COMMIT.txt")"
if [ "$genesis_source_commit" != "$source_commit" ]; then
  printf '%s\n' \
    "release.sh: Genesis bundle source commit disagrees with the release source." >&2
  exit 1
fi

staging_root="$(
  python3 "$integrity_tool" create-stage --repository-root "$repository_root"
)"
stage_name="${staging_root##*/}"
package="$staging_root/$target"
mkdir -p \
  "$package/bin" \
  "$package/share/billing" \
  "$package/share/protocol/schemas" \
  "$package/share/protocol/test-vectors" \
  "$package/share/fascia/apple" \
  "$package/share/studio" \
  "$package/share/sdk/apple/TohsenoCompanionKit" \
  "$package/share/companion/test-vectors" \
  "$package/share/companion/apple/TohsenoCompanion" \
  "$package/share/genesis"

cp -P "$rust_binary" "$package/bin/tohseno"
cp -P "$helper" "$package/bin/tohseno-apple-identity"
sign_release_binary "$package/bin/tohseno"
sign_release_binary "$package/bin/tohseno-apple-identity"
cp -P \
  "$build_root/protocol/schemas/"*.json \
  "$package/share/protocol/schemas/"
cp -RP \
  "$build_root/protocol/test-vectors/." \
  "$package/share/protocol/test-vectors/"
(
  cd "$build_root/fascia/apple"
  tar \
    --exclude .build \
    --exclude .swiftpm \
    --exclude Package.resolved \
    -cf - .
) | (
  cd "$package/share/fascia/apple"
  tar -xf -
)
cp -RP "$build_root/studio/." "$package/share/studio/"
if [ -f "$build_root/billing/verification-key-p256.txt" ]; then
  cp "$build_root/billing/verification-key-p256.txt" \
    "$package/share/billing/verification-key-p256.txt"
fi
(
  cd "$build_root/sdk/apple/TohsenoCompanionKit"
  tar --exclude .build --exclude .swiftpm -cf - .
) | (
  cd "$package/share/sdk/apple/TohsenoCompanionKit"
  tar -xf -
)
cp -RP \
  "$build_root/companion/test-vectors/." \
  "$package/share/companion/test-vectors/"
(
  cd "$build_root/companion/apple/TohsenoCompanion"
  tar --exclude .build --exclude .swiftpm --exclude Package.resolved -cf - .
) | (
  cd "$package/share/companion/apple/TohsenoCompanion"
  tar -xf -
)
cp -RP "$build_root/dist/genesis/." "$package/share/genesis/"

jq -n \
  --arg version "1.1.0" \
  --arg codename "COMPANION" \
  --arg target "$target" \
  --arg source_commit "$source_commit" \
  --arg source_state_sha256 "$source_state_sha256" \
  --argjson dirty "$source_dirty" \
  '{
    schema:"tohseno.release/1",
    version:$version,
    codename:$codename,
    target:$target,
    source_commit:$source_commit,
    source_state_sha256:$source_state_sha256,
    dirty:$dirty,
    channel:"stable",
    prerelease:false
  }' >"$package/RELEASE.json"

python3 "$integrity_tool" write-manifest --root "$package"

final_snapshot_commit="$(
  git -C "$build_root" rev-parse --verify 'HEAD^{commit}'
)"
final_source_state_sha256="$(
  python3 "$integrity_tool" source-state --repository-root "$build_root"
)"
if [ "$final_snapshot_commit" != "$source_commit" ] ||
  [ "$final_source_state_sha256" != "$source_state_sha256" ]; then
  printf '%s\n' \
    "release.sh: source state changed during assembly." >&2
  exit 1
fi
if [ "$source_dirty" = false ]; then
  final_snapshot_status="$(
    git -C "$build_root" status \
      --porcelain=v1 \
      --untracked-files=all \
      --ignore-submodules=none
  )"
  if [ -n "$final_snapshot_status" ]; then
    printf '%s\n' \
      "release.sh: private source snapshot changed during assembly." >&2
    exit 1
  fi
fi
(
  cd "$package"
  python3 "$integrity_tool" verify-manifest --root .
  jq -e \
    --arg source_commit "$source_commit" \
    --arg source_state_sha256 "$source_state_sha256" \
    --arg target "$target" \
    --argjson dirty "$source_dirty" \
    '.schema == "tohseno.release/1"
     and .version == "1.1.0"
     and .codename == "COMPANION"
     and .target == $target
     and .source_commit == $source_commit
     and .source_state_sha256 == $source_state_sha256
     and .dirty == $dirty
     and .channel == "stable"
     and .prerelease == false' \
    RELEASE.json >/dev/null
)

output="$(
  python3 "$integrity_tool" publish \
    --repository-root "$repository_root" \
    --stage-name "$stage_name" \
    --target "$target"
)"
printf 'assembled %s\n' "$output"
