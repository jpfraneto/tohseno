# ADR 0015: TOHSENO is a persistent local app factory with a private companion channel

Status: accepted

Date: 2026-08-15

Supersedes: ADR 0014 as the description of the current user-facing `create`,
`evolve`, and Studio product. ADR 0014's app-local recording format, byte
layout, safety rules, and compatibility guarantees remain accepted and are
available explicitly through `tohseno init` and `tohseno record`.

## Context

ADR 0014 made the ordinary app folder ejectable and preserved a useful local
recording format, but it also removed the intention-led factory from the
product commands. That left Studio tied to a Terminal process and provided no
safe way for the owner to operate the same factory from a paired iPhone.

The repository already contains the engine law for conception, Birth Plans,
app-specific Genomes, unattended harness execution, deterministic acceptance,
exact-Version Feedback, delivery, and historical verification. Restoring the
product must reuse that engine instead of creating separate CLI, Studio, or
mobile implementations. It must also keep private companion traffic distinct
from the public witness protocol and the `tohseno-node` implementation.

## Decision

TOHSENO owns an intention-led unattended app-factory path again. The user's
Mac is the factory and private backend. One persistent **Local Workspace
Service** hosts loopback-only Studio, owns the private command and event
journals, monitors executions, synchronizes paired companions, and invokes one
shared Rust application service. CLI, Studio HTTP handlers, the companion
command processor, and conformance clients all call that application service;
no frontend calls or shells out to another frontend.

`tohseno create` creates a Shot through the factory and `tohseno evolve`
creates an exact-base evolutionary transaction. The command is durably
recorded before execution and has a stable idempotency key, Shot identity, and
execution identity. The detached Local Workspace Service owns continued work
after an invoking Terminal exits. Completion continues to mean accepted engine
state after all applicable conception, materialization, build, test,
experience, device-delivery, and verification gates—not the mere existence of
source files or a successful harness exit.

The visible app folder remains an ordinary, ejectable working directory.
ADR 0014's exact behavior moves to `tohseno init` and `tohseno record`. A
folder containing `.tohseno/recording-layer-v1` remains `recording_only` and
must not enter the factory pipeline. There is no implicit conversion, ShotID
fabrication, or silent migration. An explicit future migration would require
a separate accepted decision.

The Local Workspace Service is installed as a user LaunchAgent and served
through a stable installer-controlled launcher. Studio is an authenticated
window into that service, not another backend. The HTTP implementation uses a
maintained Rust HTTP stack, binds only loopback, validates Host and exact
Origin, requires an unguessable anti-CSRF token for mutations, bounds headers
and bodies, rejects non-JSON mutations, emits no permissive CORS policy, and
uses a streaming event endpoint. The phone never calls the loopback API.

The service uses an atomic filesystem command journal under the private
service root. Each command occupies one validated, non-symlinked directory;
its immutable request is published with create-new/rename semantics and its
state is replaced atomically. This preserves inspectability and avoids adding
an embedded database migration boundary to the release. Per-command operation
markers make compound feedback-and-evolution processing crash recoverable.
The journal is the idempotency authority: replaying a command returns the same
receipt and never repeats its semantic action.

A paired **Companion** is a signed remote interface to this application
service. It is not a Builder identity, crypto wallet, coding harness, mobile
IDE, source browser, or public social client. A recoverable BIP-39 12-word
Companion identity signs private transport commands. The Mac verifies the
device signature and current capability, then performs canonical engine
actions under the existing Builder identity. Restoration of the Companion
phrase does not restore a revoked or absent workspace capability.

Pairing invitations are signed, one-use, approximately two-minute records.
Their versioned `tohseno://pair/` URI contains public rendezvous material and
an allowlisted relay identifier only. It never contains recovery words,
private keys, permanent bearer credentials, filesystem paths, or an arbitrary
network origin. Both peers prove key possession and the final capability grant
is encrypted to the phone.

