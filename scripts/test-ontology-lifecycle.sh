#!/bin/sh
set -eu
umask 077

script_name="test-ontology-lifecycle.sh"
smoke_root=""
smoke_parent=""
smoke_marker=""
control_root=""
lifecycle_log=""
expected_key_tag=""
identity_helper=""
node_bin=""

fail() {
  printf '%s: %s\n' "$script_name" "$1" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "required command is unavailable: $1"
  fi
}

require_regular_executable() {
  if [ -L "$1" ] || [ ! -f "$1" ] || [ ! -x "$1" ]; then
    fail "$2 must be an executable, non-symlink regular file: $1"
  fi
}

resolve_regular_executable() {
  executable_path=$1
  executable_label=$2
  case "$executable_path" in
    /*) ;;
    *) fail "$executable_label must be an absolute path: $executable_path" ;;
  esac
  require_regular_executable "$executable_path" "$executable_label"
  executable_directory=$(
    CDPATH= cd -- "$(dirname -- "$executable_path")" >/dev/null 2>&1
    pwd -P
  ) || fail "could not resolve $executable_label"
  printf '%s/%s\n' "$executable_directory" "$(basename -- "$executable_path")"
}

run_logged() {
  run_label=$1
  shift
  printf '%s\n' "→ $run_label"
  if "$@" >>"$lifecycle_log" 2>&1; then
    return 0
  else
    run_status=$?
    fail "$run_label failed with exit $run_status (log: $lifecycle_log)"
  fi
}

run_json() {
  run_label=$1
  run_output=$2
  shift 2
  printf '%s\n' "→ $run_label"
  if "$@" >"$run_output" 2>>"$lifecycle_log"; then
    if jq -e . "$run_output" >/dev/null 2>&1; then
      return 0
    fi
    fail "$run_label returned non-JSON output (output: $run_output)"
  else
    run_status=$?
    fail "$run_label failed with exit $run_status (log: $lifecycle_log)"
  fi
}

one_version_record() {
  version_root=$1
  version_ordinal=$2
  version_count=$(
    find "$version_root" \
      -type f \
      -path "*/$version_ordinal/version.json" \
      -print |
      awk 'END { print NR + 0 }'
  )
  if [ "$version_count" -ne 1 ]; then
    fail "expected one immutable version $version_ordinal record, found $version_count"
  fi
  find "$version_root" \
    -type f \
    -path "*/$version_ordinal/version.json" \
    -print |
    awk 'NR == 1 { print; exit }'
}

