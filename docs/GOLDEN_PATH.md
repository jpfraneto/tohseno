# Evolution golden path

This is the failure-walk for one real Companion evolution. It describes the
current implementation; [`protocol/`](../protocol/) remains authoritative over
normative public bytes.

```text
CompanionModel.evolve
  -> EvolutionRequest
  -> signed CompanionCommand + encrypted durable phone outbox
  -> opaque Companion Relay mailbox
  -> CompanionCoordinator reconciliation on the Mac
  -> signature, replay, capability, and exact-base admission
  -> EvolveShotCommand
  -> ShotApplicationService.evolve_shot
  -> filesystem command journal + prepared execution
  -> detached `tohseno shot run`
  -> harness, deterministic gates, accepted Version
```

## Boundary trace

### 1. Finger tap to durable phone command

[`CompanionModel.evolve()`](../companion/apple/TohsenoCompanion/Sources/TohsenoCompanionApp/CompanionModel.swift)
copies the current workspace snapshot's `shotID`, `expressionID`, `versionID`,
and ordinal into an
[`EvolutionRequest`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift).
`TohsenoCompanionClient.requestEvolution` converts that into the
`shot_evolve_request` payload of a signed
[`CompanionCommand`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Commands.swift).

The SDK validates the grant and references, signs canonical bytes with the
phone Ed25519 key, encrypts reference chunks and the command to the Mac X25519
key, appends `PendingCompanionCommand`/`PendingReferenceChunk` records, and
persists encrypted `CompanionPersistentState` before attempting the network.
The call does not report `received` until that persistence succeeds.

- Persistence: encrypted Application Support state and separately encrypted
  outbox payload files in
  [`Storage.swift`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Storage.swift).
- Trust boundary: raw human intent becomes signed end-to-end-encrypted wire
  material. The phone grant limits which command kind may be signed for this
  workspace.
- Retry/idempotency: the same envelope ID and bytes are retried while valid;
  after relay expiry the same signed command/reference bytes are resealed in a
  new envelope. The stable command ID is the semantic idempotency key.
- Failure: validation or local persistence fails the tap. Transport failure
  leaves the request durably queued and reports a reconnecting state.
- Restart: a later app launch/foreground reconciliation reloads and flushes the
  outbox. iOS does not promise indefinite background execution, so delivery
  waits for a later run if the OS does not wake the app.

### 2. Phone outbox to content-blind relay

[`Client.flushOutbox`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Client.swift)
uploads all reference envelopes before the command envelope through
[`Relay.swift`](../sdk/apple/TohsenoCompanionKit/Sources/TohsenoCompanionKit/Relay.swift).
The Bun routes in
[`website/apps/companion-relay/src/routes.ts`](../website/apps/companion-relay/src/routes.ts)
authorize a mailbox write capability and pass opaque canonical bytes to
[`CompanionRelayStorage`](../website/apps/companion-relay/src/storage.ts).

- Persistence: one atomic mailbox metadata file plus create-exclusive opaque
  envelope files below `COMPANION_RELAY_ROOT`.
- Trust boundary: the relay sees mailbox and device routing IDs, sequence,
  timestamps, sizes, and ciphertext. It does not possess content keys and
  cannot authorize a Shot command.
- Retry/idempotency: equal envelope ID plus equal digest returns the original
  cursor; conflicting reuse and non-increasing sender sequences fail closed.
- Failure: unavailable disk, capacity, invalid capability, unsafe paths, or
  malformed routing reject the upload. The phone retains its outbox. Retention
  expiry creates a cursor reset, after which the phone can reseal the same
  signed semantic command.
- Restart: relay metadata, high-water marks, cursors, and opaque bytes survive a
  process restart. Acknowledgement, expiry, cancellation, and revocation commit
  their logical state before deleting payload bytes, so interrupted cleanup is
  safe to resume.

### 3. Relay to authenticated Mac admission

Every two seconds the loop in
[`cli/src/workspace_service.rs`](../cli/src/workspace_service.rs) calls
[`CompanionCoordinator.reconcile_relay_once`](../cli/src/companion_service.rs),
whether or not Studio or a Terminal is open. The coordinator fetches bounded
pages, decrypts each envelope, verifies its header and phone signature, applies
replay protection, resolves reference chunks, verifies the current revocable
capability grant and command signature, and checks the command admission
window.

For `ShotEvolveRequest`, `process_command_uncached` resolves the current Shot,
checks the exact Expression/Version/ordinal, reconstructs exact reference
bytes, and creates an
[`EvolveShotCommand`](../application/src/application_service.rs) with
`origin = Companion`.

- Persistence: admitted envelope results live under
  `~/.tohseno/service/inbox/envelopes`; processed command receipts under
  `inbox/commands`; reference chunks under `inbox/blobs`; paired device grant,
  replay cursor, and last-seen state under `devices`.
