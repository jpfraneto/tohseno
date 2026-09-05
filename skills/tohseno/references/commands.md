# Existing command surface

These commands were checked against the installed 1.2.1 runtime and repository
source on 2026-09-04. Check installed help before use. Examples use placeholders,
not defaults to execute literally. Pass prompt text through a UTF-8 file or
properly quoted argument; do not interpolate conversation text as shell code.

## Connect source you are working on

Run from the app's actual source directory:

```sh
tohseno --json status
tohseno --json init /absolute/path/to/App.xcodeproj
```

`init` adopts the project, inspects it and runs a real Simulator build. It can
require the intended iPhone's Companion inventory and completed pairing first.
Use `--scheme` only after identifying the intended app target. It does not
install the adopted app on the phone or publish it.

## Existing factory generation and evolution

```sh
tohseno --json create --prompt-file /absolute/path/to/intention.txt
tohseno --json evolve existing-factory-app --prompt-file /absolute/path/to/change.txt
```

Both launch the configured implementation harness; they do not reuse this
conversation as that harness. Both accept up to eight `--image` arguments and
optional `--wait`. Retain the returned command/execution identity and observe
the admitted work instead of submitting repeatedly after a timeout. Announce
long builds and keep status useful.

The current `evolve` command resolves **factory Shots only**. Do not pass an
adopted project such as Anky and imply it is supported. Adopted-project
evolution exists in the Mac/Companion path, but this CLI does not expose it.

## Current direct-agent delivery gap

The inspected runtime does not expose a standalone CLI operation to build,
sign, and install an externally edited adopted local project through Tohseno's
existing delivery machinery without a second coding harness invocation.

Direct source editing and `init`/publication are available. Fully integrated
private delivery after those edits still needs a supported entrypoint. Inspect
newer installed help in case one has been added. Otherwise report this precise
limitation and the completed source/build state. Do not silently launch another
agent, send a no-op evolution, publish private source, call undocumented mutation
endpoints with extracted credentials, or fabricate factory history to bridge it.

## Discovery and current network availability

Public read-only endpoints:

- `https://tohseno.com/api/registry/v1/status`
- `https://tohseno.com/api/registry/v1/shots?q=URL_ENCODED_QUERY`
- `https://tohseno.com/api/registry/v1/timeline`

Use returned canonical links and exact release identities. The website's
`https://tohseno.com/registry` is also a discovery surface. A successful status
response does not mean publication is enabled: inspect `relayer.available`.
Claims has separate activation. Never infer it from Registry availability.

## Publish only when requested

From the connected app's source directory:

```sh
tohseno --json deploy --dry-run
tohseno --json deploy
```

Dry-run prepares and inspects a snapshot without approval or publication; it
can create local packaging artifacts. Inspect its findings before proceeding.
Actual deployment requires exact Companion approval and live network gates.
Let the person choose any required first-Ship Claim Edition policy; its terms
are immutable. `--screenshot` explicitly selects public media; private reference
images must not become screenshots automatically. Return a public URL only when
the operation verifies publication. Preserve the durable job on interruption.

## Receive a public release

```sh
tohseno --json install CANONICAL_SHOT_LINK --release EXACT_RELEASE_DIGEST
tohseno --json fork CANONICAL_SHOT_LINK --release EXACT_RELEASE_DIGEST --into NEW_DIRECTORY
```

Check live eligibility and the installed command's requirements. An available
CLI flag is not evidence that Claim activation or receipt requirements pass.
Never bypass a required Claim. Keep source verification and recipient Apple
signing in the supported path. Do not blindly pass `--approve-mac-review`:
inspect the named executable-build risks and obtain any required authorization.
Fork creates a new Shot; use it only when a new derivative is intended.
