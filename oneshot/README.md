# Installer source, service lifecycle, and public pin

`oneshot/oneshot.sh` is the canonical installer source. The checked-in
`website/apps/site/public/oneshot.sh` and `install.sh` are public,
immutable-release-pinned copies. They intentionally remain pinned to the last
published release, **0.8.5**, until the ordered 0.9.9 activation is complete
under `docs/runbooks/V0_9_9_READINESS.md`.

Do not copy a 0.9.9 candidate into the public directory, change the live pin,
or declare the Companion Relay ready before every immutable 0.9.9 artifact is
published and independently verified. Repository source version and public
installer pin are deliberately different during release preparation.

## 0.9.9 installed layout

The 0.9.9 release contract extends the transactional release layout without
putting user app folders inside it:

```text
~/.tohseno/
├── bin/                         stable launchers
├── current -> releases/...      atomic release pointer
├── releases/                    immutable verified release trees
├── logs/                        bounded operational logs
├── service/                     durable journals and pairing state
└── share/                       installer-controlled released materials

~/Library/LaunchAgents/
└── com.tohseno.workspace-service.plist
```

The LaunchAgent is user-level, uses no `sudo`, has `RunAtLoad`, and invokes
only:

```text
~/.tohseno/bin/tohseno service run
```

The stable launcher resolves `~/.tohseno/current` only after validating that
it remains beneath the installer-owned release directory. The service binds
Studio only to loopback and stores its private state outside every release
tree, so an update cannot replace app data, journals, or pairing records.

## 0.9.9 installation and update transaction

After publication and pin activation, the golden flow is:

```text
download immutable manifest and target artifacts
        ↓
verify release identity and every checksum
        ↓
stage beneath ~/.tohseno/releases on the destination filesystem
        ↓
atomically publish the release and stable launchers
        ↓
install or validate the recognized LaunchAgent
        ↓
atomically switch ~/.tohseno/current
        ↓
start or restart the Local Workspace Service
        ↓
verify loopback health and exact service version
        ↓
open Studio and let the installer exit
```

For an update, the previous pointer is retained until new health succeeds. If
health or version verification fails, the installer restores the previous
pointer, restarts that service, verifies rollback health, and exits nonzero.
It never deletes app folders, command journals, Builder identity, or pairing
state.

Tests use isolated homes and an injected/fake `launchctl`; ordinary repository
tests must not load or remove the developer's real LaunchAgent.

## Uninstall boundary

Default `tohseno uninstall` stops and removes only a recognized,
installer-owned LaunchAgent and installed program/release artifacts. It:

- preserves every app folder;
- preserves Builder identity;
- preserves command journals and companion pairing records for reinstall or
  explicit export;
- refuses symlinked or unrecognized service artifacts; and
- never follows an unsafe release or app-data path.

Destructive identity or data removal is not implied by uninstall and requires
a separately named and confirmed future operation.

## Historical web-intention claim

The published 0.8.5 installer accepts `--claim TOKEN` and `--no-studio` for
ADR 0011's encrypted browser-intention handoff. A claim is shape-checked
without being printed; the verified CLI receives it on stdin rather than as a
nested process argument. A person's original pasted command may remain in
shell history, so the token stays high entropy, short-lived, and single-use.

This historical relay is not the persistent Companion Relay. Neither relay is
protocol lineage, a Shot, or an execution service.

## Activation gate

The exact build, checksum, health, installer-pin, and rollback gates are
recorded in `docs/runbooks/V0_9_9_READINESS.md`. Until their verification
evidence exists, the correct public behavior is the immutable 0.8.5
installer—not an unpublished 0.9.9 URL.
