# ADR 0030: The website downloads the native installer directly

Status: accepted

Date: 2026-08-30

Supersedes:

- ADR 0026 and ADR 0028 only where they make a copied shell one-liner the
  canonical consumer door on the public website. Their immutable artifact,
  exact digest, Developer ID, notarization, Finder handoff, and fail-closed
  publication requirements remain accepted.

## Context

The shell installer added verification and a familiar Finder handoff, but it
also made Terminal and a second network request part of first contact. When the
script route is unavailable, `curl -f` reports only an HTTP error and gives the
person no useful product surface.

The website already knows the only currently supported product is the native
Mac app and already has a direct, configuration-gated DMG route. A browser can
present that route as an ordinary download without asking someone to copy or
run shell code.

## Decision

The canonical public website action is an ordinary link to
`/download/macos`. Its visible label is progressively enhanced from the
browser's reported platform:

- on macOS it says **Download for this Mac** and identifies the universal
  macOS 14-or-newer installer;
- on iPhone or iPad it says to open the page on a Mac; and
- on Windows, Android, Linux, ChromeOS, or an unknown platform it says
  truthfully that TOHSENO currently requires macOS 14 or newer.

The link remains usable without JavaScript and defaults to **Download for
Mac**. Every normal landing-page install action uses the same link. There is no
clipboard operation, Terminal instruction, shell execution, architecture
choice, account step, or second installation path.

`/install` and `/download` remain compatibility aliases for the accepted
interactive shell transport, and `/install.sh` and `/oneshot.sh` retain their
legacy and encrypted-relay roles. They are no longer presented as the normal
consumer door.

`/download/macos` remains fail-closed unless an enabled absolute immutable
HTTPS DMG URL and its exact lowercase SHA-256 are configured. This decision
does not publish an artifact, activate a release, waive a release gate, or
authorize an external deployment.

## Consequences

A supported visitor sees one familiar download button and Finder performs the
normal DMG installation gesture. Unsupported visitors see the real system
requirement instead of a command that cannot install there.

The direct browser download does not reproduce the shell transport's local
preflight verification. Trust therefore remains anchored in the exact pinned
artifact, Developer ID signature, notarization, Gatekeeper, and the existing
publication gate.

This decision changes no public protocol encoding, schema, frozen vector,
Shot, Evolution, Builder, Registry, identity, receipt, billing, or managed
inference behavior.
