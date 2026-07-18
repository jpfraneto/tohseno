#!/usr/bin/env bash
#
# TOHSENO one-shot bootstrap
#
#   curl -fsSL https://tohseno.com/oneshot.sh | bash
#
# This script is a small, inspectable bootstrap. It does exactly four things:
#
#   1. checks that git (and optionally bun) exist;
#   2. clones the TOHSENO rails repository and verifies it matches the exact
#      pinned commit below;
#   3. creates a new continuity-app workspace from the pinned template;
#   4. prints the next step for your coding agent.
#
# It never asks for or accepts credentials, capabilities, or payment. It sends
# no telemetry. Its only network calls are the git clone/fetch of the public
# repository below. It creates no paid resources and deploys nothing.
#
# Read it first if you prefer:  curl -fsSL https://tohseno.com/oneshot.sh | less
#
# Usage:
#   ... | bash                       interactive setup
#   ... | bash -s -- --dry-run       print the plan, change nothing
#   ... | bash -s -- --target DIR    choose the new app directory
#
# Environment overrides (for development and mirrors):
#   TOHSENO_HOME   rails install location   (default: ~/.tohseno)
#   TOHSENO_REPO   rails repository URL     (default: pinned public repo)
#
# Release discipline: TOHSENO_PIN below is the single source of trust. It must
# be bumped to the released commit whenever a new rails release is published,
# and the served copy of this script must change in the same release.

set -euo pipefail

TOHSENO_PIN="358adcf2ecd3bfebd53056684d837a273303d1b6"
DEFAULT_REPO="https://github.com/jpfraneto/tohseno.git"
ONESHOT_VERSION="0.1.0"

say() { printf '%s\n' "$*"; }
fail() { printf 'oneshot: %s\n' "$*" >&2; exit 1; }

usage() {
  say "TOHSENO oneshot ${ONESHOT_VERSION}"
  say "  --target DIR   directory to create the new continuity app in"
  say "  --dry-run      print the exact plan and exit without changing anything"
  say "  --help         this text"
}

detect_agents() {
  # Report which supported local coding agents are on PATH. Detection only:
  # nothing is installed, launched, or contacted.
  local found=""
  local candidate
  for candidate in claude codex gemini cursor-agent aider opencode; do
    if command -v "$candidate" >/dev/null 2>&1; then
      found="${found}${found:+, }${candidate}"
    fi
  done
  printf '%s' "${found:-none found}"
}

