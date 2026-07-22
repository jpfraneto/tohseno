#!/usr/bin/env bash
#
# TOHSENO one-shot bootstrap
#
#   curl -fsSL https://tohseno.com/oneshot.sh | bash
#
# This script is a small, inspectable bootstrap. It does exactly four things:
#
#   1. checks that git (and optionally bun and Xcode) exist;
#   2. clones the TOHSENO rails repository and verifies it matches the exact
#      pinned commit below;
#   3. creates your app workspace as a copy of the base app — a compiling,
#      running iOS writing app with a seed phrase instead of a signup form;
#   4. prints the one command to hand your coding agent, plus your sentence.
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
#
# Environment overrides (for development and mirrors):
#   TOHSENO_HOME   rails install location   (default: ~/.tohseno)
#   TOHSENO_REPO   rails repository URL     (default: pinned public repo)
#
# Release discipline: TOHSENO_PIN below is the single source of trust. It must
# be bumped to the released commit whenever a new rails release is published,
# and the served copy of this script must change in the same release.

set -euo pipefail

TOHSENO_PIN="35021b38e71257d137c184081a1ba0d4503fa5ef"
DEFAULT_REPO="https://github.com/jpfraneto/tohseno.git"
ONESHOT_VERSION="0.3.0"

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
  say "  ${D}one prompt → an iOS app on your phone · oneshot v${ONESHOT_VERSION}${R}"
  say ""
}

