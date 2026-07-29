#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-archive-test.XXXXXX")"

cleanup() {
  case "$temporary_root" in
    "${TMPDIR:-/tmp}"/tohseno-archive-test.*)
      if [ -d "$temporary_root" ] && [ ! -L "$temporary_root" ]; then
        rm -rf "$temporary_root"
      fi
      ;;
    *)
      printf '%s\n' \
        "test-normalized-archive.sh: refusing unsafe cleanup." >&2
      ;;
  esac
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

make_source() {
  destination="$1"
  mkdir -p "$destination/docs"
  printf '%s\n' "candidate bytes" >"$destination/docs/example.txt"
  (
    cd "$destination"
    shasum -a 256 docs/example.txt >FILES.sha256
  )
}

valid_source="$temporary_root/valid"
valid_archive="$temporary_root/valid.tar.gz"
make_source "$valid_source"
python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$valid_source" \
  --output "$valid_archive" \
  --root-name genesis \
  --mtime 1
tar -tzf "$valid_archive" >/dev/null

bad_digest_source="$temporary_root/bad-digest"
bad_digest_archive="$temporary_root/bad-digest.tar.gz"
make_source "$bad_digest_source"
printf '%064d  docs/example.txt\n' 0 >"$bad_digest_source/FILES.sha256"
printf '%s\n' "previous archive" >"$bad_digest_archive"
cp "$bad_digest_archive" "$temporary_root/previous-archive"
if python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$bad_digest_source" \
  --output "$bad_digest_archive" \
  --root-name genesis \
  --mtime 1 >/dev/null 2>&1; then
  printf '%s\n' \
    "test-normalized-archive.sh: accepted a manifest digest mismatch." >&2
  exit 1
fi
if ! cmp "$temporary_root/previous-archive" "$bad_digest_archive"; then
  printf '%s\n' \
    "test-normalized-archive.sh: replaced the prior archive after failure." >&2
  exit 1
fi

extra_source="$temporary_root/extra"
extra_archive="$temporary_root/extra.tar.gz"
make_source "$extra_source"
printf '%s\n' "unmanifested" >"$extra_source/extra.txt"
if python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$extra_source" \
  --output "$extra_archive" \
  --root-name genesis \
  --mtime 1 >/dev/null 2>&1; then
  printf '%s\n' \
    "test-normalized-archive.sh: accepted an unmanifested file." >&2
  exit 1
fi

missing_manifest_source="$temporary_root/missing-manifest"
missing_manifest_archive="$temporary_root/missing-manifest.tar.gz"
mkdir -p "$missing_manifest_source"
printf '%s\n' "unmanifested" >"$missing_manifest_source/example.txt"
if python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$missing_manifest_source" \
  --output "$missing_manifest_archive" \
  --root-name genesis \
  --mtime 1 >/dev/null 2>&1; then
  printf '%s\n' \
    "test-normalized-archive.sh: accepted a missing FILES.sha256." >&2
  exit 1
fi
if [ -e "$missing_manifest_archive" ] || [ -L "$missing_manifest_archive" ]; then
  printf '%s\n' \
    "test-normalized-archive.sh: published an archive without FILES.sha256." >&2
  exit 1
fi

symlink_source="$temporary_root/symlink"
symlink_archive="$temporary_root/symlink.tar.gz"
make_source "$symlink_source"
ln -s docs/example.txt "$symlink_source/alias.txt"
if python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$symlink_source" \
  --output "$symlink_archive" \
  --root-name genesis \
  --mtime 1 >/dev/null 2>&1; then
  printf '%s\n' \
    "test-normalized-archive.sh: accepted a symlink." >&2
  exit 1
fi

special_source="$temporary_root/special"
special_archive="$temporary_root/special.tar.gz"
make_source "$special_source"
mkfifo "$special_source/pipe"
if python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$special_source" \
  --output "$special_archive" \
  --root-name genesis \
  --mtime 1 >/dev/null 2>&1; then
  printf '%s\n' \
    "test-normalized-archive.sh: accepted a special input entry." >&2
  exit 1
fi

if python3 "$repository_root/scripts/build-normalized-tar.py" \
  --source "$valid_source" \
  --output "$temporary_root/backslash-root.tar.gz" \
  --root-name 'genesis\\escape' \
  --mtime 1 >/dev/null 2>&1; then
  printf '%s\n' \
    "test-normalized-archive.sh: accepted a backslash in the archive root." >&2
  exit 1
fi

printf '%s\n' "normalized archive regressions passed."
