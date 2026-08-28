# ADR 0026: Keyboard-first creation, a truthful local Registry, and one-line native installation

Status: accepted

Date: 2026-08-28

Supersedes:

- ADR 0016 and ADR 0025 only where they forbid all Shot and Builder vocabulary
  from every normal native destination. Their App → Intent → App creation and
  evolution path, one-factory rule, six-state projection, and deleted Studio
  dashboard remain accepted.
- ADR 0025 only where it leaves the consumer download mechanism unspecified.
  Its self-contained signed/notarized DMG, integrity, fail-closed publication,
  and explicit owner-activation requirements remain accepted.

## Context

The native Mac app has made the keyboard visible as part of the product. A
person should be able to write an intention and send it without moving to a
button, while still being able to write a multi-line intention deliberately.

The local factory also already has a meaningful private track record: one
local DeviceKey identity, accepted Shot heads, contiguous Evolutions, and a
client-trusted active contract generation. That is useful to the owner, but it
is not the same as a secure public Builder account or an operational public
Registry. The active 0.8 generation does not make registry RPC, publication,
source hosting, catalog discovery, or download exist.

Finally, a self-contained Mac product should not require npm, Node, Homebrew,
or a manual drag operation as its normal installation door. A shell one-liner
can be convenient only if it remains consentful, inspectable, pinned to the
immutable artifact, and safe when replacing an existing installation.

## Decision

### Keyboard is a primary native interface

When a create, quick-creation, or evolve intention composer is focused, plain
Return sends the intention. Shift–Return inserts a line. The UI states this
next to the action. Empty input, a disabled action, or an already admitted
command does not submit; the existing exactly-once command guard and durable
application-service admission remain authoritative.

This shortcut changes no intention bytes after submission and creates no
second command path. Buttons and accessibility actions call the same model
operation.

### Registry is an optional local track-record destination

The native sidebar may contain a first-class **Registry** destination. It is a
calm owner-facing view beside the primary App → Intent → App path, not a
restoration of the deleted Studio dashboard or execution-pipeline renderer.
It may use the protocol terms Shot, Evolution, Builder, and Registry because
its purpose is to inspect the track record explicitly.

The destination reads only the existing trusted bundled helper and the same
local workspace snapshot. It projects:

- locally verified accepted Shot heads and their local sequence;
- the current local identity, explicitly labeled local/test-only when that is
  what the helper reports;
- the current client-trusted active generation; and
- whether a public Registry check actually occurred.

It does not add a Swift factory, derive its own verification rules, create a
new Builder identity, or infer publication from contract activation. When
registry RPC is absent, the public card says **Not connected** and the screen
says that nothing shown is published publicly. Records without a verified
accepted local head do not appear in this projection.

The Registry destination also carries a focused **New Shot** composer. It uses
the existing automatic intelligence route and existing create admission. The
full Create App destination remains available for a name, references, and
advanced choices.

### One-line native installer

The canonical consumer command is:

```sh
curl -fsSL https://tohseno.com/install | sh
```

`/download` is an equivalent script alias. `/download/macos` remains the
direct immutable DMG redirect, and `/api/distribution/v1/macos` remains its
machine-readable metadata. The retained `/install.sh` and `/oneshot.sh` stay
as legacy and encrypted-relay claim transports; this decision does not change
their contract.

The native installer is emitted only when the public download configuration
contains an enabled absolute HTTPS DMG URL and an exact lowercase SHA-256.
Otherwise GET and HEAD fail closed with `503`. `curl -I` can inspect status and
instructions in response headers, but a HEAD request never installs anything.

The script explains what it will do, names the source and documentation, shows
the artifact URL and digest, and waits for Return on the controlling terminal.
It then:

1. requires macOS 14 or newer on a supported Mac architecture;
2. downloads only over HTTPS with bounded size and redirects;
3. checks the exact SHA-256 before mounting;
4. verifies the nested code signature, bundle identifier, Developer ID Team,
   and Gatekeeper assessment;
5. installs into `/Applications` when writable or `~/Applications` otherwise,
   without administrator elevation or shell-profile changes;
6. moves a recognized previous installation to the user's Trash before an
   atomic replacement and restores it if replacement fails; and
7. opens the verified app.

It refuses an unrecognized existing path, symlink, unexpected app identity,
running TOHSENO process, failed integrity check, or unavailable interactive
terminal. Temporary deletion is limited to exact installer-owned paths.

## Consequences

The Mac app now acknowledges both kinds of work already present in the
product: making an app quickly and inspecting the private record that proves
what this Mac accepted. Keyboard users can send from every native intention
composer while multi-line writing remains explicit.

The repository contains a complete native one-line installation route, but
that is not a publication claim. It remains unavailable until the accepted
notarized DMG is hosted immutably, downloaded independently, matched to its
configured digest, enabled by the operator, and authorized for publication.

This decision changes no protocol encoding, schema, frozen vector, Shot or
Evolution identity, Builder authority, public contract, active generation, or
registry validation rule. It authorizes no Builder creation, registry RPC,
transaction, receipt, public metadata, source publication, catalog, release
upload, environment activation, or external publication.
