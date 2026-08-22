# TohsenoCompanionKit 0.9.9

`TohsenoCompanionKit` is the native Apple client for TOHSENO's private
companion channel. It does not contain a coding harness, public-node client,
wallet, source browser, or loopback-Studio client.

The package provides:

- standard 12-word BIP-39 identity creation and restoration;
- Keychain-backed secret storage using `WhenUnlockedThisDeviceOnly`;
- HKDF-SHA-256 domain-separated Ed25519, X25519, and storage keys;
- allowlisted, signed, expiring pairing invitations and proof of possession;
- signed revocable capabilities;
- recipient-specific X25519/ChaCha20-Poly1305 envelopes with replay defense;
- an encrypted durable state journal plus a separately bounded protected
  payload store for the offline command/reference outbox;
- cursor reconciliation, snapshots, incremental events, and full-snapshot gap
  detection;
- an explicit encrypted `product.entitlement` projection without payment
  details or public lineage;
- authenticated, bounded PNG/JPEG `icon.blob` events with SHA-256 and decoded
  dimension verification plus an encrypted-at-rest cache;
- exact-version feedback, private marketing notes, evolution, and new-Shot
  command APIs;
- exact PNG/JPEG creation/evolution references up to 64 MiB each, split into
  canonical 8 MiB chunks with per-chunk and whole-object SHA-256 commitments;
- content-free APNs wake registration, a bounded foreground SSE synchronizer,
  and connection/workspace streams.

## Embedding an immutable copy

From a TOHSENO source release, run:

```sh
python3 sdk/apple/vendor-companion-kit.py --into /path/to/Shot
```

This publishes `Vendor/TohsenoCompanionKit`, includes the release license and
the exact shared Rust/Swift test vector, normalizes file modes and timestamps,
and writes a SHA-256 manifest. It refuses to overwrite an existing package or
follow a symlink destination. Generated apps should commit this directory and
use a relative Swift Package dependency.

In a future Shot's `MASTER_PROMPT.md`, declare companion connectivity as an
explicit required capability rather than prescribing a mutable SDK path. For
example:

```text
This app is a TOHSENO Companion. It must use the exact vendored
TohsenoCompanionKit release supplied by the factory for identity, pairing,
encrypted synchronization, offline outbox, commands, and revocation. It must
not implement a second wire protocol or resolve package source from
~/.tohseno/current.
```

The accepted Birth Plan can then require the factory's CompanionKit material
and the harness can add `Vendor/TohsenoCompanionKit` as a local Swift Package
dependency. Product design—home screen, phrase ceremony, camera scanner, Shot
grid, feedback, marketing, and create surfaces—belongs in that later prompt,
not in this SDK fixture.

## Pairing and background behavior

`TohsenoCompanionClient.pair(with:displayName:)` accepts only a relay identifier
already present in the app's `RelayAllowlist`. The QR cannot choose a URL. The
phone creates a response mailbox, sends an encrypted proof-of-possession body,
and accepts a capability only when it is signed by the Studio key from the QR.

APNs carries no workspace data. A wake calls `handlePushWake()`, which performs
ordinary authenticated mailbox reconciliation. While the app is active, call
`startForegroundSynchronization()` once and call
`stopForegroundSynchronization()` when leaving the active foreground. The
client reconciles before subscribing, consumes the relay's content-blind live
wakes, reconnects with bounded exponential backoff, and cancels the HTTP stream
cleanly. It does not claim that iOS keeps a socket alive indefinitely.

If relay retention has advanced beyond the durable phone cursor, the client
clears guessed incremental state and submits one signed, idempotent
`workspace.snapshot.request` under `workspace.read`. The Mac answers with a
fresh encrypted authoritative snapshot. Repeated reset delivery reuses the
same deterministic command ID and cannot create an unrelated factory action.

Camera scanning and APNs authorization remain app-level adapters. The minimal
fixture under `Examples/CompanionConformanceApp` demonstrates that boundary.

`TohsenoCompanionClient` requires both a `CompanionStateStore` and a
`CompanionPayloadStore`. Production apps should use `FileCompanionStateStore`
and `FileCompanionPayloadStore` under a protected Application Support
directory. The payload store contains the exact signed/encrypted relay envelope
and a separately identity-key-encrypted canonical chunk copy for each pending
reference. Both are bounded protected files, so a maximum-size offline outbox
does not have to be rewritten or loaded as one monolithic state file, and no
plaintext reference is written to disk. The durable queue admits at most 32
pending commands and 2,048 reference chunks (4,096 protected payload files).

Create and evolve requests accept `[CompanionReferenceBlob]`, not detached
descriptors. CompanionKit validates the declared PNG/JPEG header, exact bytes,
origin filename, length, and digest; durably queues every deterministic chunk;
uploads those envelopes first; and only then uploads the signed command whose
reference descriptors commit to the same blobs. Relaunch and foreground
reconciliation retry the exact same envelope IDs until a Mac-signed command
receipt arrives, and relay duplicate delivery therefore remains idempotent.
If an unacknowledged envelope reaches its bounded six-day lifetime, the SDK
uses the protected canonical copy to reseal the same chunk under fresh routing
metadata; the command ID and committed bytes do not change. Receipt processing
is persisted before those files are retired, while revocation erases the whole
pending payload store. Canonical companion commands have a 30-day offline
admission limit. After that boundary the SDK preserves the outbox record and
reports an explicit error; it does not re-sign the same command ID with a new
digest that could conflict with a Mac journal entry whose receipt was lost.

Workspace snapshots contain only opaque icon descriptors. The Mac sends exact
image bytes afterward as recipient-encrypted `icon.blob` events; the relay
never sees an icon, Shot name, or blob commitment. Call
`iconBlob(for:)` with a snapshot descriptor to obtain cached bytes. The SDK
rejects undeclared blobs, mismatched revisions or commitments, malformed image
headers, dimensions above 2048 pixels, and payloads above 2 MiB.