cleanup() {
  lifecycle_status=$?
  trap - EXIT HUP INT TERM
  cleanup_failed=0

  if [ -n "$expected_key_tag" ] &&
    [ -n "$identity_helper" ] &&
    [ -x "$identity_helper" ] &&
    [ -n "$control_root" ] &&
    [ -d "$control_root" ]; then
    cleanup_public="$control_root/cleanup-identity-public.json"
    cleanup_error="$control_root/cleanup-identity-error.json"
    if "$identity_helper" public --tag "$expected_key_tag" \
      >"$cleanup_public" 2>"$cleanup_error"; then
      if ! jq -e \
        --arg tag "$expected_key_tag" \
        '.ok == true
          and .result.tag == $tag
          and .result.backend == "software_test"
          and .result.test_only == true' \
        "$cleanup_public" >/dev/null; then
        printf '%s: refusing to delete a key that is not the expected software-test identity: %s\n' \
          "$script_name" "$expected_key_tag" >&2
        cleanup_failed=1
      elif ! "$identity_helper" delete --tag "$expected_key_tag" \
        >"$control_root/cleanup-identity-delete.json" \
        2>>"$lifecycle_log"; then
        printf '%s: could not delete software-test identity %s\n' \
          "$script_name" "$expected_key_tag" >&2
        cleanup_failed=1
      elif ! jq -e \
        --arg tag "$expected_key_tag" \
        '.ok == true
          and .result.tag == $tag
          and .result.backend == "software_test"
          and .result.test_only == true
          and .result.deleted == true' \
        "$control_root/cleanup-identity-delete.json" >/dev/null; then
        printf '%s: identity helper returned an invalid deletion receipt for %s\n' \
          "$script_name" "$expected_key_tag" >&2
        cleanup_failed=1
      fi
    elif ! jq -e \
      '.ok == false and .error.code == "identity_not_found"' \
      "$cleanup_error" >/dev/null 2>&1; then
      printf '%s: could not establish cleanup state for software-test identity %s\n' \
        "$script_name" "$expected_key_tag" >&2
      cleanup_failed=1
    fi
  fi

  if [ "$lifecycle_status" -ne 0 ] && [ -f "$lifecycle_log" ]; then
    printf '%s\n' "---- lifecycle log (last 100 lines) ----" >&2
    tail -n 100 "$lifecycle_log" >&2 || true
  fi

  keep_root=${TOHSENO_KEEP_SMOKE_ROOT:-0}
  if [ "$lifecycle_status" -ne 0 ] || [ "$cleanup_failed" -ne 0 ]; then
    keep_root=1
  fi
  if [ "$keep_root" = "1" ]; then
    if [ -n "$smoke_root" ]; then
      printf '%s\n' "retained isolated smoke root: $smoke_root" >&2
    fi
  elif [ "$keep_root" = "0" ]; then
    root_parent=""
    root_name=""
    if [ -n "$smoke_root" ] && [ -d "$smoke_root" ] && [ ! -L "$smoke_root" ]; then
      root_parent=$(
        CDPATH= cd -- "$(dirname -- "$smoke_root")" >/dev/null 2>&1
        pwd -P
      ) || root_parent=""
      root_name=$(basename -- "$smoke_root")
    fi
    case "$root_name" in
      tohseno-ontology-lifecycle.*) ;;
      *) root_name="" ;;
    esac
    if [ -n "$root_name" ] &&
      [ "$root_parent" = "$smoke_parent" ] &&
      [ -f "$smoke_marker" ] &&
      [ ! -L "$smoke_marker" ] &&
      [ "$(sed -n '1p' "$smoke_marker")" = "tohseno-ontology-lifecycle-smoke-v1" ] &&
      [ "$(wc -l <"$smoke_marker" | tr -d ' ')" = "1" ]; then
      if ! rm -rf -- "$smoke_root"; then
        printf '%s: could not remove verified smoke root: %s\n' \
          "$script_name" "$smoke_root" >&2
        cleanup_failed=1
      fi
    else
      printf '%s: refusing to remove an unverified smoke root: %s\n' \
        "$script_name" "$smoke_root" >&2
      cleanup_failed=1
    fi
  else
    printf '%s: TOHSENO_KEEP_SMOKE_ROOT must be 0 or 1\n' "$script_name" >&2
    cleanup_failed=1
  fi

  if [ "$lifecycle_status" -eq 0 ] && [ "$cleanup_failed" -ne 0 ]; then
    lifecycle_status=1
  fi
  exit "$lifecycle_status"
}

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

repository_root=$(
  CDPATH= cd -- "$(dirname -- "$0")/.." >/dev/null 2>&1
  pwd -P
) || fail "could not resolve the repository root"

if [ "$(uname -s)" != "Darwin" ]; then
  fail "the ontology lifecycle smoke requires macOS and Xcode"
fi

for required_command in \
  awk \
  cmp \
  cut \
  defaults \
  find \
  grep \
  jq \
  mktemp \
  openssl \
  python3 \
  security \
  sed \
  shasum \
  tail \
  tr \
  uname \
  wc \
  xcodebuild
do
  require_command "$required_command"
done

candidate_input=${TOHSENO_CANDIDATE_BIN:-"$repository_root/target/debug/tohseno"}
identity_input=${TOHSENO_APPLE_IDENTITY_HELPER:-"$repository_root/apple-identity/.build/debug/tohseno-apple-identity"}
node_input=${TOHSENO_NODE_BIN:-"$repository_root/target/debug/tohseno-node"}
candidate_bin=$(resolve_regular_executable "$candidate_input" "TOHSENO_CANDIDATE_BIN")
identity_helper=$(resolve_regular_executable "$identity_input" "TOHSENO_APPLE_IDENTITY_HELPER")
node_bin=$(resolve_regular_executable "$node_input" "TOHSENO_NODE_BIN")

candidate_version=$("$candidate_bin" --version 2>/dev/null) ||
  fail "the candidate executable did not start"
if [ "$candidate_version" != "tohseno 0.8.4" ]; then
  fail "candidate reported '$candidate_version', expected 'tohseno 0.8.4'"
