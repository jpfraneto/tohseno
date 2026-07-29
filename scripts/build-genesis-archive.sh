#!/bin/sh
set -eu

if [ "$#" -ne 1 ] || [ -z "$1" ]; then
  printf '%s\n' \
    "usage: build-genesis-archive.sh OUTPUT.tar.gz" >&2
  exit 2
fi

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
integrity_tool="$repository_root/scripts/release-package-integrity.py"
bundle="$repository_root/dist/genesis"
output="$1"

for tool in git python3 sed shasum; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'build-genesis-archive.sh: required tool is missing: %s\n' \
      "$tool" >&2
    exit 1
  fi
done

if [ -L "$repository_root/dist" ] ||
  [ ! -d "$repository_root/dist" ] ||
  [ "$(CDPATH= cd -- "$repository_root/dist" && pwd -P)" != "$repository_root/dist" ] ||
  [ ! -d "$bundle" ] ||
  [ -L "$bundle" ]; then
  printf '%s\n' \
    "build-genesis-archive.sh: build dist/genesis before archiving it." >&2
  exit 1
fi

source_commit="$(git -C "$repository_root" rev-parse --verify HEAD)"
if [ "$(sed -n '1p' "$bundle/SOURCE_COMMIT.txt")" != "$source_commit" ]; then
  printf '%s\n' \
    "build-genesis-archive.sh: dist/genesis does not match HEAD." >&2
  exit 1
fi
python3 "$integrity_tool" verify-manifest \
  --root "$bundle" \
  --manifest-name FILES.sha256
(
  cd "$bundle"
  shasum -a 256 -c FILES.sha256 >/dev/null
)

source_epoch="$(
  if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
    printf '%s\n' "$SOURCE_DATE_EPOCH"
  else
    git -C "$repository_root" show -s --format=%ct "$source_commit"
  fi
)"
case "$source_epoch" in
  '' | *[!0-9]*)
    printf '%s\n' \
      "build-genesis-archive.sh: SOURCE_DATE_EPOCH must be an unsigned integer." >&2
    exit 1
    ;;
esac

python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$bundle" \
  --output "$output" \
  --root-name genesis \
  --mtime "$source_epoch"

printf 'assembled %s\n' "$output"
