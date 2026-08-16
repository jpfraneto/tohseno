# TOHSENO 0.9.0

TOHSENO is a persistent private app factory on your Mac. One Local Workspace
Service owns factory commands, executions, Studio, and synchronization with a
paired iPhone Companion. The Mac remains the backend: prompts, source, coding
harnesses, Xcode, signing, installation, and acceptance stay local.

```bash
tohseno create my-app --prompt-file MASTER_PROMPT.md --wait
tohseno evolve my-app --prompt "Make the first-run experience clearer" --wait
tohseno studio
```

`create` begins an intention-led Shot birth. `evolve` binds a new intention to
the Shot's exact current Expression and accepted Version; a stale request is
rejected rather than rebased. Both routes use the same durable application
service as Studio and the Companion, so work survives the invoking Terminal.

With no supplied intention, an interactive `tohseno create my-app` opens
Studio at `/create?name=my-app`. Studio contains the intention editor,
reference-image intake, live execution state, and **CONNECT IPHONE** pairing
surface. It is served only on loopback by the persistent service.

## Recording an ordinary app folder

ADR 0014's byte-compatible recording layer remains explicit:

```bash
tohseno init my-app
# edit with any tools
tohseno record my-app --note "Describe these exact files"
```

The visible folder stays ordinary and ejectable. Existing
`.tohseno/recording-layer-v1` folders remain `recording_only`; TOHSENO never
silently turns them into factory Shots or rewrites their accepted records.

## Private Companion channel

A paired Companion receives encrypted workspace summaries and privacy-safe
execution events. It can submit exact-Version feedback, private marketing
notes, exact-base evolutions, and new-Shot intentions according to an explicit
revocable capability grant. It never receives source code or harness output.

The shared Companion Relay is a content-blind encrypted mailbox. It cannot
decrypt commands, interpret prompts, build apps, run agents, or authorize Shot
actions. Private companion records never enter the public `tohseno-node`.

The native integration package and conformance fixture are in
[`sdk/apple/TohsenoCompanionKit`](sdk/apple/TohsenoCompanionKit/README.md).

## Administration

```bash
tohseno service status
tohseno service restart
tohseno service logs
tohseno companion pair
tohseno companion devices
```

The intended installed layout uses a user LaunchAgent and the stable
`~/.tohseno/bin/tohseno` launcher. It requires no `sudo`. See the
[CLI contract](cli/README.md), [Studio guide](studio/README.md), and
[installer boundary](oneshot/README.md).

## Release status and authority

The repository source targets 0.9.0. The public one-line installer remains
pinned to immutable 0.8.5 until 0.9.0 artifacts are published and independently
verified by an authorized owner; no source checkout is installed on user Macs.
See [current state](docs/STATE.md) and the
[0.9.0 release runbook](release/V0_9_0_OPERATOR_RUNBOOK.md).

`protocol/` remains normative over prose. Historical protocol bytes,
Builder identities, signatures, and public-node validation remain unchanged.

- [Architecture decisions](docs/adr/README.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Privacy boundary](docs/PRIVACY.md)
- [Protocol specification](protocol/SPECIFICATION.md)
- [Protocol conformance](protocol/CONFORMANCE.md)
- [Frozen history](history/README.md)
