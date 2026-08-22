# Tohseno v0 architecture

This is the concrete runtime map for the private iPhone-to-Mac factory. It is
descriptive, not protocol authority: [`protocol/`](../protocol/) and the
accepted [ADRs](adr/README.md) win if this document disagrees with them.

```text
iPhone Companion
  signed command + encrypted durable outbox
                    |
                    v
          content-blind relay
                    |
                    v
        Local Workspace Service
  authenticate -> admit -> journal -> execute
                    |
                    v
       ordinary app folder / Shot
```

The iPhone is the wand. The Mac is the factory. The relay is a bounded opaque
mailbox; it is neither a factory nor an authority over a Shot.

## Runtime components

### Companion

The shipping iOS product is
[`companion/apple/TohsenoCompanion`](../companion/apple/TohsenoCompanion/).
[`CompanionModel.swift`](../companion/apple/TohsenoCompanion/Sources/TohsenoCompanionApp/CompanionModel.swift)
owns the thin product flow, and
[`CompanionBackend.swift`](../companion/apple/TohsenoCompanion/Sources/TohsenoCompanionApp/CompanionBackend.swift)
constructs the production client. The transport and persistence implementation
lives in
[`sdk/apple/TohsenoCompanionKit`](../sdk/apple/TohsenoCompanionKit/), principally
[`Client.swift`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift)
and
[`Storage.swift`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Storage.swift).

The app reads a privacy-safe workspace projection, binds an evolution to the
displayed Shot, Expression, accepted Version, and ordinal, and asks the SDK to
queue it. The SDK returns only after the signed command, encrypted envelope,
and any encrypted reference payloads have been persisted. Network delivery is
then best-effort and repeatable.

### Local Workspace Service, Studio, and factory

[`cli/src/workspace_service.rs`](../cli/src/workspace_service.rs) is the
loopback-only, long-lived Mac service. It owns the one
[`ShotApplicationService`](../application/src/application_service.rs), opens
the command journal, recovers it before accepting traffic, serves Studio, and
polls paired relay mailboxes independently of any Terminal or browser window.
The installed process is the user LaunchAgent
`com.tohseno.workspace-service`; its lifecycle and paths are in
[`cli/src/service_commands.rs`](../cli/src/service_commands.rs).

Studio in [`studio/`](../studio/) is a thin browser projection of that service.
It is not a second factory. CLI, Studio, and Companion commands all converge on
the same application service. The removed dashboard, pipeline renderer,
Feedback/Marketing forms, and manual Version controls are intentionally absent
under ADR 0016.

The application layer in [`application/`](../application/) owns admission,
idempotency, recovery, the machine-wide factory lease, and detached execution.
The engine in [`engine/`](../engine/) owns Shot state, the exact lifecycle,
build/test/verification/delivery gates, and accepted lineage. Actual unattended
work enters through `tohseno shot run`, launched by
[`application/src/execution_manager.rs`](../application/src/execution_manager.rs).

### Companion relay

[`website/apps/companion-relay`](../website/apps/companion-relay/) is a separate
Bun service. Routes are in
[`src/routes.ts`](../website/apps/companion-relay/src/routes.ts), filesystem
mailboxes are in
[`src/storage.ts`](../website/apps/companion-relay/src/storage.ts), and
production fail-closed configuration is in
[`config.ts`](../website/apps/companion-relay/config.ts).

The relay validates routing metadata, bearer capabilities, sizes, cursors,
sender sequence watermarks, expiry, and capacity. It persists opaque bytes and
cannot decrypt a command, inspect an intention, admit a Shot mutation, or start
execution. Retention is bounded. APNs, when configured, is only a content-free
wake-up hint; correctness uses mailbox reconciliation.

### Identities, keys, and pairing

The shared private wire contract and its Rust verifier live in
[`companion/`](../companion/). The important modules are
[`identity.rs`](../companion/src/identity.rs),
[`pairing.rs`](../companion/src/pairing.rs),
[`command.rs`](../companion/src/command.rs),
[`capability.rs`](../companion/src/capability.rs), and
[`envelope.rs`](../companion/src/envelope.rs). Swift implements the same bytes
in the correspondingly named SDK files and is checked against
[`companion/test-vectors/companion-v1.json`](../companion/test-vectors/companion-v1.json).

The Mac's workspace seed is held through Keychain by
[`cli/src/workspace_identity.rs`](../cli/src/workspace_identity.rs). The phone's
BIP-39 identity derives domain-separated Ed25519 signing, X25519 agreement, and
local storage keys; its identity stays in Keychain. Commands are signed by the
phone and envelopes are end-to-end encrypted to the Mac. Command receipts and
workspace events are signed by the Mac and encrypted to the phone.