- Trust boundary: opaque relay bytes become authenticated private plaintext.
  Relay authorization is not command authorization; Ed25519, X25519,
  capability, revocation epoch, replay, and exact Shot checks all happen here.
- Retry/idempotency: an identical admitted envelope returns its stored result.
  A resealed duplicate reaches command-level lookup and returns the stored
  result for the same signed command digest. Conflicting reuse fails closed.
- Failure: missing/corrupt chunks, bad signatures, revoked or insufficient
  capability, replay, expired admission, unknown Shot, or stale base is rejected
  without execution.
- Restart: local admission records and device cursors survive. The service
  processes and publishes the Mac-signed command receipt before storing its
  local relay cursor, then stores that cursor before relay ACK. Crashes at
  either edge cause only idempotent replay/ACK.

### 4. Application admission to durable execution

[`ShotApplicationService.evolve_shot`](../application/src/application_service.rs)
first calls
[`CommandJournal.admit_with_files`](../application/src/journal.rs). The journal
creates `request.json`, canonical `payload.json`, exact `inputs/`, and atomic
`status.json` under `~/.tohseno/service/command-journal/<command-id>/`. Only
then does the service validate semantics, recheck the exact accepted base,
resolve the configured harness, and transition `received -> validated ->
accepted`.

The service calls `Engine.evolve_exact`, then
[`execution_manager::prepare_for_command`](../application/src/execution_manager.rs).
The stable execution ID is derived from the command ID. Preparation writes the
intent/reference package and an execution record beneath
`<app>/.tohseno/executions/<execution-id>/` before the command becomes
`running`. `ensure_background_runner` starts the detached
`tohseno shot run --app ... --execution ...` process and a monitor.

- Persistence: the command journal is the admission/recovery authority; the
  app-local execution directory is the runner/completion authority.
- Trust boundary: authenticated command intent becomes permission to mutate
  the owner's local Shot. Exact base, exact reference digests, Builder binding,
  safe filesystem paths, and harness selection are checked again.
- Retry/idempotency: the same command ID must match its first canonical payload,
  metadata, and files. Otherwise admission returns a conflict. A matching retry
  receives the stable receipt and reattaches monitoring.
- Failure: invalid intent, stale base, missing harness, or unsafe/mismatched
  durable data transitions the command to rejected/failed. It does not silently
  rebase or start a different execution.
- Restart: `recover_commands()` runs before the service listener opens. It
  replays pre-running states from committed payload/input bytes and resumes or
  monitors prepared/running work.

### 5. Detached execution to accepted Version

[`application/src/execution_manager.rs`](../application/src/execution_manager.rs)
owns the detached runner and one advisory
[`FactoryLease`](../application/src/factory_lease.rs) for expensive machine
work. [`run_shot`](../application/src/execution_manager.rs) verifies its exact
prepared identity, claims the execution so duplicate children cannot mutate
the Shot, invokes the one implementation harness, permits at most the one
ADR-0019 targeted repair, and runs deterministic build, test, verification,
recording, install, launch, and acceptance gates through [`engine/`](../engine/).

- Persistence: `execution.json`, `events.jsonl`, `completion.json`, runner/claim
  records, harness log, and private `state-transition.json` live in the
  app-local execution directory. Accepted Shot lineage lives in the app/ledger.
- Trust boundary: untrusted harness output must pass deterministic engine and
  Apple gates before it can become an accepted Version.
- Retry/idempotency: one execution claim allows only one mutating runner.
  Completion and the landed Version are independently reconciled before the
  command journal becomes terminal.
- Failure: harness success alone is irrelevant. A failed gate records failure;
  device absence becomes `waiting_for_device` and releases the factory lease.
- Restart: prepared work starts; a live runner remains authoritative; waiting
  delivery resumes without another harness. An in-flight harness with no live
  runner is failed, not blindly rerun, because arbitrary partial source
  mutation is not a resumable checkpoint.

## Receipt and acknowledgement order

The phone deletes an outbound command only after receiving and verifying the
Mac-signed command receipt. The Mac publishes that receipt only after the
application boundary has durably admitted the command (and, for accepted
evolution, prepared the stable execution). It then persists its mailbox cursor
before acknowledging the relay. This order is why the phone may disappear
after the initial local persist without being the process that keeps the build
alive.

## Failure walk

When an evolution appears stuck, walk these artifacts in order:

