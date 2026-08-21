# TOHSENO 0.9.0

Describe an app. TOHSENO makes it and puts it on your iPhone.

```bash
tohseno create my-app
```

Describe what should change. TOHSENO evolves it and puts the new version on
your iPhone.

```bash
tohseno evolve my-app
```

Each command opens one screen with one box on it. Write the intent, optionally
attach images, press the one button, and wait. When the app is ready TOHSENO
installs it on your iPhone by itself — or, if the phone is not plugged in, says
so and installs it automatically the moment it is:

```text
Your app is ready.

Plug your iPhone into this Mac
and I’ll install it automatically.
```

There is no button to press there. TOHSENO orchestrates itself.

## What it actually is

A persistent private app factory on your Mac. One Local Workspace Service owns
factory commands, executions, Studio, and synchronization with a paired iPhone.
The Mac remains the backend: prompts, source, coding harnesses, Xcode, signing,
installation, and acceptance stay local. Completion means the build, test,
verification, delivery, and acceptance gates passed — never that a harness
exited successfully.

That machinery is sophisticated and it is entirely beneath the floor. The
product is App → Intent → App on your iPhone ([ADR 0016](docs/adr/0016-app-intent-app-on-your-iphone.md)).

## Scriptable forms

The human defaults are simple; nothing was taken away from automation.

```bash
tohseno create my-app --prompt "..."
tohseno create my-app --prompt-file MASTER_PROMPT.md --wait
cat MASTER_PROMPT.md | tohseno create my-app
tohseno evolve my-app --prompt "Make the first-run experience clearer" --wait
tohseno --json create my-app --prompt-file MASTER_PROMPT.md
tohseno studio
```

An evolution binds the app's exact current Expression and accepted Version at
submission; a stale request is refused rather than rebased. Every route uses
the same durable application service as Studio and the iPhone, so work survives
the invoking Terminal, a closed browser, and a service restart.

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

## The iPhone

```text
Your Apps  →  choose an app  →  What should change?  →  Evolve App
```

One tap. No confirmation, no version picker, no separate feedback step. If your
Mac is asleep the phone says `Waiting for your Mac…`, you can close the app, and
the request delivers itself later without another tap.

The phone is a remote control for intent, not a mobile TOHSENO: it receives
encrypted workspace summaries and privacy-safe state, never source code or
harness output, under an explicit revocable capability grant. The shared relay
is a content-blind encrypted mailbox that cannot decrypt commands, interpret
prompts, build apps, run agents, or authorize actions. Private companion
records never enter the public `tohseno-node`.

- [The Companion app](companion/apple/TohsenoCompanion/README.md)
- [Companion SDK and conformance fixture](sdk/apple/TohsenoCompanionKit/README.md)

## Administration

Available when you want it; never in the way when you don't.

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