fi
helper_version=$("$identity_helper" --version 2>/dev/null) ||
  fail "the Apple identity helper did not start"
if [ "$helper_version" != "tohseno-apple-identity 0.8.4" ]; then
  fail "identity helper reported '$helper_version', expected 'tohseno-apple-identity 0.8.4'"
fi
node_version=$("$node_bin" --version 2>/dev/null) ||
  fail "the node executable did not start"
if [ "$node_version" != "tohseno-node 0.8.4" ]; then
  fail "node reported '$node_version', expected 'tohseno-node 0.8.4'"
fi

fixture_materializer="$repository_root/engine/fixtures/apple-expression/materialize.sh"
require_regular_executable "$fixture_materializer" "Apple expression fixture materializer"
fixture_conceptor="$repository_root/engine/fixtures/apple-expression/prepare-birth-fixture.py"
require_regular_executable "$fixture_conceptor" "deterministic conception fixture"
fixture_exerciser="$repository_root/engine/fixtures/apple-expression/exercise-birth.sh"
require_regular_executable "$fixture_exerciser" "Apple expression experience fixture"

temp_input=${TMPDIR:-/tmp}
case "$temp_input" in
  /*) ;;
  *) fail "TMPDIR must be absolute" ;;
esac
smoke_parent=$(
  CDPATH= cd -- "$temp_input" >/dev/null 2>&1
  pwd -P
) || fail "could not resolve TMPDIR"
if [ -L "$smoke_parent" ] || [ ! -d "$smoke_parent" ]; then
  fail "resolved TMPDIR must be a real directory"
fi
smoke_root=$(mktemp -d "$smoke_parent/tohseno-ontology-lifecycle.XXXXXX") ||
  fail "could not create an isolated smoke root"
if [ -L "$smoke_root" ] || [ ! -d "$smoke_root" ]; then
  fail "mktemp did not create a real isolated directory"
fi
smoke_marker="$smoke_root/.tohseno-ontology-smoke-root"
printf '%s\n' "tohseno-ontology-lifecycle-smoke-v1" >"$smoke_marker"
control_root="$smoke_root/control"
mkdir "$control_root"
lifecycle_log="$control_root/lifecycle.log"
: >"$lifecycle_log"
# The isolated root and control directory are already mode 0700. TOHSENO's
# public Shot-body files deliberately require 0644, so do not mask those bits
# from the engine and fixture materializer.
umask 022

export TOHSENO_DATA_ROOT="$smoke_root"
export TOHSENO_IDENTITY_BACKEND="software-test"
export TOHSENO_APPLE_IDENTITY_HELPER="$identity_helper"

identity_root="$TOHSENO_DATA_ROOT/identity"
key_hash=$(
  {
    printf 'TOHSENO-LOCAL-KEY-TAG-V1\000'
    printf '%s' "$identity_root"
  } | shasum -a 256
)
key_hash=${key_hash%% *}
case "$key_hash" in
  [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*)
    ;;
  *) fail "could not derive the isolated identity tag" ;;
esac
expected_key_tag="org.tohseno.builder.device.$(printf '%s' "$key_hash" | cut -c 1-32)"

if "$identity_helper" public --tag "$expected_key_tag" \
  >"$control_root/preexisting-identity.json" \
  2>"$control_root/preexisting-identity-error.json"; then
  fail "the unique smoke identity tag unexpectedly exists: $expected_key_tag"
fi
if ! jq -e \
  '.ok == false and .error.code == "identity_not_found"' \
  "$control_root/preexisting-identity-error.json" >/dev/null; then
  fail "the identity helper could not prove the smoke tag is unused"
fi

if ! xcodebuild -version >>"$lifecycle_log" 2>&1; then
  fail "xcodebuild is not ready; install Xcode, accept its license, and select it with xcode-select"
fi
signing_identities=$(security find-identity -v -p codesigning 2>>"$lifecycle_log") ||
  fail "could not inspect local Apple signing identities"
case "$signing_identities" in
  *'"Apple Development:'*) ;;
  *) fail "no Apple Development signing identity is available; recording would wait indefinitely" ;;
esac
xcode_accounts=$(
  defaults read com.apple.dt.Xcode IDEProvisioningTeamByIdentifier 2>>"$lifecycle_log"
) || fail "Xcode has no provisioning team configuration; recording would wait indefinitely"
xcode_teams_file="$control_root/xcode-teams.txt"
printf '%s\n' "$xcode_accounts" |
  awk '/teamID = / {
    value = $0
    sub(/^.*teamID = /, "", value)
    sub(/;.*$/, "", value)
    if (value ~ /^[A-Za-z0-9]+$/) print value
  }' |
  awk '!seen[$0]++' >"$xcode_teams_file"
if [ ! -s "$xcode_teams_file" ]; then
  fail "Xcode provisioning settings contain no bounded team ID"
fi

xcode_team_match=0
while IFS= read -r xcode_team; do
  case "$signing_identities" in
    *"($xcode_team)\""*) xcode_team_match=1 ;;
  esac
done <"$xcode_teams_file"

if [ "$xcode_team_match" -ne 1 ]; then
  certificates_file="$control_root/apple-development-certificates.pem"
  certificate_root="$control_root/apple-development-certificates"
  certificate_teams_file="$control_root/apple-development-certificate-teams.txt"
  mkdir "$certificate_root"
  if security find-certificate -a -c "Apple Development" -p \
    >"$certificates_file" 2>>"$lifecycle_log"; then
    awk -v directory="$certificate_root" '
      /-----BEGIN CERTIFICATE-----/ {
        count += 1
        output = sprintf("%s/certificate-%03d.pem", directory, count)
      }
      output != "" { print >output }
      /-----END CERTIFICATE-----/ {
        close(output)
        output = ""
      }
    ' "$certificates_file"
    : >"$certificate_teams_file"
    for certificate_file in "$certificate_root"/*.pem; do
      if [ ! -f "$certificate_file" ] || [ -L "$certificate_file" ]; then
        continue
      fi
      if certificate_subject=$(
        openssl x509 -noout -subject -in "$certificate_file" 2>>"$lifecycle_log"
      ); then
        printf '%s\n' "$certificate_subject" |
          tr '/,' '\n\n' |
          awk -F '=' '
            {
              key = $1
              value = $2
              gsub(/[[:space:]]/, "", key)
              gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
              if (key == "OU" && value ~ /^[A-Za-z0-9]+$/) print value
            }
          ' >>"$certificate_teams_file"
      fi
    done
    if [ -s "$certificate_teams_file" ] &&
      awk '
        NR == FNR { xcode[$0] = 1; next }
        xcode[$0] { matched = 1 }
        END { exit(matched ? 0 : 1) }
      ' "$xcode_teams_file" "$certificate_teams_file"; then
      xcode_team_match=1
    fi
  fi
fi
if [ "$xcode_team_match" -ne 1 ]; then
  fail "no Apple Development identity matches an Xcode provisioning team; recording would wait indefinitely"
fi

app_name="ontologysmoke"
shot_root="$TOHSENO_DATA_ROOT/$app_name"
intention_file="$control_root/intention.md"
printf '%s\n' \
  "# Coherent Intention" \
  "" \
  "Show a complete, quiet native iPhone expression with a clear visible identity." \
  "The primary screen must state that it is a TOHSENO expression and that the Apple materialization gates passed." \
  >"$intention_file"

run_logged \
  "capture the exact intention and prepare intelligent conception" \
  "$candidate_bin" create "$app_name" \
  --prompt-file "$intention_file" \
  --no-launch

conception_output="$control_root/conception-output.json"
run_logged \
  "derive the deterministic app-specific conception fixture" \
  "$fixture_conceptor" conception \
  "$shot_root/.tohseno/private/planning/conception-input.json" \
  "$conception_output"
run_logged \
  "validate and accept the intelligence-shaped Genome, Birth Plan, and Experience Contract" \
  "$candidate_bin" create "$app_name" \
  --prompt-file "$intention_file" \
  --accept-genome \
  --conception-file "$conception_output" \
  --no-launch

builder_file="$TOHSENO_DATA_ROOT/identity/builder.json"
if [ -L "$builder_file" ] || [ ! -f "$builder_file" ]; then
  fail "creation did not persist a safe builder identity descriptor"
fi
if ! jq -e \
  --arg tag "$expected_key_tag" \
  '.schema == "tohseno.builder/1"
    and .local_key_tag == $tag
    and .key_backend == "software_test"
    and .security_level == "software_test"
    and .test_only == true' \
  "$builder_file" >/dev/null; then
  fail "creation did not use the explicit software-test identity"
fi

if [ -L "$shot_root" ] || [ ! -d "$shot_root" ]; then
  fail "creation did not produce a real Shot directory"
fi
if ! cmp -s "$intention_file" "$shot_root/INTENTION.md"; then
  fail "the preserved intention differs from the exact human source"
fi
for required_path in \
  "$shot_root/GENOME.md" \
  "$shot_root/EVOLUTIONARY_INTENT.md" \
  "$shot_root/.gitignore" \
  "$shot_root/.tohseno/shot.json" \
  "$shot_root/.tohseno/genome.json" \
  "$shot_root/.tohseno/expression.json" \
  "$shot_root/.tohseno/capabilities.lock" \
  "$shot_root/.tohseno/lineage.jsonl" \
  "$shot_root/.tohseno/private/planning" \
  "$shot_root/feedback/versions" \
  "$shot_root/versions"
do
  if [ ! -e "$required_path" ] || [ -L "$required_path" ]; then
    fail "created Shot is missing a safe ontology body path: $required_path"
  fi
done
for private_ignore in \
  "INTENTION.md" \
  "GENOME.md" \
  "EVOLUTIONARY_INTENT.md" \
  "feedback/" \
  "versions/" \
  ".tohseno/lineage.jsonl" \
  ".tohseno/shot.json" \
  ".tohseno/private/"
do
  if ! grep -F -x "$private_ignore" "$shot_root/.gitignore" >/dev/null; then
    fail "generated source-publication boundary does not ignore: $private_ignore"
  fi
done

app_record="$shot_root/.tohseno/app.toml"
if [ -L "$app_record" ] || [ ! -f "$app_record" ]; then
  fail "created Shot has no safe app.toml compatibility record"
fi
bundle_id=$(
  awk -F '"' '
    /^bundle_id = "[A-Za-z0-9.-]+"$/ {
      count += 1
      value = $2
    }
    END {
      if (count == 1) print value
    }
  ' "$app_record"
)
if [ -z "$bundle_id" ]; then
  fail "could not resolve the generated Apple bundle identifier"
fi

run_logged \
  "materialize the deterministic native Apple fixture" \
  "$fixture_materializer" "$shot_root" "$app_name" "$bundle_id"
run_logged \
  "exercise the bounded target-user journey and bind its evidence" \
  "$fixture_exerciser" "$shot_root" "$app_name"

printf '%s\n' "→ accept and record immutable version 0001"
if "$candidate_bin" evolve "$app_name" </dev/null >>"$lifecycle_log" 2>&1; then
  :
else
  record_status=$?
  fail "recording version 0001 failed with exit $record_status (log: $lifecycle_log)"
fi

version_1_path=$(one_version_record "$shot_root/versions" "0001")
if ! jq -e '.schema == "tohseno.version/2" and .ordinal == 1' \
  "$version_1_path" >/dev/null; then
  fail "version 0001 is not a canonical immutable Version record"
fi
if [ ! -f "$shot_root/.tohseno/evolutions/0001/.complete" ]; then
  fail "the v1 compatibility Evolution 0001 completion marker is absent"
fi
version_1_id=$(jq -er '.version_id' "$version_1_path")
version_1_digest=$(shasum -a 256 "$version_1_path" | awk '{ print $1 }')

feedback_attachment="$control_root/experienced-state.txt"
printf '%s\n' \
  "Experienced version: 0001" \
  "Observation: continuity is clear; make the next state explicit in the expression." \
  >"$feedback_attachment"
feedback_output="$control_root/feedback.json"
run_json \
  "attach private feedback to exact version 0001" \
  "$feedback_output" \
  "$candidate_bin" --json feedback "$app_name" \
  --version 1 \
  --text "Keep the calm local experience and make continuity into the next state visible." \
  --attachment "$feedback_attachment"
if ! jq -e \
  '.version_ordinal == 1
    and .visibility == "intentionally_private"
    and (.feedback_id | startswith("0x"))
    and (.action_commitment | startswith("0x"))
    and (.attachments | length) == 1' \
  "$feedback_output" >/dev/null; then
  fail "feedback output did not prove exact-version private attachment"
fi
feedback_action=$(jq -er '.action_commitment' "$feedback_output")
case "$feedback_action" in
  0x[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*)
    if [ "${#feedback_action}" -ne 66 ]; then
      fail "Feedback action commitment has the wrong length"
    fi
    ;;
  *) fail "Feedback output did not expose a lowercase signed action commitment" ;;
esac
feedback_directory=$(jq -er '.directory' "$feedback_output")
case "$feedback_directory/" in
  "$shot_root"/feedback/versions/*/0001/*/) ;;
  *) fail "feedback was stored outside version 0001 private feedback storage" ;;
esac
if [ -L "$feedback_directory" ] || [ ! -d "$feedback_directory" ]; then
  fail "feedback directory is not a real directory"
fi

evolutionary_intent="$shot_root/EVOLUTIONARY_INTENT.md"
printf '%s\n' \
  "# Evolutionary Intent" \
  "" \
  "Current version: 0001" \
  "" \
  "Feedback and references:" \
  "- Use the private feedback attached to version 0001." \
  "- Preserve the exact Shot, expression, and accepted Genome identity." \
  "" \
  "Desired changes:" \
  "- Make continuity into the next accepted state explicit in the fixture's primary message." \
  "" \
  "Invariants to preserve:" \
  "- Keep the native, local-first, private experience." \
  "- Do not mutate the accepted Genome." \
  "" \
  "Proposed genome mutations:" \
  "- None." \
  >"$evolutionary_intent"

run_logged \
  "stage the explicit evolutionary intent without materializing a version" \
  "$candidate_bin" evolve "$app_name" \
  --prompt-file "$evolutionary_intent" \
  --feedback-action "$feedback_action" \
  --no-launch

version_count_before_change=$(
  find "$shot_root/versions" -type f -name version.json -print |
    awk 'END { print NR + 0 }'
)
if [ "$version_count_before_change" -ne 1 ]; then
  fail "staging an evolutionary intent unexpectedly accepted a new Version"
fi

source_file="$shot_root/TemplateApp.swift"
if [ -L "$source_file" ] || [ ! -f "$source_file" ]; then
  fail "materialized fixture source is missing or unsafe"
fi
original_sentence="This fixture passes the real Apple materialization gates."
replacement_sentence="Version 0002 keeps the exact Shot identity and makes continuity visible."
sentence_count=$(
  awk -v sentence="$original_sentence" '
    index($0, sentence) { count += 1 }
    END { print count + 0 }
  ' "$source_file"
)
if [ "$sentence_count" -ne 1 ]; then
  fail "the deterministic source mutation target occurred $sentence_count times, expected once"
fi
source_stage=$(mktemp "$control_root/TemplateApp.swift.XXXXXX") ||
  fail "could not stage the deterministic source mutation"
sed "s|$original_sentence|$replacement_sentence|" "$source_file" >"$source_stage"
chmod 0644 "$source_stage"
mv "$source_stage" "$source_file"

printf '%s\n' "→ materialize and accept immutable version 0002"
if "$candidate_bin" evolve "$app_name" </dev/null >>"$lifecycle_log" 2>&1; then
  :
else
  record_status=$?
  fail "recording version 0002 failed with exit $record_status (log: $lifecycle_log)"
fi

version_2_path=$(one_version_record "$shot_root/versions" "0002")
if ! jq -e '.schema == "tohseno.version/2" and .ordinal == 2' \
  "$version_2_path" >/dev/null; then
  fail "version 0002 is not a canonical immutable Version record"
fi
if [ ! -f "$shot_root/.tohseno/evolutions/0002/.complete" ]; then
  fail "the v1 compatibility Evolution 0002 completion marker is absent"
fi
version_2_id=$(jq -er '.version_id' "$version_2_path")
if [ "$version_1_id" = "$version_2_id" ]; then
  fail "versions 0001 and 0002 collapsed to one VersionID"
fi
if [ "$(shasum -a 256 "$version_1_path" | awk '{ print $1 }')" != "$version_1_digest" ]; then
  fail "accepting version 0002 mutated immutable version 0001"
fi
if ! cmp -s "$intention_file" "$shot_root/INTENTION.md"; then
  fail "evolution mutated the preserved original intention"
fi
if ! jq -e \
  --slurpfile first "$version_1_path" \
  '.expression_id == $first[0].expression_id
    and .genome_revision == $first[0].genome_revision
    and .genome_digest == $first[0].genome_digest
    and .source_digest != $first[0].source_digest' \
  "$version_2_path" >/dev/null; then
  fail "version 0002 did not preserve expression/Genome continuity across a source change"
fi

embedded_metadata="$shot_root/TOHSENO/embedded-provenance.json"
if [ -L "$embedded_metadata" ] || [ ! -f "$embedded_metadata" ]; then
  fail "the current Apple expression has no safe embedded identity resource"
fi
if ! jq -e \
  --slurpfile version "$version_2_path" \
  --slurpfile snapshot "$shot_root/.tohseno/shot.json" \
  '.schema == "tohseno.app-metadata/2"
    and .protocol_version == "2"
    and .shot_id == $snapshot[0].shot_id
    and .expression_id == $version[0].expression_id
    and .version_id == $version[0].version_id
    and .version_ordinal == 2
    and .bundle_version == 2
    and .genome_revision == $version[0].genome_revision
    and .genome_digest == $version[0].genome_digest
    and .source_tree_sha256 == $version[0].source_digest' \
  "$embedded_metadata" >/dev/null; then
  fail "embedded Apple metadata is not bound to accepted version 0002"
fi

lineage_file="$shot_root/.tohseno/lineage.jsonl"
lineage_before_public=$(
  shasum -a 256 "$lineage_file" | awk '{print $1}'
)
printf '%s\n' "→ reject mixed-ancestry public Token Association export"
if "$candidate_bin" --json token associate "$app_name" \
  8453 \
  0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7 \
  --symbol ANKY \
  --public >>"$lifecycle_log" 2>&1; then
  fail "mixed-ancestry public Token Association export unexpectedly succeeded"
fi
lineage_after_public=$(
  shasum -a 256 "$lineage_file" | awk '{print $1}'
)
if [ "$lineage_after_public" != "$lineage_before_public" ]; then
  fail "rejected public Token Association mutated canonical lineage"
fi
if [ -e "$shot_root/.tohseno/private/public-action-outbox" ]; then
  fail "rejected public Token Association created a legacy outbox"
fi

token_output="$control_root/token-association.json"
run_json \
  "sign a private optional Token Association for a mock Base contract" \
  "$token_output" \
  "$candidate_bin" --json token associate "$app_name" \
  8453 \
  0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7 \
  --symbol ANKY
if ! jq -e \
  '.schema == "tohseno.cli-token-association/1"
    and .operation == "associate"
    and .chain_id == 8453
    and .token_address == "0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7"
    and .symbol == "ANKY"
    and .availability == "intentionally_private"
    and .shot_identity_changed == false
    and .ownership_changed == false
    and .chain_anchor_verified == false
    and .relayed == false
    and (.action_commitment | startswith("0x"))
    and .outbox_path == null' \
  "$token_output" >/dev/null; then
  fail "Token Association output conflated the token with identity, ownership, anchor, or relay"
fi
token_shot_id=$(jq -er '.shot_id' "$token_output")
snapshot_shot_id=$(jq -er '.shot_id' "$shot_root/.tohseno/shot.json")
if [ "$token_shot_id" != "$snapshot_shot_id" ]; then
  fail "Token Association changed or detached from the stable ShotID"
fi
if ! jq -s -e \
  --arg version1 "$version_1_id" \
  --arg version2 "$version_2_id" \
  --arg feedback_action "$feedback_action" \
  '
    . as $actions
    | ($actions | length) > 0
      and ([$actions[].action.sequence]
        == [range(1; (($actions | length) + 1))])
      and all($actions[]; .signature.low_s == true)
      and (([$actions[].action.shot_id] | unique | length) == 1)
      and (([
        "commitment",
        "intention",
        "genome_proposal",
        "genome_acceptance",
        "expression",
        "organ",
        "verification_result",
        "version",
        "feedback",
        "evolutionary_intent",
        "evolution",
        "token_association"
      ] - [$actions[].action.payload.type]) | length == 0)
      and ([
        $actions[]
        | select(.action.payload.type == "version")
        | .action.payload.ordinal
      ] == [1, 2])
      and ([
        $actions[]
        | select(.action.payload.type == "feedback")
        | .action.payload.version_id
      ] == [$version1])
      and any(
        $actions[];
        .action.payload.type == "feedback"
          and .action.availability == "intentionally_private"
      )
      and any(
        $actions[];
        .action.payload.type == "evolutionary_intent"
          and .action.payload.from_version_id == $version1
          and .action.payload.feedback_actions == [$feedback_action]
      )
      and any(
        $actions[];
        .action.payload.type == "evolution"
          and .action.payload.from_version_id == $version1
          and .action.payload.to_version_id == $version2
          and .action.payload.from_genome_digest
            == .action.payload.to_genome_digest
      )
      and any(
        $actions[];
        .action.payload.type == "token_association"
          and .action.payload.operation == "associate"
          and .action.payload.chain_id == 8453
          and .action.payload.token
            == "0xa7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7"
          and .action.availability == "intentionally_private"
      )
  ' "$lineage_file" >/dev/null; then
  fail "canonical lineage does not prove the complete 0001 → feedback → 0002 continuity"
fi

local_verification="$control_root/local-verification.json"
run_json \
  "verify the local Shot and reconstruct its lineage" \
  "$local_verification" \
  "$candidate_bin" --json verify "$app_name"
if ! jq -e \
  '.schema == "tohseno.cli-verification/1"
    and .conformant == true
    and .local.report.shot_body.selected_version_id != null' \
  "$local_verification" >/dev/null; then
  fail "local verification did not report a conformant Shot body at version 0002"
fi

portable_bundle="$control_root/private-shot-bundle"
export_output="$control_root/export.json"
run_json \
  "export the verified private portable Shot record bundle" \
  "$export_output" \
  "$candidate_bin" --json export "$app_name" \
  --output "$portable_bundle" \
  --include-private
if ! jq -e \
  '.manifest.visibility == "include_private"
    and .manifest.materialization_ready == false
    and .manifest.intention_bytes == "intentionally_private"
    and .manifest.feedback_bytes == "intentionally_private"
    and .manifest.action_count > 0
    and (.manifest.omitted_artifacts | index("expression_source")) != null
    and (.manifest.omitted_artifacts | index("owner_private_keys")) != null' \
  "$export_output" >/dev/null; then
  fail "private export did not state its visibility and non-materialization boundary honestly"
fi

imported_root="$control_root/imported-shot"
import_output="$control_root/import.json"
run_json \
  "verify and import the portable Shot without ownership or source" \
  "$import_output" \
  "$candidate_bin" --json import "$portable_bundle" \
  --output "$imported_root"
if ! jq -e \
  '.visibility == "include_private"
    and .ownership_acquired == false
    and .source_materialized == false
    and .action_count > 0' \
  "$import_output" >/dev/null; then
  fail "import output conflated receiving records with ownership or materialization"
fi
if find "$imported_root" -maxdepth 1 -type d -name '*.xcodeproj' -print |
  awk 'NR == 1 { found = 1 } END { exit(found ? 0 : 1) }'; then
  fail "portable import invented an Xcode source project"
fi
for forbidden_import in \
  "$imported_root/TemplateApp.swift" \
  "$imported_root/.tohseno/app.toml" \
  "$imported_root/.tohseno/private/identity" \
  "$imported_root/identity"
do
  if [ -e "$forbidden_import" ] || [ -L "$forbidden_import" ]; then
    fail "portable import invented source, ledger ownership, or private key state: $forbidden_import"
  fi
done

import_verification="$control_root/import-verification.json"
run_json \
  "verify the imported Shot from canonical records" \
  "$import_verification" \
  "$candidate_bin" --json verify "$imported_root"
if ! jq -e \
  '.schema == "tohseno.cli-verification/1"
    and .conformant == true
    and .local.scope == "shot_body"
    and .local.report.embedded_metadata_verified == false
    and .local.report.selected_version_id != null' \
  "$import_verification" >/dev/null; then
  fail "the imported record-only Shot did not verify without pretending source exists"
fi

printf '%s\n' \
  "ontology lifecycle smoke passed:" \
  "  intention → app-specific conception → accepted Birth Plan and Genome" \
  "  → Apple expression → XCUITest journey → accepted birth/version 0001" \
  "  → exact-version private feedback → evolutionary intent → version 0002" \
  "  → private Base Token Association → mixed-ancestry public export rejected" \
  "  → reconstructed lineage → private export/import → imported verification"