| Boundary | What to inspect | Healthy evidence |
|---|---|---|
| Phone tap | Companion UI and encrypted Application Support state | request shows queued/reconnecting or later receives a Mac receipt |
| Phone -> relay | relay health/capacity logs; do not expect plaintext IDs or intent | envelope upload returns a stable cursor or duplicate success |
| Relay mailbox | `COMPANION_RELAY_ROOT/mailboxes/<id>/metadata.json` on the relay host | increasing sender high-water/cursor; opaque envelope file present until ACK |
| Mac relay reconciliation | `tohseno service status` and `tohseno service logs` | service healthy and polling independently of Studio |
| Authentication/admission | `~/.tohseno/service/inbox/{envelopes,commands,blobs}` | durable admitted envelope and processed-command receipt |
| Command journal | `~/.tohseno/service/command-journal/<command-id>/{request,payload,status}.json` | canonical request exists; state advances from received toward running/terminal |
| Execution | `<app>/.tohseno/executions/<execution-id>/execution.json` and `events.jsonl` | stable execution ID and monotonic phases |
| Completion | `completion.json`, `state-transition.json`, accepted app lineage | landed completion agrees with accepted Version |

Private files may contain sensitive local state; inspect them locally and do
not paste them into relay or public-node logs.

## Expected interruption behavior

**The phone closes immediately.** After `requestEvolution` has returned, the
signed command and encrypted payloads are already on disk. If upload completed,
the Mac continues without the phone. If it did not, the next launch/foreground
reconciliation retries. A kill before the SDK's local persist completes is not
claimed as durable submission.

**The relay disconnects.** The phone retains and retries its outbox. The Mac's
bounded two-second reconciliation passes retry after the relay returns. Relay
retention is finite; expired envelopes require cursor reconciliation and
resealing of the same signed command. No claim is made that an unavailable or
full relay provides unbounded delivery.

**The Mac service restarts.** It recovers all nonterminal command journals
before accepting new HTTP or Companion work. Detached runners and app-local
records are reconciled by stable execution ID. A whole-Mac reboot during the
harness can turn that execution into an explicit failure; prepared work and
waiting-for-device delivery are resumable.

**The same request arrives twice.** Envelope identity/digest, persistent
admitted-envelope records, signed-command digest, command ID/payload equality,
stable execution ID, and the runner claim provide layered idempotency. An exact
retry returns the existing result. A conflicting reuse fails closed.

**The Shot Version does not match.** The Mac coordinator rejects it before
translation where possible, and `ShotApplicationService` rechecks the current
accepted base immediately before `Engine.evolve_exact`. The durable command is
rejected as stale. It is never rebased to a newer Version.

## Prove the loop is alive

On macOS with Rust, Bun, Swift, Xcode command-line tools, and Keychain access:

```sh
./scripts/test-local-companion-e2e.sh
```

This is the single core-loop smoke command. It uses an isolated real Bun relay,
a real Local Workspace Service, the shared cryptographic contract through the
Companion simulator, real durable outboxes/inboxes/journals, and the
deterministic fixture harness. It proves pairing, encrypted relay delivery,
authentication and capability admission, exact-Version feedback/evolution,
offline outbox relaunch, duplicate delivery exactly once, durable execution
acceptance, revocation, and log secrecy without paying for model inference or
touching the developer's real service state.

Focused supporting gates are:

```sh
(cd website && bun run companion-relay:test)
swift test --package-path sdk/apple/TohsenoCompanionKit
swift test --package-path companion/apple/TohsenoCompanion
./scripts/test-ontology-lifecycle.sh
./scripts/test-macos-service-lifecycle.sh
```

## Guarantees and current limits

The v0 path guarantees durable local submission after the SDK call returns,
authenticated and capability-limited Mac admission, exact-base fail-closed
semantics, layered idempotency, command recovery before new service work, and
acceptance only after deterministic gates.

It does **not** yet guarantee continuous iOS background execution, unbounded
relay retention/capacity, recovery of a harness interrupted mid-mutation by a
whole-Mac restart, or production relay/APNs/release activation merely because
source tests pass. Those are explicit limits, not responsibilities silently
moved onto the phone or relay.

The highest remaining architectural risks are:

1. **Serious — mid-harness machine restart is non-resumable.** Safety is
   fail-closed, but the owner must submit/retry after the explicit failure.
2. **Serious — the production loop depends on one relay filesystem.** It is
   persistent and crash-ordered, but still bounded and operationally
   single-site; backup, capacity, and disk health remain deployment duties.
3. **Moderate — iOS wake-up is opportunistic.** A locally persisted request is
   safe, but an offline upload may wait for foreground/APNs scheduling.
4. **Moderate — background reconciliation reports little boundary-specific
   telemetry.** Private logs are deliberately content-free, so diagnosis still
   relies on walking several durable stores rather than one correlated trace.
5. **Release gate — repository source is not deployment evidence.** The public
   installer, production relay, APNs, and immutable 0.9.0 artifacts remain
   governed by [`STATE.md`](STATE.md) and the release runbook.
