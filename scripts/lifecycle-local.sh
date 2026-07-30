#!/bin/sh
set -eu
umask 077

fail() {
  printf 'lifecycle-local.sh: %s\n' "$1" >&2
  exit 1
}

reject_symlink_components() {
  checked_path="$1"
  checked_label="$2"
  case "$checked_path" in
    /*) ;;
    *) fail "$checked_label must be an absolute path." ;;
  esac
  case "$checked_path" in
    */../*|*/..|*/./*|*/.) fail "$checked_label contains a dot path component." ;;
  esac
  while [ "$checked_path" != "/" ]; do
    if [ -L "$checked_path" ]; then
      fail "$checked_label contains a symlinked path component."
    fi
    checked_path="$(dirname -- "$checked_path")"
  done
}

require_real_directory() {
  if [ -L "$1" ] || [ ! -d "$1" ]; then
    fail "$2 must be a real directory."
  fi
}

paths_overlap() {
  overlap_left="$1"
  overlap_right="$2"
  case "$overlap_left/" in
    "$overlap_right"/*) return 0 ;;
  esac
  case "$overlap_right/" in
    "$overlap_left"/*) return 0 ;;
  esac
  return 1
}

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)"
candidate_root="${TOHSENO_DATA_ROOT:-$HOME/.tohseno-genesis}"
evidence_directory="${TOHSENO_LIFECYCLE_EVIDENCE:-$repository_root/genesis/lifecycle/evidence}"

case "${HOME:-}" in
  /*) ;;
  *) fail "HOME must be an absolute path." ;;
esac
reject_symlink_components "$HOME" "HOME"
require_real_directory "$HOME" "HOME"
home_directory="$(CDPATH= cd -- "$HOME" && pwd -P)"

reject_symlink_components "$candidate_root" "TOHSENO_DATA_ROOT"
stable_root="$home_directory/.tohseno"
if paths_overlap "$candidate_root" "$stable_root"; then
  fail "candidate lifecycle root overlaps the stable data root."
fi
mkdir -p "$candidate_root"
require_real_directory "$candidate_root" "candidate data root"
candidate_root="$(CDPATH= cd -- "$candidate_root" && pwd -P)"

if [ -L "$stable_root" ]; then
  fail "stable data root is symlinked; candidate isolation cannot be proven."
fi
if [ -e "$stable_root" ]; then
  require_real_directory "$stable_root" "stable data root"
  stable_root="$(CDPATH= cd -- "$stable_root" && pwd -P)"
fi
if paths_overlap "$candidate_root" "$stable_root"; then
  fail "candidate lifecycle root overlaps the stable data root."
fi

lifecycle_marker="$candidate_root/.genesis-lifecycle-root"
install_marker="$candidate_root/.genesis-install-root"
install_layout=0
for install_entry in \
  "$candidate_root/bin" \
  "$candidate_root/share" \
  "$candidate_root/releases" \
  "$candidate_root/current"; do
  if [ -e "$install_entry" ] || [ -L "$install_entry" ]; then
    install_layout=1
  fi
done

if [ "$install_layout" -eq 1 ]; then
  if [ -L "$install_marker" ] ||
    [ ! -f "$install_marker" ] ||
    [ "$(cat "$install_marker")" != "tohseno-genesis-install-v1" ]; then
    fail "candidate installation exists without a valid installer marker."
  fi
  for install_directory in \
    "$candidate_root/bin" \
    "$candidate_root/share" \
    "$candidate_root/releases"; do
    if [ -e "$install_directory" ] || [ -L "$install_directory" ]; then
      require_real_directory "$install_directory" "candidate installation directory"
    fi
  done
  if [ -e "$candidate_root/current" ] || [ -L "$candidate_root/current" ]; then
    if [ ! -L "$candidate_root/current" ]; then
      fail "candidate release pointer is not an installer-controlled symlink."
    fi
    current_release="$(CDPATH= cd -- "$candidate_root/current" && pwd -P)" ||
      fail "candidate release pointer is broken."
    case "$current_release/" in
      "$candidate_root"/releases/*/) ;;
      *) fail "candidate release pointer escapes the installation root." ;;
    esac
  fi
elif [ -e "$install_marker" ] || [ -L "$install_marker" ]; then
  fail "candidate installer marker exists without its installation layout."
fi

