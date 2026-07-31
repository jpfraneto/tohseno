#!/bin/sh
set -eu

mode="write"
if [ "$#" -gt 1 ]; then
  printf '%s\n' "usage: build-genesis-bundle.sh [--check]" >&2
  exit 2
fi
if [ "$#" -eq 1 ]; then
  if [ "$1" != "--check" ]; then
    printf '%s\n' "usage: build-genesis-bundle.sh [--check]" >&2
    exit 2
  fi
  mode="check"
fi

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
integrity_tool="$repository_root/scripts/release-package-integrity.py"
output_directory="$repository_root/dist/genesis"

for tool in awk cargo cp date diff dirname find git grep jq mkdir python3 rm \
  sed shasum sort; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'build-genesis-bundle.sh: required tool is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

# Only paths which feed the bundle participate in this cleanliness gate. This
# permits unrelated local notes while ensuring SOURCE_COMMIT.txt identifies
# every byte used to construct a release-candidate bundle.
if [ "${TOHSENO_ALLOW_DIRTY_BUNDLE:-0}" != "1" ] &&
  [ -n "$(
    git -C "$repository_root" status --porcelain --untracked-files=all -- \
      Cargo.lock Cargo.toml MASTER_PROMPT.md README.md WHITEPAPER.md \
      contracts/src contracts/abi contracts/bytecode contracts/deployments \
      fascia/apple genesis/GENESIS_BUNDLE.md genesis/SHOT_1_INTENT.md \
      protocol scripts/build-genesis-bundle.sh \
      scripts/release-package-integrity.py
  )" ]; then
  printf '%s\n' \
    "build-genesis-bundle.sh: commit or stash modified bundle inputs first." >&2
  exit 1
fi

source_commit="$(git -C "$repository_root" rev-parse --verify HEAD)"
source_epoch="${SOURCE_DATE_EPOCH:-$(git -C "$repository_root" show -s --format=%ct "$source_commit")}"
case "$source_epoch" in
  '' | *[!0-9]*)
    printf '%s\n' \
      "build-genesis-bundle.sh: SOURCE_DATE_EPOCH must be an unsigned integer." >&2
    exit 1
    ;;
esac

if date -u -r "$source_epoch" '+%Y-%m-%dT%H:%M:%SZ' >/dev/null 2>&1; then
  created_at="$(date -u -r "$source_epoch" '+%Y-%m-%dT%H:%M:%SZ')"
elif date -u -d "@$source_epoch" '+%Y-%m-%dT%H:%M:%SZ' >/dev/null 2>&1; then
  created_at="$(date -u -d "@$source_epoch" '+%Y-%m-%dT%H:%M:%SZ')"
else
  printf '%s\n' \
    "build-genesis-bundle.sh: date cannot format SOURCE_DATE_EPOCH." >&2
  exit 1
fi

staging_root="$(
  python3 "$integrity_tool" create-genesis-stage \
    --repository-root "$repository_root"
)"
stage_name="${staging_root##*/}"
cleanup() {
  python3 "$integrity_tool" cleanup-genesis-stage \
    --repository-root "$repository_root" \
    --stage-name "$stage_name" ||
    printf '%s\n' \
      "build-genesis-bundle.sh: could not safely remove the staging directory." >&2
}
trap cleanup EXIT HUP INT TERM

bundle="$staging_root/genesis"
mkdir -p \
  "$bundle/ABI" \
  "$bundle/bytecode" \
  "$bundle/contracts" \
  "$bundle/docs" \
  "$bundle/fascia/apple" \
  "$bundle/schemas" \
  "$bundle/test-vectors"

copy_required() {
  source_path="$1"
  destination_path="$2"
  if [ ! -f "$source_path" ] || [ -L "$source_path" ]; then
    printf 'build-genesis-bundle.sh: missing or unsafe input: %s\n' \
      "$source_path" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$destination_path")"
  cp -P "$source_path" "$destination_path"
}

