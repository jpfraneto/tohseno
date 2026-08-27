# ADR 0021: A fresh global npm install enters first run

Status: accepted

Date: 2026-08-25

Supersedes ADR 0020 only where its front-door sequence requires a second
`tohseno` command after npm installation. ADR 0020's cable genesis, release
authority, private entitlement, billing boundary, and native bootstrap
verification remain accepted.

## Context

`npm i -g tohseno` successfully installed the JavaScript bootstrap and then
returned only npm's package count. The person received no next instruction.
The product's first-run guide existed behind a separate no-argument `tohseno`
invocation, so the advertised front door stopped before the product appeared.

Printing lifecycle text is not a reliable correction because npm may suppress
successful lifecycle output. The observable outcome of the install itself must
be the first-run product surface.

## Decision

The fresh-Mac sequence is one command:

```text
npm i -g tohseno
    → verified native bootstrap
    → Local Workspace Service
    → cable genesis in Studio
    → App → Intent → App on your iPhone
```

The dependency-free npm package has a postinstall entry point. It runs only for
a global install on macOS and only when no verified installer-owned native
launcher already exists. It invokes the same no-argument bootstrap used by the
explicit `tohseno` command; it does not implement a second installer, service,
genesis state machine, or factory.

Local dependency installation has no machine-level effect. A global npm update
does not reopen first run when native TOHSENO is already installed. npm's
standard `--ignore-scripts` control remains effective; when lifecycle scripts
are deliberately disabled, `tohseno` remains the explicit, idempotent recovery
door.

Failure remains fail-closed. A native download, integrity, Apple-signature, or
activation failure makes postinstall fail instead of opening an unverified
product. Successful verification starts the service and opens the existing
Studio cable guide, which continues to project observable device, Xcode,
signing, Companion installation, launch, and pairing gates from ADR 0020.

## Consequences

The command presented as installation now reaches visible first run without a
hidden second command. The explicit `tohseno install`, `tohseno`, and
`tohseno doctor` commands remain available for recovery and diagnostics.

This decision does not authorize npm publication, a native release, manifest
publication or repinning, signing credential use, billing activation, DNS or
relay activation, or contract generation or deployment.