has_candidate_state=0
for entry in "$candidate_root"/* "$candidate_root"/.[!.]* "$candidate_root"/..?*; do
  if [ -e "$entry" ] || [ -L "$entry" ]; then
    case "$entry" in
      "$lifecycle_marker") ;;
      "$install_marker"|"$candidate_root/bin"|"$candidate_root/share"|\
        "$candidate_root/releases"|"$candidate_root/current")
        if [ "$install_layout" -ne 1 ]; then
          has_candidate_state=1
          break
        fi
        ;;
      *)
        has_candidate_state=1
        break
        ;;
    esac
  fi
done

if [ -e "$lifecycle_marker" ] || [ -L "$lifecycle_marker" ]; then
  if [ -L "$lifecycle_marker" ] ||
    [ ! -f "$lifecycle_marker" ] ||
    [ "$(cat "$lifecycle_marker")" != "tohseno-genesis-lifecycle-v1" ]; then
    fail "candidate lifecycle marker is missing, symlinked, or unrecognized."
  fi
elif [ "$has_candidate_state" -eq 1 ]; then
  fail "candidate root is nonempty and is not a recorded Genesis lifecycle."
else
  marker_stage="$(mktemp "$candidate_root/.genesis-lifecycle.XXXXXX")" ||
    fail "could not stage the lifecycle marker."
  (umask 077 && printf '%s\n' "tohseno-genesis-lifecycle-v1" >"$marker_stage")
  if [ -e "$lifecycle_marker" ] || [ -L "$lifecycle_marker" ]; then
    rm -f "$marker_stage"
    fail "candidate lifecycle marker appeared concurrently."
  fi
  mv "$marker_stage" "$lifecycle_marker"
fi

reject_symlink_components "$evidence_directory" "TOHSENO_LIFECYCLE_EVIDENCE"
if paths_overlap "$evidence_directory" "$stable_root"; then
  fail "lifecycle evidence directory overlaps stable TOHSENO state."
fi
mkdir -p "$evidence_directory"
require_real_directory "$evidence_directory" "lifecycle evidence directory"
evidence_directory="$(CDPATH= cd -- "$evidence_directory" && pwd -P)"
if paths_overlap "$evidence_directory" "$stable_root"; then
  fail "lifecycle evidence directory overlaps stable TOHSENO state."
fi

export TOHSENO_DATA_ROOT="$candidate_root"
tohseno_bin="${TOHSENO_CANDIDATE_BIN:-$home_directory/.tohseno-genesis/bin/tohseno-genesis}"
reject_symlink_components "$tohseno_bin" "TOHSENO_CANDIDATE_BIN"
if [ -L "$tohseno_bin" ] || [ ! -f "$tohseno_bin" ] || [ ! -x "$tohseno_bin" ]; then
  fail "install the GENESIS candidate before running its lifecycle."
fi
tohseno_bin_directory="$(CDPATH= cd -- "$(dirname -- "$tohseno_bin")" && pwd -P)"
tohseno_bin="$tohseno_bin_directory/$(basename -- "$tohseno_bin")"
case "$tohseno_bin/" in
  "$stable_root"/*) fail "candidate executable is inside stable TOHSENO state." ;;
esac
candidate_version="$("$tohseno_bin" --version 2>/dev/null)" ||
  fail "candidate executable did not start."
if [ "$candidate_version" != "tohseno 0.7.1" ]; then
  fail "candidate executable reported '$candidate_version', not tohseno 0.7.1."
fi

prompt_file="$repository_root/genesis/SHOT_1_INTENT.md"
if [ -L "$prompt_file" ] || [ ! -f "$prompt_file" ]; then
  fail "Genesis Shot 1 intent is missing or symlinked."
fi

identity_evidence="$evidence_directory/identity.json"
evolution_evidence="$evidence_directory/evolution-1.json"
for evidence_path in "$identity_evidence" "$evolution_evidence"; do
  if [ -L "$evidence_path" ] ||
    { [ -e "$evidence_path" ] && [ ! -f "$evidence_path" ]; }; then
    fail "refusing unsafe existing evidence path $evidence_path."
  fi
done

identity_stage=""
evolution_stage=""
cleanup_evidence() {
  for evidence_stage in "$identity_stage" "$evolution_stage"; do
    case "$evidence_stage" in
      "$evidence_directory"/.lifecycle-evidence.*)
        if [ -f "$evidence_stage" ] && [ ! -L "$evidence_stage" ]; then
          rm -f "$evidence_stage"
        fi
        ;;
    esac
  done
}
trap cleanup_evidence EXIT
trap 'exit 1' HUP INT TERM

printf '%s\n' "$candidate_version"
if ! "$tohseno_bin" doctor; then
  fail "candidate doctor failed; no Evolution was claimed complete."
fi

identity_stage="$(mktemp "$evidence_directory/.lifecycle-evidence.XXXXXX")" ||
  fail "could not stage identity evidence."
if ! "$tohseno_bin" --json identity show >"$identity_stage"; then
  fail "identity inspection failed; no identity evidence was published."
fi
if [ ! -s "$identity_stage" ]; then
  fail "identity inspection produced empty evidence."
fi
if [ -L "$identity_evidence" ] ||
  { [ -e "$identity_evidence" ] && [ ! -f "$identity_evidence" ]; }; then
  fail "identity evidence path became unsafe before publication."
fi
mv "$identity_stage" "$identity_evidence"
identity_stage=""

if ! "$tohseno_bin" create tohseno --prompt-file "$prompt_file"; then
  fail "candidate creation failed; no Evolution was claimed complete."
fi

evolution_stage="$(mktemp "$evidence_directory/.lifecycle-evidence.XXXXXX")" ||
  fail "could not stage Evolution evidence."
if ! "$tohseno_bin" --json verify tohseno >"$evolution_stage"; then
  fail "local verification failed; no Evolution evidence was published."
fi
if [ ! -s "$evolution_stage" ]; then
  fail "local verification produced empty evidence."
fi
if [ -L "$evolution_evidence" ] ||
  { [ -e "$evolution_evidence" ] && [ ! -f "$evolution_evidence" ]; }; then
  fail "Evolution evidence path became unsafe before publication."
fi
mv "$evolution_stage" "$evolution_evidence"
evolution_stage=""

printf '%s\n' \
  "Evolution 1 is locally complete; run the guarded mainnet lifecycle to publish it."