Capabilities are explicit, signed, workspace-scoped, and revocable:
`workspace.read`, `execution.read`, `feedback.write`, `marketing.write`,
`shot.create`, and `shot.evolve`. Revocation advances the device epoch before
new command admission or event delivery. Old envelopes and restored phones
cannot regain authority without a new owner-created pairing session.

The private companion suite is versioned and cross-language: BIP-39,
HKDF-SHA-256 with explicit `tohseno.companion.*.v1` domains, Ed25519
signatures, X25519 agreement, ChaCha20-Poly1305 authenticated encryption, and
SHA-256 commitments. Canonical signed objects use RFC 8785 bytes and fixed UTC
timestamp syntax. Rust and Swift consume the same checked-in positive and
negative fixtures.

The **Companion Relay** is a separate Bun service. It stores and delivers only
bounded, recipient-specific authenticated ciphertext plus minimum routing
metadata. It performs pairing rendezvous, cursor catch-up, acknowledgements,
bounded retention, live wake-up, rate limiting, revocation propagation, and
optional content-free APNs wake-ups. It cannot interpret commands, run an
agent, build an app, store source, or authorize a Shot action. Production
storage, origins, retention, and declared APNs configuration fail closed.

The Mac is authoritative for workspace and Shot state. The phone is
authoritative for its unsent drafts and durable outbox until command
acknowledgement. Synchronization uses stable event IDs, per-sender sequences,
per-mailbox cursors, snapshot versions, delivery and command acknowledgements,
bounded retention, reconciliation on launch, and full-snapshot fallback.
“Always synced” means near-real-time when active plus safe offline replay; it
does not promise an indefinitely running iOS socket.

Private companion envelopes, capabilities, marketing notes, command
provenance, and snapshots are never ordinary public lineage and never enter
`tohseno-node`. Exact-Version Feedback and accepted factory Evolutions continue
through the existing canonical engine paths after private authorization.
Historical public protocol encodings and all frozen fixtures remain unchanged
and independently verifiable. The web-to-local handoff remains transport and
does not become a Shot or companion capability.

## Security and operational consequences

Service private keys are held through an injectable secret-store boundary;
production uses the macOS Keychain and tests use isolated storage. Filesystem
state contains public metadata, encrypted records, or intentionally local
private records with restrictive permissions. Operational logs exclude
prompts, private paths, secrets, tokens, ciphertext, content digests, complete
relay identifiers, APNs tokens, source filenames, and harness output.

The LaunchAgent is user-level, requires no `sudo`, starts at login, and restarts
unexpected failures. Explicit administration can stop it without a restart
loop. Release updates atomically change the installer-controlled `current`
pointer, restart the service, verify versioned health, and roll back the
pointer and service if health fails. App folders, Builder identity, command
journals, and pairing state are not release payloads and are never deleted by
an ordinary update or uninstall.

Companion pairing does not replace Apple device trust, Developer Mode,
provisioning, signing, or physical delivery. Harness authentication, route
selection, inference cost, Xcode, signing, installation, and final acceptance
remain on the Mac. When the required iPhone is unavailable, a valid execution
may continue through source work but must become `waiting_for_device` before
delivery and may not report acceptance.

No contract generation, public-witness deployment, release publication, live
DNS change, or production APNs activation is authorized by this decision.
Existing activation gates remain fail-closed.

## Consequences

TOHSENO again has one product-level factory with three interfaces: Studio,
CLI, and the Companion. Commands survive Terminal closure and intermittent
phone or relay connectivity. Duplicate delivery is harmless, stale evolution
bases fail closed, revocation is immediate at local admission, and private
content remains end-to-end encrypted through shared infrastructure.

The recording layer remains useful and byte-compatible, but is an explicit
capability rather than the meaning of creation and evolution. Existing
recording-only folders remain readable and unmodified. Existing historical
protocol artifacts, Builder identities, 24-word recovery behavior, canonical
encodings, signatures, node validation, and deployment boundaries are not
reinterpreted or rewritten.
