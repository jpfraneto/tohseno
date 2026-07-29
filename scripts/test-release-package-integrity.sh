#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
integrity_tool="$repository_root/scripts/release-package-integrity.py"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-release-integrity.XXXXXX")"

cleanup() {
  case "$temporary_root" in
    "${TMPDIR:-/tmp}"/tohseno-release-integrity.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf -- "$temporary_root"
      fi
      ;;
    *)
      printf '%s\n' \
        "test-release-package-integrity.sh: refusing unsafe cleanup." >&2
      ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

package="$temporary_root/package"
mkdir -p "$package/bin" "$package/nested"
printf '%s\n' "candidate" >"$package/bin/tohseno"
printf '%s\n' "nested manifest name" >"$package/nested/CHECKSUMS.sha256"
python3 "$integrity_tool" write-manifest --root "$package"
python3 "$integrity_tool" verify-manifest --root "$package"
grep -F '  nested/CHECKSUMS.sha256' "$package/CHECKSUMS.sha256" >/dev/null

printf '%s\n' "extra" >"$package/extra.txt"
if python3 "$integrity_tool" verify-manifest \
  --root "$package" >/dev/null 2>&1; then
  printf '%s\n' \
    "test-release-package-integrity.sh: accepted an unmanifested file." >&2
  exit 1
fi
rm "$package/extra.txt"

ln -s bin/tohseno "$package/alias"
if python3 "$integrity_tool" write-manifest \
  --root "$package" >/dev/null 2>&1; then
  printf '%s\n' \
    "test-release-package-integrity.sh: accepted a package symlink." >&2
  exit 1
fi
rm "$package/alias"

mkfifo "$package/pipe"
if python3 "$integrity_tool" write-manifest \
  --root "$package" >/dev/null 2>&1; then
  printf '%s\n' \
    "test-release-package-integrity.sh: accepted a special package entry." >&2
  exit 1
fi
rm "$package/pipe"

escaped_repository="$temporary_root/escaped-repository"
external_directory="$temporary_root/external"
mkdir -p "$escaped_repository" "$external_directory"
ln -s "$external_directory" "$escaped_repository/dist"
if python3 "$integrity_tool" create-stage \
  --repository-root "$escaped_repository" >/dev/null 2>&1; then
  printf '%s\n' \
    "test-release-package-integrity.sh: followed a dist symlink." >&2
  exit 1
fi
if [ -e "$external_directory/release-candidate" ]; then
  printf '%s\n' \
    "test-release-package-integrity.sh: wrote through a dist symlink." >&2
  exit 1
fi

publish_repository="$temporary_root/publish-repository"
mkdir -p "$publish_repository"
first_stage="$(
  python3 "$integrity_tool" create-stage \
    --repository-root "$publish_repository"
)"
first_stage_name="${first_stage##*/}"
first_package="$first_stage/aarch64-apple-darwin"
mkdir "$first_package"
printf '%s\n' "first" >"$first_package/payload"
python3 "$integrity_tool" write-manifest --root "$first_package"
python3 "$integrity_tool" publish \
  --repository-root "$publish_repository" \
  --stage-name "$first_stage_name" \
  --target aarch64-apple-darwin >/dev/null
grep -Fx 'first' \
  "$publish_repository/dist/release-candidate/aarch64-apple-darwin/payload" \
  >/dev/null
python3 "$integrity_tool" cleanup-stage \
  --repository-root "$publish_repository" \
  --stage-name "$first_stage_name"

second_stage="$(
  python3 "$integrity_tool" create-stage \
    --repository-root "$publish_repository"
)"
second_stage_name="${second_stage##*/}"
second_package="$second_stage/aarch64-apple-darwin"
mkdir "$second_package"
printf '%s\n' "second" >"$second_package/payload"
python3 "$integrity_tool" write-manifest --root "$second_package"
python3 "$integrity_tool" publish \
  --repository-root "$publish_repository" \
  --stage-name "$second_stage_name" \
  --target aarch64-apple-darwin >/dev/null
grep -Fx 'second' \
  "$publish_repository/dist/release-candidate/aarch64-apple-darwin/payload" \
  >/dev/null
grep -Fx 'first' "$second_package/payload" >/dev/null
python3 "$integrity_tool" cleanup-stage \
  --repository-root "$publish_repository" \
  --stage-name "$second_stage_name"
if [ -e "$second_stage" ] || [ -L "$second_stage" ]; then
  printf '%s\n' \
    "test-release-package-integrity.sh: did not remove the swapped old package." >&2
  exit 1
fi

genesis_stage="$(
  python3 "$integrity_tool" create-genesis-stage \
    --repository-root "$publish_repository"
)"
genesis_stage_name="${genesis_stage##*/}"
genesis_bundle="$genesis_stage/genesis"
mkdir "$genesis_bundle"
printf '%s\n' "genesis" >"$genesis_bundle/payload"
python3 "$integrity_tool" write-manifest \
  --root "$genesis_bundle" \
  --manifest-name FILES.sha256
python3 "$integrity_tool" publish-genesis \
  --repository-root "$publish_repository" \
  --stage-name "$genesis_stage_name" >/dev/null
grep -Fx 'genesis' "$publish_repository/dist/genesis/payload" >/dev/null
python3 "$integrity_tool" cleanup-genesis-stage \
  --repository-root "$publish_repository" \
  --stage-name "$genesis_stage_name"

printf '%s\n' "release package integrity regressions passed."