copy_tree() {
  source_root="$1"
  destination_root="$2"
  if [ ! -d "$source_root" ] || [ -L "$source_root" ]; then
    printf 'build-genesis-bundle.sh: missing or unsafe input tree: %s\n' \
      "$source_root" >&2
    exit 1
  fi
  find "$source_root" -type l -print |
    while IFS= read -r link_path; do
      printf 'build-genesis-bundle.sh: symlink is not allowed in bundle input: %s\n' \
        "$link_path" >&2
      exit 1
    done
  find "$source_root" ! -type d ! -type f ! -type l -print |
    while IFS= read -r unsafe_path; do
      printf 'build-genesis-bundle.sh: unsupported bundle input type: %s\n' \
        "$unsafe_path" >&2
      exit 1
    done
  find "$source_root" -type f -print |
    LC_ALL=C sort |
    while IFS= read -r source_path; do
      relative_path="${source_path#"$source_root"/}"
      copy_required "$source_path" "$destination_root/$relative_path"
    done
}

copy_required "$repository_root/WHITEPAPER.md" "$bundle/WHITEPAPER.md"
copy_required "$repository_root/protocol/SPECIFICATION.md" "$bundle/SPECIFICATION.md"
copy_required "$repository_root/protocol/IMPLEMENTERS.md" "$bundle/IMPLEMENTERS.md"
copy_required "$repository_root/protocol/CONFORMANCE.md" "$bundle/CONFORMANCE.md"
copy_required "$repository_root/fascia/apple/FASCIA.json" "$bundle/FASCIA.json"

copy_required "$repository_root/MASTER_PROMPT.md" "$bundle/docs/MASTER_PROMPT.md"
copy_required "$repository_root/README.md" "$bundle/docs/README.md"
copy_required \
  "$repository_root/genesis/GENESIS_BUNDLE.md" \
  "$bundle/docs/GENESIS_BUNDLE.md"
copy_required \
  "$repository_root/genesis/SHOT_1_INTENT.md" \
  "$bundle/docs/SHOT_1_INTENT.md"
copy_required \
  "$repository_root/protocol/README.md" \
  "$bundle/docs/PROTOCOL_README.md"

# The Apple Fascia is a reusable reference tree, not only its manifest. Package
# manager/build state is intentionally excluded from that normative tree.
find "$repository_root/fascia/apple" \
  \( -path '*/.build' -o -path '*/.build/*' \
  -o -path '*/.swiftpm' -o -path '*/.swiftpm/*' \
  -o -name 'Package.resolved' \) -prune \
  -o -type l -print |
  while IFS= read -r link_path; do
    printf 'build-genesis-bundle.sh: symlink is not allowed in Fascia input: %s\n' \
      "$link_path" >&2
    exit 1
  done
find "$repository_root/fascia/apple" \
  \( -path '*/.build' -o -path '*/.build/*' \
  -o -path '*/.swiftpm' -o -path '*/.swiftpm/*' \
  -o -name 'Package.resolved' \) -prune \
  -o ! -type d ! -type f ! -type l -print |
  while IFS= read -r unsafe_path; do
    printf 'build-genesis-bundle.sh: unsupported Fascia input type: %s\n' \
      "$unsafe_path" >&2
    exit 1
  done
find "$repository_root/fascia/apple" \
  \( -path '*/.build' -o -path '*/.build/*' \
  -o -path '*/.swiftpm' -o -path '*/.swiftpm/*' \
  -o -name 'Package.resolved' \) -prune \
  -o -type f -print |
  LC_ALL=C sort |
  while IFS= read -r source_path; do
    relative_path="${source_path#"$repository_root/fascia/apple"/}"
    copy_required "$source_path" "$bundle/fascia/apple/$relative_path"
  done

copy_tree "$repository_root/protocol/schemas" "$bundle/schemas"
copy_tree "$repository_root/protocol/test-vectors" "$bundle/test-vectors"
copy_tree "$repository_root/contracts/src" "$bundle/contracts"
copy_tree "$repository_root/contracts/abi" "$bundle/ABI"
copy_tree "$repository_root/contracts/bytecode" "$bundle/bytecode"
copy_required \
  "$repository_root/contracts/deployments/robinhood-mainnet-next.json" \
  "$bundle/DEPLOYMENT.json"

for required_directory in \
  "$bundle/ABI" \
  "$bundle/bytecode" \
  "$bundle/contracts" \
  "$bundle/fascia/apple" \
  "$bundle/schemas" \
  "$bundle/test-vectors"; do
  if ! find "$required_directory" -type f -print -quit | grep -q .; then
    printf 'build-genesis-bundle.sh: required bundle section is empty: %s\n' \
      "$required_directory" >&2
    exit 1
  fi
done

