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
#   3. creates a new continuity-app workspace — blank, or from a shipped
#      working example — and asks which you want;
#   4. prints the one command to hand your coding agent.
#
# It never asks for or accepts credentials, capabilities, or payment. It sends
# no telemetry. Its only network calls are the git clone/fetch of the public
# repository below. It creates no paid resources and deploys nothing.
#
# Read it first if you prefer:  curl -fsSL https://tohseno.com/oneshot.sh | less
#
# Usage:
#   ... | bash                            interactive setup
#   ... | bash -s -- --dry-run            print the plan, change nothing
#   ... | bash -s -- --target DIR         choose the new app directory
#   ... | bash -s -- --example NAME       start from a working example
#                                         (anky | daily-observation)
#
# Environment overrides (for development and mirrors):
#   TOHSENO_HOME   rails install location   (default: ~/.tohseno)
#   TOHSENO_REPO   rails repository URL     (default: pinned public repo)
#
# Release discipline: TOHSENO_PIN below is the single source of trust. It must
# be bumped to the released commit whenever a new rails release is published,
# and the served copy of this script must change in the same release.

set -euo pipefail

TOHSENO_PIN="fc047fc4f67dfd4860298f55fdae6ec7fc89bac9"
DEFAULT_REPO="https://github.com/jpfraneto/tohseno.git"
ONESHOT_VERSION="0.2.1"

# Colors only when stdout is a terminal.
if [ -t 1 ]; then
  B=$'\033[1m'; D=$'\033[2m'; C=$'\033[36m'; G=$'\033[32m'; Y=$'\033[33m'; R=$'\033[0m'
else
  B=""; D=""; C=""; G=""; Y=""; R=""
fi

say() { printf '%s\n' "$*"; }
ok() { printf '%s\n' "  ${G}✓${R} $*"; }
fail() { printf 'oneshot: %s\n' "$*" >&2; exit 1; }

banner() {
  say ""
  say "  ${C}${B}▀█▀ █▀█ █ █ █▀ █▀▀ █▄ █ █▀█${R}"
  say "  ${C}${B} █  █▄█ █▀█ ▄█ ██▄ █ ▀█ █▄█${R}"
  say "  ${D}an app for one repeated action · oneshot v${ONESHOT_VERSION}${R}"
  say ""
}

usage() {
  say "TOHSENO oneshot ${ONESHOT_VERSION}"
  say "  --target DIR     directory to create the new continuity app in"
  say "  --example NAME   start from a working example (anky | daily-observation)"
  say "  --dry-run        print the exact plan and exit without changing anything"
  say "  --help           this text"
}

