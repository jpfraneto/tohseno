# ADR 0038: npm installs the CLI and init teaches the Ship path

Status: accepted

Date: 2026-09-02

This decision adds a builder-focused npm door to ADR 0034's person-to-person
software network. It supersedes ADR 0021's automatic postinstall launch and
ADR 0025 only where that decision described npm as a retained legacy path.
`Tohseno.app` remains the native Mac product; npm is the direct CLI product for
people who already have an Xcode project to Ship.

It changes no frozen protocol encoding, generation-0.8 ABI, Claims ABI,
DeviceKey authority, Claim, Ship, Update, signing, installation, or release
truth.

## Context

The published npm package currently installs a JavaScript bootstrap whose
postinstall hook immediately downloads a native release, starts the Local
Workspace Service, and opens Studio. That is not a CLI-only install and it
places setup before the builder's highest-value action.

The person-to-person path is simpler: enter an existing Xcode project, connect
it once with `tohseno init`, and explicitly publish with `tohseno deploy`.
Those commands must teach their own meaning. A help page or GUI shown before
the person reaches the project folder is not a substitute for guidance at the
moment of action.

## Decision

The builder installation command is:

```text
npm install --global tohseno
```

npm installs only the dependency-free command launcher. Installation performs
no lifecycle download, service mutation, Companion setup, or GUI launch. A
no-argument `tohseno` invocation prints the three-command path:

```text
cd /path/to/YourApp
tohseno init
tohseno deploy
```

The launcher downloads the command runtime only when a real command requires
it. It accepts only the fixed HTTPS CLI manifest, exact architecture, version,
byte length, SHA-256, closed release layout and checksums, and the declared
Apple Developer ID requirement. It activates the verified runtime in the
existing user-owned `~/.tohseno` layout without `sudo`.

In an interactive Terminal, `tohseno init [path]` presents one short fact at a
time and pauses after every fact with exactly:

```text
Press Enter for next step
```

The walkthrough explains the Xcode Simulator check, non-destructive adoption,
stable candidate ShotID, the subsequent `tohseno deploy`, Companion authority,
and exactly one Ship followed by Updates. Only then does the existing adoption
operation run. Structured `--json` use and non-interactive stdin/stdout never
pause, so automation remains composable.

Before Xcode adoption begins, `init` asks the existing local genesis service to
identify the private intended iPhone and read that device's installed-app list
through CoreDevice's file-based JSON interface. The exact
`com.tohseno.companion` bundle must be present, the intended-device digest must
bind that phone, and the specific one-use private pairing session must have
completed. A remembered local install state, a successful `devicectl install`
exit code, another reachable phone, or an unrelated prior Companion pairing is
not sufficient. If the exact bundle is absent, `init` stops before touching the
Xcode project and tells the person to run `tohseno companion install`; that
command uses the one existing genesis path to build, sign, install,
inventory-verify, launch, and privately pair Companion before `init` is retried.
An unreadable inventory remains unknown and asks the person to keep the phone
reachable and unlocked; it is never reported as a missing or installed app.

`init` still succeeds only after the real Xcode project is detected and built.
`deploy` still snapshots and checks the real source, requires the real paired
Companion to approve the exact action, and prints a public route only after the
Registry, source, catalog, and applicable Claim Edition evidence agree.

## Release boundary

The npm version may become public only after its fixed CLI manifest and both
architecture-specific signed runtime archives are public and independently
round-trip verified. Publishing the npm launcher cannot activate a missing or
mismatched runtime because installation fails closed at first operational use.

The CLI artifact channel is separate from stable DMG promotion. It does not
claim that `Tohseno.app` stable acceptance, another human's Claim, recipient
Apple signing, or physical iPhone installation occurred. It does not authorize
a new contract generation, contract deployment, generic relayer, fabricated
receipt, or bypass of owner-attended Companion and Apple boundaries.

## Consequences

People who want only the CLI can install exactly that through npm and see the
real next action immediately. The largest explanation sits at `init`, where it
has project context, and advances at the person's pace. Existing native-app
users, structured automation, and the one service/factory implementation keep
their current paths.
