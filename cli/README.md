# TOHSENO CLI 1.2.1

`tohseno` with no arguments is the normal entry point: it ensures the local
service is available and opens the native Tohseno app.
`tohseno doctor` is read-only and reports the machine, toolchain, signing,
privacy-minimal iPhone readiness, Companion pairing, and entitlement phase.

The CLI is a client and local administration surface for the same
`ShotApplicationService` used by Studio and the Companion. It does not contain
an independent creation or evolution pipeline.

## Ship an existing Xcode app

```bash
cd ExistingApp
tohseno init
tohseno deploy
```

`init [path]` detects one native iOS `.xcodeproj` or `.xcworkspace`, performs a
real Simulator build, and adopts the current source as a new Tohseno root. It
does not write into the selected repository, rewrite Git, or invent earlier
lineage. Repeating it preserves the same reserved random ShotID.

`deploy` creates a deterministic sanitized source archive, rejects secrets and
unsafe archive entries, classifies Xcode build behavior, and presents the exact
release and generation-0.8 Registry action on the paired Companion. The first
public release is Ship; it also requires one immutable Claim Edition selected
on Companion. Later deploys are Updates and cannot carry edition flags. It
prints a public URL only after Companion approval, Registry receipt/current
head, edition-open receipt for first Ship, source promotion, and catalog
discovery agree. The durable job resumes after process, service, or network
interruption.

For automation and dry-run review, first Ship accepts exactly one of the four
closed policy shapes through `--claim-edition`, `--max-claims`, and
`--closes-at`. Invalid combinations and every attempt to apply those flags to
an already shipped Shot fail permanently. The human Companion approval remains
authority; command-line flags do not sign or open an edition.

## Claim, install, fork, and refresh an exact release

```bash
tohseno install https://tohseno.com/s/<shot-id> --release 0x<release-digest>
tohseno fork tohseno://fork/<shot-id>?release=0x<release-digest>
```

Both commands accept only an official canonical link, an exact Tohseno deep
link, or a ShotID. The Mac independently verifies the signed active generation,
factory/Registry bytecode, Builder DeviceKey authority, exact receipt and block,
current Shot head, manifest, and content-addressed source before extraction.
Source is materialized visibly under `~/Developer/Tohseno` by default.

Green projects build automatically. A non-Green compatible project stops with
named reasons before `xcodebuild`; after reviewing the visible source, repeat
with `--approve-mac-review`. Unsupported projects never build. The recipient's
local Xcode team signs the app, and `Installed` requires the exact bundle in one
physical iPhone inventory. Repeating `install` for the same immutable release
is Refresh: it rebuilds and re-signs without AI, a catalog release, or a
Registry append.

Claim itself is a Companion action and remains unavailable until the released
client carries separate threshold-signed Claims activation evidence. Canonical
confirmation queues the existing install command for the exact encountered
release even if the Mac is offline. The CLI/service then follows the same
independent verification, visible source, Xcode signing, and physical-device
truth as a direct Install. Claim never implies that those steps succeeded.

An install-only copy has no child Shot identity. A fork reserves a new random
ShotID and retains the exact parent ShotID and release digest; if later shipped,
that parent becomes part of the signed child catalog release.

## Create and evolve

Generated creation remains the same Mac factory, and Companion requests remain
durable signed, encrypted commands routed to that Mac:

```bash
tohseno create my-app --prompt "An app that..." --wait
tohseno evolve <name> --prompt "..."
```

The request binds the Shot's exact current Expression and accepted base
Version. A changed base is rejected as stale; it is never silently rebased.
Stable command IDs make a retried request one semantic operation.

Expensive local work is serialized by one advisory factory lease, so a second
command admitted while the Mac is busy waits in its durable `queued` state and
starts by itself. Nothing needs to be re-sent.

Create and evolve share the same bounded executor: one implementation harness,
at most one concrete code/build repair, and one shared 60-minute harness
budget. A repair never resets the clock. Missing device, signing, provisioning,
network, and protocol conditions do not invoke intelligence. The resulting
private State Transition Receipt is available under execution Details.

## Historical recording compatibility

```bash
tohseno recording init <name>
tohseno recording record [name] --note "..."
```

These commands preserve ADR 0014's recording-layer bytes and safety rules.
They do not run the factory. A `.tohseno/recording-layer-v1` folder remains
`recording_only` and is never silently migrated into a factory Shot.

## Local Workspace Service

```bash
tohseno service install
tohseno service start
tohseno service stop
tohseno service restart
tohseno service status
tohseno service logs
tohseno service uninstall
```

`tohseno service run` is the internal foreground command invoked by launchd.
`tohseno studio` verifies service health, opens the verified loopback origin,
and returns. A hidden foreground-port option exists only for isolated
development and integration tests.

An installed user LaunchAgent is
`~/Library/LaunchAgents/com.tohseno.workspace-service.plist` and executes the
stable installer-controlled `~/.tohseno/bin/tohseno service run` launcher. No
operation requires `sudo`.

## Companion administration

```bash
tohseno companion status
tohseno companion pair
tohseno companion devices
tohseno companion revoke <device-id>
tohseno companion relay-status
tohseno companion simulate ...
tohseno companion sdk vendor --into <shot-path>
```

Fresh Mac-to-iPhone pairing is driven by the cable-genesis surface. The service
uses CoreDevice's supported URL payload to deliver one signed, expiring
invitation only after Companion installation. Revocation changes local
admission immediately. The simulator uses the private companion schemas and
durable command journal rather than a test-only factory path. SDK vendoring
copies the exact released Swift source, license, shared vectors, and integrity
manifest into the destination; the generated app never resolves SDK code from
a mutable `~/.tohseno/current` path.

## Structured output

Place `--json` before the subcommand. Supported service, creation, evolution,
pairing, device, command-acknowledgement, and execution-status operations emit
one stable JSON object on stdout. Diagnostics and progress go to stderr;
scripts should continue to honor nonzero exit status.

```bash
receipt="$(tohseno --json create fixture --prompt-file intention.md)"
command_id="$(printf '%s' "$receipt" | jq -r .command_id)"
```

Never parse human progress rendering as an API.
