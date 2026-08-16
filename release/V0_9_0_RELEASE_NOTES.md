# TOHSENO 0.9.0 release notes

Status: source release candidate; not yet published or activated

TOHSENO 0.9.0 restores the product as an intention-led app factory and makes
that factory a persistent private service on the owner's Mac. Studio, CLI, and
a paired iPhone Companion use one shared application service and the existing
engine acceptance law. The cloud component is a content-blind encrypted
mailbox, never a build or source host.

## Product change from 0.8.5

- `tohseno create <name>` again begins a durable factory birth from an exact
  intention and bounded references.
- `tohseno evolve <name>` begins an evolutionary transaction bound to the
  exact current Expression and accepted base Version. Stale bases fail closed.
- ADR 0014's app-local recording behavior moves intact to `tohseno init` and
  `tohseno record`; existing recording-only folders are not migrated.
- One `ShotApplicationService` is shared by CLI, Studio, companion command
  processing, and conformance clients. Stable command IDs and the durable
  journal provide exactly-once semantic actions across retries and crashes.
- Executions continue after the invoking Terminal closes. Acceptance still
  requires the applicable plan, materialization, build, test, experience,
  device delivery, launch, and verification gates.

## Persistent Local Workspace Service

The installer-managed user LaunchAgent is
`com.tohseno.workspace-service` and invokes the stable
`~/.tohseno/bin/tohseno service run` launcher. It starts at login, restarts
unexpected failures, and can be stopped explicitly without a restart loop.
Studio binds only to loopback. `tohseno studio` starts or verifies the service,
opens the authenticated local origin, and returns.

Service administration is available through `tohseno service install`,
`start`, `stop`, `restart`, `status`, `logs`, `run`, and `uninstall`. Private
service state, journals, pairing records, and visible app folders are outside
immutable release payloads.

## Companion pairing and synchronization

Studio adds **CONNECT IPHONE** and paired-device management. An owner-created
pairing invitation is signed, one-use, approximately two minutes long, and
rendered as a standard QR inside the TOHSENO pairing seal. It carries an
allowlisted relay identifier and public rendezvous material only.

The Companion identity is a revocable device identity recoverable from a
standard BIP-39 12-word phrase. It does not replace the Builder identity,
become a wallet, or acquire canonical control of historical Shots. Pairing
grants an explicit workspace-scoped capability set. Revocation is checked
before every new command and event.

Recipient-specific signed ChaCha20-Poly1305 envelopes use domain-separated
X25519/HKDF key agreement and Ed25519 authentication. Mailbox cursors, sender
sequences, stable event and command IDs, delivery and command
acknowledgements, durable outboxes, bounded retention, and snapshot fallback
support offline reconciliation without promising an indefinitely alive iOS
socket.

The Companion can receive bounded workspace summaries and privacy-safe
execution states, then request:

- exact-Version feedback;
- private Shot-bound marketing notes;
- an exact-base evolution; or
- a new Shot from an exact intention.

It never receives source code, source filenames, harness credentials,
transcripts, or model output. Apple signing, device trust, Developer Mode,
installation, and acceptance remain Mac responsibilities.

## Companion Relay and APNs

The Bun Companion Relay is separate from both the historical browser-intention
relay and `tohseno-node`. It provides opaque pairing rendezvous, recipient
mailboxes, cursor catch-up, acknowledgement, bounded retention, live wake-up,
revocation propagation, and optional APNs dispatch. It cannot decrypt private
content, execute code, invoke a harness, or authorize an engine action.

Production relay startup fails closed without an explicit durable root and
activation assertion. APNs has production, fake, and intentional no-op modes;
declared production APNs fails closed when its key, team, topic, environment,
or key path is missing or malformed. Push contains no private content and CI
requires no Apple credential.

## Apple CompanionKit

`sdk/apple/TohsenoCompanionKit` is a self-contained async Swift package with:

- BIP-39 identity generation and restoration;
- domain-separated Ed25519, X25519, and storage-key derivation;
- injectable Keychain-backed secret storage;
- invitation, proof, capability, envelope, command, event, and snapshot models;
- relay transport, encrypted durable outbox, cursor reconciliation, and
  revocation handling;
- feedback, marketing, evolution, and creation APIs;
- connection and workspace event streams; and
- APNs token registration as a transport adapter.

Rust and Swift consume the single checked-in
`companion/test-vectors/companion-v1.json` fixture, including positive bytes,
tampering, and replay cases. The minimal conformance app proves the protocol
surface without preempting the later branded TOHSENO iOS product. The
vendoring helper copies an immutable CompanionKit source snapshot, license,
vectors, and SHA-256 inventory into a generated Shot.

## Compatibility and migration

There is no automatic data migration in 0.9.0:

- historical accepted protocol bytes and signatures remain unchanged;
- the existing 24-word Builder recovery behavior remains canonical;
- `.tohseno/recording-layer-v1` folders remain readable and `recording_only`;
- ordinary app folders remain in their existing locations;
- service updates preserve Builder identity, journals, and pairing state; and
- private companion records never enter Public Node replication.

## Installation and update

The 0.9.0 installer contract continues to download verified compiled release
artifacts rather than cloning the monorepo. It stages a release beneath
`~/.tohseno/releases`, atomically switches `current`, restarts the service,
checks exact versioned health, and rolls back the pointer and service if health
fails. Default uninstall removes installer-owned program and LaunchAgent
artifacts while preserving apps and identity.

`tohseno update` accepts only GitHub's immutable, non-draft stable release. It
binds both `oneshot.sh` and `SHA256SUMS` to GitHub's canonical SHA-256 asset
metadata, verifies the installer again through that aggregate manifest, and
rejects redirects outside the exact GitHub release-asset host allowlist before
executing any shell bytes.

## Activation status

The public installer remains pinned to immutable 0.8.5. No v0.9.0 tag, GitHub
release, production Companion Relay, APNs activation, DNS change, or public
installer change is authorized by these source changes. Follow
`V0_9_0_OPERATOR_RUNBOOK.md` only after explicit owner authorization and
record every gate in `V0_9_0_READINESS.json`.
