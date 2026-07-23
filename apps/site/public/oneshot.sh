#!/usr/bin/env bash
#
# TOHSENO legacy one-shot migration notice
#
#   curl -fsSL https://tohseno.com/oneshot.sh | bash
#
# The original script created workspaces directly from a shared rails checkout.
# That behavior is retired: the reusable local-first factory now lives in the
# `tohseno` CLI, where every shot receives an immutable release, independent Git
# history, and its own pinned validation tools.
#
# This endpoint stays small and inspectable while existing links migrate. It
# makes no filesystem changes, accepts no secrets, sends no telemetry, and
# launches nothing. A follow-up release may turn it into a thin CLI installer
# only after the CLI is present in a published commit and this pin can safely
# move to that release.

set -euo pipefail

# Trust marker for the last published rails creator. Keep this exact value until
# a released commit containing the CLI has landed; bump it only in the required
# follow-up commit that converts this notice into a pinned thin installer.
TOHSENO_PIN="35021b38e71257d137c184081a1ba0d4503fa5ef"
ONESHOT_VERSION="0.4.0-deprecated"

say() { printf '%s\n' "$*"; }

usage() {
  say "TOHSENO oneshot ${ONESHOT_VERSION}"
  say ""
  say "This legacy workspace creator is retired. The canonical installer is:"
  say "  curl -fsSL https://tohseno.com/install.sh | bash"
}

main() {
  if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    usage
    return 0
  fi

  say ""
  say "TOHSENO oneshot has moved to the canonical TOHSENO installer."
  say ""
  say "This script no longer creates a workspace. Nothing was installed,"
  say "downloaded, launched, or changed on your machine."
  say ""
  say "Install TOHSENO:"
  say ""
  say "  curl -fsSL https://tohseno.com/install.sh | bash"
  say "  tohseno"
  say ""
  say "Repository contributors can still run: bun run tohseno"
  say ""
  say "The former creator remains auditable at pinned commit ${TOHSENO_PIN}."
  say "A follow-up release can make this endpoint a thin pinned CLI installer"
  say "after the CLI itself is published. Until then, inspect and run the local"
  say "canonical installer above."
  say ""
  return 2
}

main "$@"