Pairing is a signed, one-use, short-lived invitation. The relay carries an
opaque encrypted response. The Mac verifies proof of the phone identity and
issues a revocable capability grant plus two directional mailbox capability
sets. Recovery words restore the phone identity, not the workspace grant;
pairing must be repeated.

### Shots, commands, and execution

A Shot is the factory identity and accepted history behind an ordinary app
folder. Its layout and engine persistence are implemented by
[`engine/src/shot_layout.rs`](../engine/src/shot_layout.rs),
[`engine/src/ledger.rs`](../engine/src/ledger.rs), and
[`engine/src/machine.rs`](../engine/src/machine.rs). An evolution is bound to
one exact accepted base. Both the Companion coordinator and application service
check it; stale work is durably rejected and never silently rebased.

The application command state machine is in
[`application/src/command.rs`](../application/src/command.rs). Its filesystem
journal is in [`application/src/journal.rs`](../application/src/journal.rs).
Before semantic work begins it stores immutable request metadata, canonical
payload bytes, and exact reference inputs. The execution manager then prepares
a stable app-local execution and starts a detached runner. The runner performs
the bounded harness work and deterministic gates defined by ADR 0019. A Version
is accepted only after those gates pass; harness exit alone is not success.

## Persistence map

Default locations are shown; verification scripts override them with isolated
roots.

| State | Default location | Owner | Survives |
|---|---|---|---|
| Phone identity | iOS Keychain | Companion SDK | app termination and ordinary relaunch |
| Phone pairing, workspace projection, replay state, command outbox | protected, encrypted Application Support `TOHSENO/companion-state.bin` | Companion SDK | app termination and device restart |
| Exact encrypted reference/envelope copies | protected Application Support `TOHSENO/outbox/` | Companion SDK | app termination; removed after a verified Mac receipt |
| Pairing rendezvous and opaque mailboxes | configured absolute `COMPANION_RELAY_ROOT` | Companion Relay | relay process restart, within retention and available storage |
| Workspace identity | Keychain plus `~/.tohseno/service/workspace.json` reference | Local Workspace Service | service and Mac restart |
| Paired devices, relay cursors, admitted envelopes/commands, reference inbox, Mac outbox | `~/.tohseno/service/{devices,inbox,outbox}` | Companion coordinator | service and Mac restart |
| Durable commands and exact inputs | `~/.tohseno/service/command-journal/<command-id>/` | application service | service and Mac restart |
| Visible app and accepted Shot state | normally `~/Desktop/Tohseno/<app>/` | engine | service restart; the folder remains ejectable |
| Prepared/running execution, events, completion, private receipt | `<app>/.tohseno/executions/<execution-id>/` | execution manager and engine | service restart and, except for the limitation below, Mac restart |
| Factory serialization lease | private machine data root | application service | released automatically when the owning process exits |

On service startup, command recovery runs before the loopback listener opens.
A prepared execution is started; a live detached runner is reattached; a
verified candidate waiting for a device resumes deterministic delivery. A Mac
restart during arbitrary harness mutation is not replayed: if no runner remains
for an in-flight harness phase, the execution is finalized as failed to avoid a
second intelligence pass over unknown partial mutations.

## Repository map and authority

The current private core is:

- `companion/apple/TohsenoCompanion/` — shipping phone product.
- `sdk/apple/TohsenoCompanionKit/` and `companion/` — Swift/Rust private wire,
  cryptography, state, and conformance vectors.
- `website/apps/companion-relay/` — content-blind internet mailbox.
- `cli/`, `application/`, and `engine/` — Local Workspace Service, durable
  application boundary, and Mac factory.
- `studio/` — thin local UI served by the service.

Important but separate from this private loop:

- `protocol/` is normative public protocol law; `node/`, `contracts/`, and
  `release/` concern public evidence and releases.
- `website/apps/site/` is the public web terminal and its separate web-to-local
  intention handoff. A relay record there is not a Shot.
- `apple-identity/`, `fascia/`, and `oneshot/` support Mac identity, generated
  iOS source, and installation/release respectively.

Compatibility and historical material is intentionally not a second active
architecture. `MASTER_PROMPT.md`, `genome/LAWS.md`, `history/`, historical
release documents, and readable legacy lifecycle variants remain for byte and
record compatibility. `sdk/.../Examples/CompanionConformanceApp` and
`cli/src/companion_simulator.rs` are verification fixtures, not alternate
products. Old names such as readable `Conception` execution variants do not
authorize the removed Conception phase for new births.

For the boundary-by-boundary operational trace, see
[`GOLDEN_PATH.md`](GOLDEN_PATH.md).
