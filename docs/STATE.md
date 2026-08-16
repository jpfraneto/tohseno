# State of this repository

Written 2026-07-30, amended through 2026-08-16. This is the plain-language
answer to “what is going on here” for someone returning after time away. When
something below stops being true, update this file in the same change.

## Current source: persistent local factory

ADR 0015 defines the current 0.9.0 product boundary. TOHSENO is an
intention-led app factory whose private backend is the owner's Mac. A
persistent **Local Workspace Service** hosts loopback-only Studio, owns the
durable private command and event journals, monitors execution, and reconciles
paired Companions. CLI, Studio, and mobile commands converge on one Rust
`ShotApplicationService`; none implements a second factory or shells out to
another frontend.

`tohseno create <name>` begins a factory birth from `--prompt`,
`--prompt-file`, bounded piped UTF-8, or an exact regular
`./MASTER_PROMPT.md`, in that order. With no intention in an interactive
session, it starts the Local Workspace Service and opens Studio at the
prefilled creation route. Non-interactive invocation without an intention
fails instead of waiting or creating a partial Shot. Exact intentions and up
to eight safely checked reference images are preserved before execution.

`tohseno evolve <name>` begins an evolutionary transaction bound to one Shot,
one Expression, and one exact accepted base Version. Selected Feedback action
commitments and exact references are part of that durable, idempotent command.
If the base changes before admission, the request is stale and is rejected;
the service never silently rebases it.

Every admitted command is journaled before its semantic action. Stable command,
Shot, and execution identities make retries idempotent and allow recovery
after a process crash. Completion still means that the applicable conception,
Birth Plan, Genome, materialization, build, test, experience, repair, delivery,
and acceptance gates passed. Source files or a successful harness exit alone
are not acceptance. A missing development iPhone yields
`waiting_for_device`, not success.

## Recording-layer compatibility

ADR 0014's app-local recording format and safe-path rules remain accepted;
ADR 0015 changed only its user-facing role. The exact behavior is available
through:

```text
tohseno init <name>
tohseno record [name] --note "..."
tohseno record [name] --note-file note.md
```

The visible app directory remains an ordinary, ejectable working tree.
Recording excludes `.tohseno/` and `.git/` while preserving all other ordinary
files. Existing `.tohseno/recording-layer-v1` directories remain
`recording_only`; no implicit migration, fabricated Shot identity, or factory
execution is permitted. Historical accepted directories and records are read
and verified under the law that produced them, never rewritten or re-signed.

## Local Workspace Service and Studio

The installed service is a user LaunchAgent named
`com.tohseno.workspace-service`. Its stable program path is
`~/.tohseno/bin/tohseno service run`; it requires no `sudo`, starts at login,
and is restarted after unexpected failure. An explicit clean stop unloads the
job. Bounded content-free logs and private service state live beneath
`~/.tohseno/`, separately from visible app folders.

`tohseno studio` ensures the installed service is healthy, opens its verified
loopback origin, and returns to the Terminal. Studio is a visual client of the
same application service. Its principal surfaces are the Shot list,
intention/activity area, current app/execution state, and **CONNECT IPHONE**.
Mutations use exact Origin and anti-CSRF validation; the server does not bind a
public interface or grant permissive CORS. Live status uses the service event
stream rather than reloading the whole workspace once per second.

Service administration is explicit:

```text
tohseno service install|start|stop|restart|status|logs|run|uninstall
```

Ordinary service uninstall removes only a recognized installer-owned
LaunchAgent and preserves app folders, Builder identity, command journals,
and companion pairing state.

## Private Companion channel

The Companion is a signed remote interface, not a Builder identity, wallet,
mobile IDE, source browser, coding harness, public social client, or substitute
for Apple device trust. Its recoverable BIP-39 12-word identity derives
domain-separated Ed25519, X25519, and local-storage keys. Restoring those words
does not restore a workspace capability; the device must pair again.

Studio creates a signed, one-use invitation that expires after approximately
two minutes. Its `tohseno://pair/v1/…` QR contains public rendezvous material
and an allowlisted relay identifier—not recovery words, private keys,
filesystem paths, arbitrary URLs, or permanent credentials. The owner grants
explicit revocable capabilities for workspace/execution reads, feedback,
marketing notes, Shot creation, and evolution. Revocation is checked before
new command admission and event delivery.

Companion traffic is signed and end-to-end encrypted with the versioned suite
defined by `companion/` and the single shared Rust/Swift fixture at
`companion/test-vectors/companion-v1.json`. Mailbox cursors, sender sequences,
stable IDs, acknowledgements, durable outboxes, bounded retention, and snapshot
fallback make delivery reconnectable and idempotent. The Mac is authoritative
for workspace and Shot state; the phone is authoritative only for its unsent
drafts and outbox until acknowledgement. Pending phone envelopes are retried
verbatim, then safely resealed with the same signed command and reference bytes
before relay expiry. The canonical command-admission window is thirty days;
older unacknowledged material stays protected on the phone for explicit owner
resolution while inbound receipts and revocation continue to reconcile.

The separate Bun Companion Relay performs opaque rendezvous and mailbox
delivery. It cannot decrypt content, invoke an agent, build an app, or store
source. APNs, when separately configured, carries only a wake-up signal;
foreground reconciliation works without it. Production relay and APNs
activation remain fail-closed and have not been authorized by the source
change.

The future branded iOS app is intentionally not part of this repository.
`sdk/apple/TohsenoCompanionKit` and its minimal conformance fixture provide the
identity, pairing, envelope, durable-outbox, reconciliation, command, and
revocation primitives for a later `tohseno create tohseno`. Released SDK source
can be vendored into a Shot so generated apps do not depend on a mutable
`~/.tohseno/current` path.

## Public protocol and node remain separate

`protocol/` remains normative over every prose document. Its canonical byte
encodings, schemas, frozen vectors, historical Builder identity behavior, and
verification rules are not changed by private companion transport.

The existing `tohseno-node` remains the Public Node. It validates and preserves
only eligible public lineage evidence and continues to reject private actions.
Private capabilities, envelopes, snapshots, marketing notes, command
provenance, and companion events never enter that system. The web-to-local
handoff from ADR 0011 also remains transport rather than a Shot.

The remediated 0.8 contract generation was deployed to Robinhood Chain mainnet
as a candidate on 2026-08-01 and activated by the recorded owner ceremony on
2026-08-02. That evidence remains under `release/` and `contracts/`. No
contract generation or deployment command is active on current source.

## Repository source versus published release

Repository source and package metadata target **0.9.0**. That is not a claim
that a public 0.9.0 release, production relay, APNs provider, or new installer
pin exists. As of this document's date:

- the public installer still pins immutable **0.8.5**;
- the published website copies must remain byte-identical to that authorized
  installer until 0.9.0 activation;
- no v0.9.0 tag or GitHub release has been created;
- no production companion relay, APNs credentials, DNS, or deployment has been
  activated by this change;
- local 0.9.0 source must pass the readiness gates and be built from a clean,
  captured commit before an owner authorizes publication.

The ordered release, independent checksum verification, service-health,
relay/APNs activation, installer-pin, and rollback instructions are in
`release/V0_9_0_OPERATOR_RUNBOOK.md`. Until every gate is recorded, the public
0.8.5 pin is the honest production state.