# Ask a question even though stdin is the curl pipe.
ask() {
  local prompt="$1" var="$2" answer=""
  if [ -t 0 ]; then
    printf '%s' "$prompt"
    read -r answer
  elif [ -r /dev/tty ]; then
    printf '%s' "$prompt" > /dev/tty
    read -r answer < /dev/tty
  else
    fail "no terminal available to ask: ${prompt}  (rerun with --target DIR and optionally --example NAME)"
  fi
  eval "$var=\$answer"
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

first_agent() {
  local candidate
  for candidate in claude codex gemini cursor-agent aider opencode; do
    if command -v "$candidate" >/dev/null 2>&1; then
      printf '%s' "$candidate"
      return
    fi
  done
  printf '%s' "your-agent"
}

main() {
  local target=""
  local example=""
  local dry_run="no"
  while [ $# -gt 0 ]; do
    case "$1" in
      --target) [ $# -ge 2 ] || fail "--target requires a directory"; target="$2"; shift 2 ;;
      --example) [ $# -ge 2 ] || fail "--example requires a name (anky | daily-observation)"; example="$2"; shift 2 ;;
      --dry-run) dry_run="yes"; shift ;;
      --help|-h) usage; exit 0 ;;
      *) fail "unknown argument: $1 (secrets and capabilities are never accepted as arguments)" ;;
    esac
  done
  case "$example" in
    ""|anky|daily-observation) ;;
    *) fail "unknown example: $example (available: anky, daily-observation)" ;;
  esac

  local home_dir="${TOHSENO_HOME:-$HOME/.tohseno}"
  local repo_url="${TOHSENO_REPO:-$DEFAULT_REPO}"
  local rails_dir="$home_dir/rails"

  banner
  say "  ${B}This will${R}   clone the tohseno rails ${D}(pinned commit ${TOHSENO_PIN:0:7})${R} into"
  say "              ${D}${rails_dir}${R}, create a new app workspace, and hand"
  say "              you one command for your coding agent."
  say "  ${B}It won't${R}    create accounts, take payment, deploy anything, send"
  say "              telemetry, or touch any file outside those two directories."
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
  ok "git found"
  if command -v bun >/dev/null 2>&1; then
    ok "bun found"
  else
    say "  ${Y}!${R} bun is not installed — the rails validators need it eventually."
    say "    ${D}Install it from https://bun.sh when you are ready; this bootstrap${R}"
    say "    ${D}will not run another vendor's installer for you.${R}"
  fi

  # Acquire the rails at exactly the pinned commit.
  if [ -d "$rails_dir/.git" ]; then
    git -C "$rails_dir" fetch --quiet origin
    ok "rails already present, fetched"
  else
    mkdir -p "$home_dir"
    git clone --quiet "$repo_url" "$rails_dir"
    ok "rails cloned"
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
    "examples/anky/MASTER_PROMPT.md" \
    "examples/anky/continuity.manifest.json" \
    "examples/daily-observation/MASTER_PROMPT.md" \
    "examples/daily-observation/continuity.manifest.json" \
    "skills/continuity-app/SKILL.md"; do
    [ -f "$rails_dir/$required" ] || fail "pinned rails are missing $required — the pin predates a required release; report this"
  done
  ok "rails verified at pinned commit ${TOHSENO_PIN:0:7}"
  say ""

  # Choose the app workspace.
  if [ -z "$target" ]; then
    ask "${B}Name your new continuity app (this becomes its directory):${R} " target
  fi
  [ -n "$target" ] || fail "a target directory is required"
  if [ -e "$target" ] && [ -n "$(ls -A "$target" 2>/dev/null)" ]; then
    fail "$target exists and is not empty — refusing to overwrite. Choose a new directory."
  fi

  # Choose the starting point: a blank prompt the agent will co-write through
  # an interview, or a shipped working example that builds immediately.
  if [ -z "$example" ] && { [ -t 0 ] || [ -r /dev/tty ]; }; then
    say ""
    say "  ${B}How do you want to start?${R}"
    say ""
    say "    ${B}1)${R} my own idea ${D}— your agent interviews you first, then writes${R}"
    say "       ${D}MASTER_PROMPT.md with you before building anything${R}"
    say "    ${B}2)${R} example: ${B}anky${R} ${D}— a continuous-writing ritual (8 minutes of${R}"
    say "       ${D}forward writing seals a session); builds right away${R}"
    say "    ${B}3)${R} example: ${B}daily-observation${R} ${D}— photograph one living thing and${R}"
    say "       ${D}write one sentence about it; builds right away${R}"
    say ""
    local choice=""
    ask "  ${B}Choose [1/2/3]${R} (default 1): " choice
    case "$choice" in
      ""|1) example="" ;;
      2) example="anky" ;;
      3) example="daily-observation" ;;
      *) fail "unrecognized choice: $choice" ;;
    esac
  fi

  # Create the workspace from the pinned template, plus the agent entry point.
  mkdir -p "$target"
  if [ -n "$example" ]; then
    cp "$rails_dir/examples/$example/MASTER_PROMPT.md" "$target/MASTER_PROMPT.md"
    cp "$rails_dir/examples/$example/continuity.manifest.json" "$target/continuity.manifest.json"
  else
    cp "$rails_dir/templates/continuity-app/MASTER_PROMPT.md" "$target/MASTER_PROMPT.md"
    cp "$rails_dir/templates/continuity-app/continuity.manifest.json" "$target/continuity.manifest.json"
  fi
  cp "$rails_dir/templates/continuity-app/EVOLUTION.md" "$target/EVOLUTION.md"
  cp "$rails_dir/templates/continuity-app/OPERATOR.md" "$target/OPERATOR.md"
  cp "$rails_dir/templates/continuity-app/README.md" "$target/TEMPLATE_README.md"
  mkdir -p "$target/skills/continuity-app"
  cp "$rails_dir/skills/continuity-app/SKILL.md" "$target/skills/continuity-app/SKILL.md"
  cat > "$target/AGENTS.md" <<AGENTS_ENTRY