main() {
  local target=""
  local dry_run="no"
  while [ $# -gt 0 ]; do
    case "$1" in
      --target) [ $# -ge 2 ] || fail "--target requires a directory"; target="$2"; shift 2 ;;
      --dry-run) dry_run="yes"; shift ;;
      --help|-h) usage; exit 0 ;;
      *) fail "unknown argument: $1 (secrets and capabilities are never accepted as arguments)" ;;
    esac
  done

  local home_dir="${TOHSENO_HOME:-$HOME/.tohseno}"
  local repo_url="${TOHSENO_REPO:-$DEFAULT_REPO}"
  local rails_dir="$home_dir/rails"

  say "TOHSENO oneshot ${ONESHOT_VERSION}"
  say "  rails source : $repo_url"
  say "  pinned commit: $TOHSENO_PIN"
  say "  rails install: $rails_dir"
  say ""
  say "This bootstrap will:"
  say "  1. verify git is installed (and report whether bun is);"
  say "  2. clone the rails repository and refuse to continue unless the"
  say "     checkout is exactly the pinned commit above;"
  say "  3. create a new app workspace from templates/continuity-app,"
  say "     refusing to touch a directory that is not empty;"
  say "  4. print the instruction to hand your coding agent."
  say "It will NOT create accounts, take payment, send telemetry, deploy"
  say "anything, or read/transmit any file outside the two directories above."
  say ""

  if [ "$dry_run" = "yes" ]; then
    say "Dry run: nothing was checked, downloaded, created, or modified."
    say "Planned filesystem changes:"
    say "  $rails_dir            (pinned clone, created or fast-forwarded to pin)"
    say "  ${target:-<target asked interactively>}   (new app workspace)"
    say "Rollback at any time: remove those two directories."
    exit 0
  fi

  command -v git >/dev/null 2>&1 || fail "git is required. Install it, then run this command again."
  if ! command -v bun >/dev/null 2>&1; then
    say "note: bun is not installed. The rails validators need it eventually."
    say "      Install it from https://bun.sh when you are ready; this"
    say "      bootstrap will not run another vendor's installer for you."
    say ""
  fi

  # Acquire the rails at exactly the pinned commit.
  if [ -d "$rails_dir/.git" ]; then
    say "Rails already present; fetching and checking out the pin."
    git -C "$rails_dir" fetch --quiet origin
  else
    mkdir -p "$home_dir"
    git clone --quiet "$repo_url" "$rails_dir"
  fi
  git -C "$rails_dir" checkout --quiet "$TOHSENO_PIN" 2>/dev/null \
    || fail "pinned commit $TOHSENO_PIN not found in $repo_url — refusing to run unpinned code"
  local actual
  actual="$(git -C "$rails_dir" rev-parse HEAD)"
  [ "$actual" = "$TOHSENO_PIN" ] || fail "checkout is $actual, expected $TOHSENO_PIN — refusing to continue"
  local required
  for required in \
    "templates/continuity-app/MASTER_PROMPT.md" \
    "templates/continuity-app/continuity.manifest.json" \
    "templates/continuity-app/EVOLUTION.md" \
    "templates/continuity-app/OPERATOR.md" \
    "templates/continuity-app/README.md" \
    "skills/continuity-app/SKILL.md"; do
    [ -f "$rails_dir/$required" ] || fail "pinned rails are missing $required — the pin predates a required release; report this"
  done
  say "Rails verified at pinned commit."

  # Choose the app workspace.
  if [ -z "$target" ]; then
    if [ -t 0 ]; then
      printf 'Directory for your new continuity app: '
      read -r target
    elif [ -r /dev/tty ]; then
      printf 'Directory for your new continuity app: ' > /dev/tty
      read -r target < /dev/tty
    else
      fail "no terminal available to ask for a target; rerun with --target DIR"
    fi
  fi
  [ -n "$target" ] || fail "a target directory is required"
  if [ -e "$target" ] && [ -n "$(ls -A "$target" 2>/dev/null)" ]; then
    fail "$target exists and is not empty — refusing to overwrite. Choose a new directory."
  fi

  # Create the workspace from the pinned template, plus the agent entry point.
  mkdir -p "$target"
  cp "$rails_dir/templates/continuity-app/MASTER_PROMPT.md" "$target/MASTER_PROMPT.md"
  cp "$rails_dir/templates/continuity-app/continuity.manifest.json" "$target/continuity.manifest.json"
  cp "$rails_dir/templates/continuity-app/EVOLUTION.md" "$target/EVOLUTION.md"
  cp "$rails_dir/templates/continuity-app/OPERATOR.md" "$target/OPERATOR.md"
  cp "$rails_dir/templates/continuity-app/README.md" "$target/TEMPLATE_README.md"
  mkdir -p "$target/skills/continuity-app"
  cp "$rails_dir/skills/continuity-app/SKILL.md" "$target/skills/continuity-app/SKILL.md"
  cat > "$target/AGENTS.md" <<AGENTS_ENTRY
# Agent entry point

You are the coding agent for this continuity application.

1. Read MASTER_PROMPT.md as private product input. Never commit, log, echo,
   or transmit it, or any credential or capability, anywhere.
2. Follow skills/continuity-app/SKILL.md in order. It is the build protocol:
   interview, manifest, privacy inventory, smallest offline vertical slice,
   invariant tests, deployment preparation, ejection package.
3. The manifest schema and validators live in the pinned rails checkout at
   $rails_dir (commit $TOHSENO_PIN).
4. Do not create paid infrastructure, alter DNS, submit to stores, publish
   packages, or deploy production without the owner's explicit approval.

The owner's first step is to rewrite MASTER_PROMPT.md around one repeated
action, then start you in this directory.
AGENTS_ENTRY
  git -C "$target" init --quiet
  git -C "$target" add -A
  git -C "$target" -c user.name="tohseno-oneshot" -c user.email="oneshot@tohseno.com" \
    commit --quiet -m "chore: tohseno oneshot workspace at rails ${TOHSENO_PIN}"

  say ""
  say "Workspace created: $target"
  say "Coding agents detected on PATH: $(detect_agents)"
  say ""
  say "Next steps:"
  say "  1. Rewrite $target/MASTER_PROMPT.md around one repeated action."
  say "  2. Start your coding agent inside $target."
  say "  3. Tell it: read AGENTS.md, then build this continuity app by"
  say "     following skills/continuity-app/SKILL.md."
  say ""
  say "Rollback: rm -rf $target $rails_dir  (nothing else was touched)"
}

main "$@"
