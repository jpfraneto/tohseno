# ADR 0041: Workshop Runtime (Evolution 0040)

Status: accepted

Date: 2026-09-03

This decision implements the live runtime beneath ADR 0039's Living Workshop.
The evolution is numbered 0040 in its implementation brief; ADR number 0040
was already assigned to public app media and the network home, so this record
uses the next available ADR number.

It changes no frozen protocol encoding, generation-0.8 or Claims ABI, Shot
lineage, Claim Edition, Ship-versus-Update rule, Registry authority, Apple
signing boundary, intended-device rule, release gate, or deployed service.

## Context

The Mac and Companion already share durable, signed, end-to-end encrypted
commands through the content-blind relay. That plane is correct for intentions,
approvals, receipts, reconciliation, and offline delivery. It is intentionally
the wrong shape for low-latency interaction while both devices are nearby.

ADR 0039 also projects the Mac factory, intended iPhone, intelligence, and
device capabilities as one workshop. Until this decision, that projection had
no authenticated live Session beneath it. A visual connection could therefore
not truthfully mean that an app could exchange ephemeral events between the
paired devices.

The smallest useful addition is one private local Session plane. It must reuse
the existing pairing authorities, remain unavailable when the intended peer is
ambiguous, and be unable to perform durable product actions.

## Decision

### Two planes remain separate

The existing Companion command plane remains the only path for durable or
human-authorized actions. It retains persistence, offline delivery,
idempotency, receipts, and reconciliation.

The new Workshop Session plane carries only short-lived device capability
snapshots and app-namespaced events while the paired devices are reachable on
the local network. It carries no Claim, Ship, Update, installation, payment,
publication, Registry mutation, device revocation, receipt, or authority
substitute. The shared SDK rejects event namespaces for those actions.

A visual Session connection is evidence only of the currently authenticated
local transport. It is not evidence of durable pairing success, app
installation, publication, or physical acceptance.

### Discovery and authentication

The Mac advertises `_tohseno-ws._tcp` through Bonjour. The advertisement has
no stable device ID, workspace ID, key, token, or capability metadata.

For each start, the native Mac client asks the authenticated loopback Workspace
Service for a random Session ID and 32-byte challenge. The service:

- issues a credential signed by the existing Mac workspace Ed25519 identity;
- limits the credential lifetime to two minutes;
- exposes a peer only when exactly one active Companion pairing is
  unambiguous;
- derives a 32-byte Session key from the existing Mac/Companion X25519 shared
  secret with HKDF-SHA256; and
- binds that key to the challenge, Session ID, workspace, Companion device ID,
  and current revocation epoch.

Long-term private keys are never returned by the service or sent across the
Session. The service returns only the short-lived credential, active peer
public verification material, and derived Session key to the already
authenticated native process.

Companion accepts a host only when the credential verifies under its persisted
paired Mac signing key and matches the exact workspace and Mac device IDs. It
signs a proof with its existing Ed25519 DeviceKey and derives the same Session
key from its X25519 key. The Mac accepts that proof only for the exact active
peer and revocation epoch. Unknown, multiply active, stale, revoked, expired,
or incorrectly signed peers fail closed.

After authentication, events use ChaCha20-Poly1305 with direction-separated
HKDF keys, authenticated direction labels, monotonically increasing sequence
numbers, and versioned typed envelopes. Replayed, out-of-order, oversized,
unknown-version, wrong-Session, or wrong-sender envelopes are rejected. A lost
transport reconnects and performs a fresh handshake; no Session state is
persisted.

### Devices and capabilities

`TohsenoWorkshopKit` defines one typed device and capability model shared by
the Mac, Companion, and Shot-facing SDK:

- stable device identity and platform;
- current transport connection;
- declared capability;
- physical availability;
- operating-system permission;
- reachability; and
- Session authorization.

These facts remain separate. A connected phone does not imply camera or motion
permission, and declared intelligence does not imply an available provider.
Each side sends its local snapshot only after authenticated encryption is
active. Companion reads camera, microphone, and motion permission status from
the operating system without requesting a permission merely for discovery.
The Mac reports intelligence ready only when an installed, usable local/BYO
provider is detected.

### Shot-facing boundary

The Swift package at `sdk/apple/TohsenoWorkshopKit` is the small integration
surface. A Shot can inspect `TohsenoWorkshop.current.devices`, join an installed
Session, consume typed event envelopes, and send an app-namespaced event. A
standalone app receives no fabricated Session and `join()` fails when its host
has not installed one.

A Shot may optionally declare versioned surface roles and required or preferred
capabilities with `tohseno.workshop-shot/1`. No declaration means the existing
focused app and remains runnable. A declaration does not grant a permission or
create a device. Every required surface must resolve to one connected device
whose required capabilities are truly ready; otherwise resolution fails
closed. Preferred capabilities never become requirements.

Factory and adopted-project harness context mentions the Workshop as an
optional capability, but explicitly tells the coding intelligence to preserve
the ordinary focused target unless the exact human intention materially needs
another surface. It must not request unrelated permissions or invent its own
pairing, authentication, or authority.

### Product projection and intelligence

The Living Workshop surfaces the same Session truth in direct language on both
devices. Mac and Companion each show unavailable, discovering, authenticating,
connected, reconnecting, or rejected state and one two-way Workshop Pulse.
Pulse measures the live encrypted round trip and produces a Companion haptic;
it performs no command, build, installation, or approval.

Intelligence is presented as a capability already available on the Mac. The
primary creation and evolution path automatically uses the best installed and
authenticated supported provider. Exact local/BYO choice remains under one
Advanced disclosure. The unfinished managed-credits purchase surface is not
shown; `Tohseno Intelligence` is described only as coming soon. Historical
managed receipts and the fail-closed backend boundary remain readable for
compatibility, but are not restored into a new request.

## Security and privacy consequences

- An unpaired LAN peer may discover only that a Tohseno Workshop service is
  present. It cannot authenticate or decrypt a Session.
- Compromise of an ephemeral Session key does not export either long-term
  X25519 key and does not confer durable product authority.
- Bonjour and Network.framework are local transports, not cloud identity or a
  third-party authentication substitute.
- Capability snapshots remain private to the authenticated Session and are
  never published to Registry.
- Session loss can lose ephemeral events by design; anything requiring durable
  delivery must use the existing Companion command plane.

## Verification and release truth

Focused tests cover credential/proof success, wrong signing keys, unknown and
stale-revocation peers, key derivation, encrypted event round trips, replay and
authority-event rejection, capability truth, required-capability failure,
focused-app fallback, and reconnect state. Native tests retain the existing
Mac, Companion, distribution, adoption, Claim, and Ship/Update projections.

This source change prepares RC10. Local verification may assemble an unsigned
app bundle, but it does not sign, notarize, staple, publish, activate, or deploy
an RC10 release artifact. RC9 remains the exact public installer until
separately authorized release work produces and verifies new immutable bytes.

## Consequences

The Mac and intended paired iPhone can now act as one real low-latency workshop
without turning the phone into a factory or the Session into authority. Focused
apps keep working without metadata or Session availability. Richer surfaces can
be added only as typed capabilities and events whose runtime truth is visible;
they cannot be inferred from animation or product prose.