# Agent entry point

You are the coding agent for this continuity application.

Open MASTER_PROMPT.md first and check for the marker \`tohseno:template-prompt\`.

- If the marker is PRESENT, MASTER_PROMPT.md is still a placeholder. Do not
  build it. Instead, interview the owner following step 2 of
  skills/continuity-app/SKILL.md ("Interview for one observable action"), one
  question at a time. Then write MASTER_PROMPT.md together with the owner,
  remove the marker, and get their explicit confirmation of the core action,
  completion, interruption, and ritual-destroyer list before any scaffolding.
- If the marker is ABSENT, MASTER_PROMPT.md is real product input. Build it.

In both cases:

1. Treat MASTER_PROMPT.md as private product input. Never commit, log, echo,
   or transmit it, or any credential or capability, anywhere.
2. Follow skills/continuity-app/SKILL.md in order. It is the build protocol:
   interview, manifest, privacy inventory, smallest offline vertical slice,
   invariant tests, deployment preparation, ejection package.
3. The manifest schema and validators live in the pinned rails checkout at
   $rails_dir (commit $TOHSENO_PIN).
4. Do not create paid infrastructure, alter DNS, submit to stores, publish
   packages, or deploy production without the owner's explicit approval.
AGENTS_ENTRY
  cat > "$target/.gitignore" <<'GITIGNORE'
# MASTER_PROMPT.md is private product input. It must never enter git history.
MASTER_PROMPT.md
GITIGNORE
  git -C "$target" init --quiet
  git -C "$target" add -A
  git -C "$target" -c user.name="tohseno-oneshot" -c user.email="oneshot@tohseno.com" \
    commit --quiet -m "chore: tohseno oneshot workspace at rails ${TOHSENO_PIN}"
  if [ -n "$example" ]; then
    ok "workspace created from example ${B}${example}${R}"
  else
    ok "workspace created (blank — your agent will interview you)"
  fi

  local agent_cmd
  agent_cmd="$(first_agent)"
  say ""
  say "  ${D}──────────────────────────────────────────────────────────────${R}"
  say ""
  say "  ${G}${B}Workspace ready:${R} ${B}${target}${R}"
  say "  ${D}Coding agents on PATH: $(detect_agents)${R}"
  say ""
  say "  ${B}Next — start your agent:${R}"
  say ""
  say "      ${C}cd ${target}${R}"
  say "      ${C}${agent_cmd} \"Read AGENTS.md and begin.\"${R}"
  say ""
  if [ -n "$example" ]; then
    say "  It will build the ${example} example right away — MASTER_PROMPT.md"
    say "  and the manifest are already complete."
  else
    say "  It will interview you first — a few questions about the one action"
    say "  your app protects — then write MASTER_PROMPT.md with you, then build."
  fi
  say ""
  say "  ${D}Undo everything: rm -rf ${target} ${rails_dir}${R}"
  say "  ${D}──────────────────────────────────────────────────────────────${R}"
}

main "$@"