usage() {
  say "TOHSENO oneshot ${ONESHOT_VERSION}"
  say "  --target DIR     directory to create the new app in"
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
    fail "no terminal available to ask: ${prompt}  (rerun with --target DIR)"
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

  banner
  say "  ${B}This will${R}   clone the tohseno rails ${D}(pinned commit ${TOHSENO_PIN:0:7})${R} into"
  say "              ${D}${rails_dir}${R} and create your app workspace — already"
  say "              a compiling iOS app you mutate with one sentence."
  say "  ${B}It won't${R}    create accounts, take payment, deploy anything, send"
  say "              telemetry, or touch any file outside those two directories."
  say ""

  if [ "$dry_run" = "yes" ]; then
    say "Dry run: nothing was checked, downloaded, created, or modified."
    say "Planned filesystem changes:"
    say "  $rails_dir            (pinned clone, created or fast-forwarded to pin)"
    say "  ${target:-<target asked interactively>}   (new app workspace, copied from the base app)"
    say "Rollback at any time: remove those two directories."
    exit 0
  fi

  command -v git >/dev/null 2>&1 || fail "git is required. Install it, then run this command again."
  ok "git found"
  if command -v bun >/dev/null 2>&1; then
    ok "bun found"
  else
    say "  ${Y}!${R} bun is not installed — ${D}bun run setup needs it later; https://bun.sh${R}"
  fi
  if command -v xcodebuild >/dev/null 2>&1; then
    ok "Xcode found"
  else
    say "  ${Y}!${R} Xcode is not installed — ${D}the app builds on a Mac with Xcode from the App Store${R}"
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
    "templates/continuity-app/continuity.manifest.json" \
    "templates/continuity-app/project.yml" \
    "templates/continuity-app/App/WritingApp.swift" \
    "templates/continuity-app/App/AppConfig.swift" \
    "templates/continuity-app/App/Identity/BIP39.swift" \
    "templates/continuity-app/Tests/BIP39Tests.swift" \
    "templates/continuity-app/site/index.html" \
    "templates/continuity-app/scripts/setup.ts" \
    "templates/continuity-app/README.md" \
    "skills/continuity-app/SKILL.md"; do
    [ -f "$rails_dir/$required" ] || fail "pinned rails are missing $required — the pin predates a required release; report this"
  done
  ok "rails verified at pinned commit ${TOHSENO_PIN:0:7}"
  say ""

  # Choose the app workspace.
  if [ -z "$target" ]; then
    ask "${B}Name your new app (this becomes its directory):${R} " target
  fi
  [ -n "$target" ] || fail "a target directory is required"
  if [ -e "$target" ] && [ -n "$(ls -A "$target" 2>/dev/null)" ]; then
    fail "$target exists and is not empty — refusing to overwrite. Choose a new directory."
  fi

  # The workspace IS the base app: a compiling, running iOS writing app.
  # The agent mutates it toward your prompt — never an empty directory.
  mkdir -p "$target"
  cp -R "$rails_dir/templates/continuity-app/." "$target/"
  mkdir -p "$target/skills/continuity-app"
  cp "$rails_dir/skills/continuity-app/SKILL.md" "$target/skills/continuity-app/SKILL.md"
  local workspace_dir
  workspace_dir="$(cd "$target" && pwd -P)"
  cat > "$target/AGENTS.md" <<AGENTS_ENTRY
# Agent entry point

You are the coding agent for this app. The workspace already contains a
compiling, running iOS writing app — the base app. You never start from an
empty directory: copy nothing, scaffold nothing, mutate this app toward the
owner's prompt.

1. Read skills/continuity-app/SKILL.md and follow it. It is the build
   protocol: one-line input, at most three questions, sensible defaults
   recorded as ASSUMED, invariant tests, and the TOHSENO completion report.
2. The owner's prompt may be one sentence in the conversation, or a
   MASTER_PROMPT.md file here. Treat any MASTER_PROMPT.md as private product
   input: it is gitignored and must never be committed, logged, echoed, or
   transmitted — nor any credential or capability.
3. The manifest schema and validators live in the pinned rails checkout at
   $rails_dir (commit $TOHSENO_PIN). Update continuity.manifest.json to
   record what you build. The exact validation gate is:

       (cd "$rails_dir" && bun run validate "$workspace_dir/continuity.manifest.json")

   It must exit zero; importing or running validate.ts directly proves nothing.
4. Writing.xcodeproj is generated, not file-system-synced. After changing
   project.yml or adding, removing, or moving Swift files, run:

       xcodegen generate

5. Verify with the invariant tests and a simulator that actually exists:

       UDID=\$(xcrun simctl list devices available | grep -E '^[[:space:]]+iPhone' | grep -oE '[0-9A-F-]{36}' | head -1)
       if [ -z "\$UDID" ]; then
         echo "No available iPhone simulator; install one in Xcode > Settings > Platforms." >&2
         exit 1
       fi
       xcodebuild -project Writing.xcodeproj -scheme Writing \\
         -destination "platform=iOS Simulator,id=\$UDID" test

6. A prototype provider secret may use only DEV_SECRET in gitignored
   Config/Local.xcconfig. It is for an owner-controlled development device
   only, is forced empty in simulator and Release builds, and must be replaced
   by short-lived TokenMint credentials before distribution.
7. Setup is interactive by default. Only with explicit owner approval may an
   agent use its non-interactive mode:

       bun run setup --from-manifest --team auto

   App Store Connect credentials add --asc-key <absolute-.p8-path>,
   --asc-key-id <KEY_ID>, and --asc-issuer-id <ISSUER_UUID>; an enabled paywall
   may add --revenuecat-key <public-key>. Create the ASC key at
   App Store Connect > Users and Access > Integrations > App Store Connect API
   > Team Keys > "+" > role App Manager > download once. Setup validates it
   read-only before writing config; credential values never enter git.
8. Do not create paid infrastructure, alter DNS, submit to stores, publish
   packages, or deploy production without the owner's explicit approval.
   Prepare the TestFlight command; never run it unprompted.
AGENTS_ENTRY
  git -C "$target" init --quiet
  git -C "$target" add -A
  git -C "$target" -c user.name="tohseno-oneshot" -c user.email="oneshot@tohseno.com" \
    commit --quiet -m "chore: tohseno oneshot workspace at rails ${TOHSENO_PIN}"
  ok "workspace created — a working iOS app, before you say a word"

  local agent_cmd
  agent_cmd="$(first_agent)"
  say ""
  say "  ${D}──────────────────────────────────────────────────────────────${R}"
  say ""
  say "  ${G}${B}Workspace ready:${R} ${B}${target}${R}"
  say "  ${D}Coding agents on PATH: $(detect_agents)${R}"
  say ""
  say "  ${B}Run the base app right now:${R}"
  say ""
  say "      ${C}open ${target}/Writing.xcodeproj${R}   ${D}then ⌘R${R}"
  say ""
  say "  ${B}Or make it yours — one sentence:${R}"
  say ""
  say "      ${C}cd ${target}${R}"
  say "      ${C}${agent_cmd} \"Read AGENTS.md. Build this: <your app, in one sentence>\"${R}"
  say ""
  say "  ${D}Undo everything: rm -rf ${target} ${rails_dir}${R}"
  say "  ${D}──────────────────────────────────────────────────────────────${R}"
}

main "$@"