jq -e . "$bundle/FASCIA.json" "$bundle/DEPLOYMENT.json" >/dev/null
find "$bundle/schemas" "$bundle/ABI" -type f -name '*.json' -print |
  LC_ALL=C sort |
  while IFS= read -r json_path; do
    jq -e . "$json_path" >/dev/null
  done

printf '%s\n' "$source_commit" >"$bundle/SOURCE_COMMIT.txt"

fascia_sha256="$(
  cargo run \
    --quiet \
    --locked \
    --manifest-path "$repository_root/Cargo.toml" \
    -p tohseno-protocol \
    --example fascia_commitment \
    -- "$repository_root/fascia/apple"
)"
case "$fascia_sha256" in
  0x????????????????????????????????????????????????????????????????) ;;
  *)
    printf 'build-genesis-bundle.sh: invalid Fascia commitment: %s\n' \
      "$fascia_sha256" >&2
    exit 1
    ;;
esac

file_commitments() {
  commitment_root="$1"
  (
    cd "$commitment_root"
    find . -type f -print |
      sed 's|^\./||' |
      LC_ALL=C sort |
      while IFS= read -r relative_path; do
        digest="$(shasum -a 256 "$relative_path" | awk '{print $1}')"
        jq -nc \
          --arg name "$relative_path" \
          --arg sha256 "$digest" \
          '{name:$name,sha256:$sha256}'
      done |
      jq -sc 'sort_by(.name)'
  )
}

aggregate_commitment() {
  commitment_root="$1"
  (
    cd "$commitment_root"
    find . -type f -print |
      sed 's|^\./||' |
      LC_ALL=C sort |
      while IFS= read -r relative_path; do
        digest="$(shasum -a 256 "$relative_path" | awk '{print $1}')"
        printf '%s  %s\n' "$digest" "$relative_path"
      done |
      shasum -a 256 |
      awk '{print $1}'
  )
}

schema_commitments="$(file_commitments "$bundle/schemas")"
contract_commitments="$(file_commitments "$bundle/contracts")"
abi_commitment="$(aggregate_commitment "$bundle/ABI")"
bytecode_commitment="$(aggregate_commitment "$bundle/bytecode")"
test_vector_commitment="$(aggregate_commitment "$bundle/test-vectors")"

jq -S -n \
  --arg protocol "tohseno" \
  --arg version "0.8.0" \
  --arg codename "DRAFT" \
  --arg source_commit "$source_commit" \
  --arg created_at "$created_at" \
  --arg fascia_sha256 "$fascia_sha256" \
  --arg test_vector_sha256 "$test_vector_commitment" \
  --arg abi_sha256 "$abi_commitment" \
  --arg bytecode_sha256 "$bytecode_commitment" \
  --argjson schemas "$schema_commitments" \
  --argjson contracts "$contract_commitments" \
  --slurpfile deployment "$bundle/DEPLOYMENT.json" \
  '{
    schema:"tohseno.genesis/1",
    protocol:$protocol,
    candidate_version:$version,
    codename:$codename,
    status:"protocol candidate, not canonical release",
    source_commit:$source_commit,
    created_at:$created_at,
    fascia_sha256:$fascia_sha256,
    schema_commitments:$schemas,
    contract_source_commitments:$contracts,
    contract_abi_commitment:$abi_sha256,
    contract_creation_bytecode_commitment:$bytecode_sha256,
    test_vector_commitment:$test_vector_sha256,
    deployment:$deployment[0],
    canonical_release:false
  }' >"$bundle/GENESIS.json"

python3 "$integrity_tool" write-manifest \
  --root "$bundle" \
  --manifest-name FILES.sha256

if [ "$mode" = "check" ]; then
  if [ ! -d "$output_directory" ]; then
    printf '%s\n' \
      "build-genesis-bundle.sh: dist/genesis is absent; build it before --check." >&2
    exit 1
  fi
  if ! diff -r -- "$output_directory" "$bundle"; then
    printf '%s\n' \
      "build-genesis-bundle.sh: dist/genesis does not match a fresh build." >&2
    exit 1
  fi
  printf '%s\n' "Genesis bundle is reproducible and current."
  exit 0
fi

output_directory="$(
  python3 "$integrity_tool" publish-genesis \
    --repository-root "$repository_root" \
    --stage-name "$stage_name"
)"
printf 'built %s\n' "$output_directory"
