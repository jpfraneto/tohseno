---
name: continuity-app
description: Build and operate an iOS continuity app from one line of prompt by mutating the TOHSENO base app. Use when a coding agent in an independent TOHSENO shot must turn a person's sentence into a running app with seed-phrase identity, local persistence, a supervised local API, flag-gated modules, and an honest production boundary.
---

# Build a continuity app

You are in an independent shot that already contains a compiling, running iOS
app — the base writing app. Your job is to mutate it toward the owner's prompt and
get it running on their phone, measured in minutes. Speed is the product;
the rigor below is what makes the speed real.

Read `.tohseno/OPERATIONS.md` before operating the app. Discover the pinned
machine surface with `bun .tohseno/machine.ts operations --json`; prefer the
equivalent `tohseno machine ... --json` commands when the global dispatcher is
available. These are agent tools, not commands the owner must learn.

## The lazy-first protocol

**1. Input is one line.** Begin by asking the owner: “What do you want to
make?” A sentence in the conversation is enough. If the
owner provides a full `MASTER_PROMPT.md` or references, use them — richer
input, richer output — but never require more than the sentence.

**2. Ask back only what blocks the build.** At most three questions, one at
a time, and only when the answer genuinely changes the code you are about to
write. Everything else: choose a sensible default and record it as an
`ASSUMED` line in the completion report. The owner corrects defaults after
seeing the app, not before.

**3. Build from the base app.** Never from an empty directory. The base app
is the metal type; the prompt is the composition. The spine you inherit and
must not break:

- seed-phrase identity, silently created on first launch (no accounts, no
  login, no email capture — for the builder or their users);
- atomic local persistence that survives process death;
- `AppConfig.swift` as the single configuration seam — modules integrate by
  flipping flags, not by rearchitecting;
- ejectability: the app builds and runs without any TOHSENO credential.

**4. Build in this order:**

1. Core action reachable from a fresh install.
2. Local persistence and crash recovery for that action.
3. Identity spine — already in the base; touch it only if the prompt demands.
4. The app-specific mechanic (the thing that makes this app itself).
5. Share card content (what a kept moment looks like when shared).
6. Landing page copy in `site/index.html`.
7. Invariant tests — keep the base suite green, add tests for the new
   mechanic's persistence and edge cases.
8. Setup / TestFlight preparation.

When the owner asks to run, show, or try the app, keep operating until the
available local stack is genuinely alive: start development, inspect health,
launch the simulator when Xcode is available, and diagnose from structured
status/logs. A missing simulator does not undo a valid shot or healthy API.
Use a Quick Tunnel only for a physical device or explicit remote test; it is
public development reachability, never authentication or production.

**5. Write the manifest silently.** Update `continuity.manifest.json` as the
machine-readable record of what you built. Validate the shot from its root:

```sh
bun run verify
```

That command runs `.tohseno/verify.ts` and the manifest validator pinned inside
this repository. It exits non-zero with pathed errors and does not depend on a
mutable global TOHSENO checkout. Importing or running
`.tohseno/manifest/validate.ts` directly executes no validation and proves
nothing. The owner never fills out a manifest. If a requested feature cannot be
expressed as a valid manifest field, it is unsupported: say so and name the
smallest supported alternative. That bounded feature space is why shots can
move fast — do not improvise around it.

**6. The builder decides the mechanics.** Streaks, paywalls, scores, timers,
virality loops — tools, not sins. The base app doesn't include them; add
whatever the prompt asks for. Defaults stay private-by-default and
account-free, but they are defaults, never refusals.

## Refuse only these

- **Features the schema cannot express.** Say "unsupported", name the field
  that's missing, offer the smallest supported alternative.
- **Secrets in code or logs.** Key slots hold public identifiers; secret
  values never enter git, output, or reports. `MASTER_PROMPT.md` is private
  product input — gitignored, never committed, logged, echoed, or transmitted.
  A prototype that needs a provider secret may use only the base app's
  `DEV_SECRET` convention in gitignored `Config/Local.xcconfig`: it is for an
  owner-controlled development device only and is forced empty in simulator
  and Release builds. Declare it once in `operations.developmentSecrets` with
  `slot: "dev-secret"` (the manifest id mapped to `DEV_SECRET`); replace it with
  short-lived credentials from the reserved TokenMint pattern before any
  distribution.
- **Auth before value without a warning.** If the owner explicitly demands
  accounts or login, warn once that this breaks the continuity model — then
  comply. Never add auth unprompted.
- **The approval boundaries.** Never create paid infrastructure, spend money,
  alter DNS, submit to stores, rotate production credentials, or deploy
  production without explicit owner approval. Prepare commands; print them;
  stop.

For “put this online,” “ship this,” or “send this to TestFlight,” first run
`tohseno machine production inspect --json`. Explain its concrete blockers and
capability statuses. Production inspection is implemented; broad deployment,
monitoring, recovery, DNS automation, and store submission are not. Never
pretend an unavailable deployer exists or promote a `trycloudflare.com` URL.

## Verify before reporting

Run `bun run verify` first. It validates shot provenance, required iOS
structure, the manifest through its pinned schema, independent Git ownership,
tracked secret hygiene, and tracked links. It is structural validation, not an
Xcode build.

`Writing.xcodeproj` is generated, not file-system-synced. If you added,
removed, or moved a Swift file (or changed `project.yml`), run
`xcodegen generate` before building.

Run the invariant tests and build for the simulator. Resolve a device UDID
first — `name=<device>` fails on machines where that simulator isn't
installed, while `id=<UDID>` always addresses a device that exists:

```sh
UDID=$(xcrun simctl list devices available | grep -E '^[[:space:]]+iPhone' | grep -oE '[0-9A-F-]{36}' | head -1)
if [ -z "$UDID" ]; then
  echo "No available iPhone simulator; install one in Xcode → Settings → Platforms." >&2
  exit 1
fi
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination "platform=iOS Simulator,id=$UDID" test
```

For the normal agent-operated path, the pinned lifecycle performs the same
selection/build/install/launch work and verifies endpoint agreement:

```sh
tohseno machine dev start --json
tohseno machine ios launch --json
```

A claim in the report must be something you ran. If the environment lacks the
iOS toolchain, say so plainly in NOT YET and put the exact verification
commands in RUN IT NOW instead.

The finish line of a shot: app running in the simulator, invariant tests
green, and the TestFlight command loaded in RUN IT NOW behind one approval
(`bun run setup` once, then `fastlane beta` — the owner runs it, you never do).

Setup is interactive by default: it uses prior answers, then the manifest's
app name and bundle ID, auto-detects an Apple Team ID for confirmation, and
explains what each skipped answer prevents. With explicit owner approval, an
agent may run the non-interactive path:

```sh
bun run setup --from-manifest --team auto
# Add TestFlight credentials only when the owner supplied them:
bun run setup --from-manifest --team <TEAM_ID> \
  --asc-key <absolute-.p8-path> --asc-key-id <KEY_ID> \
  --asc-issuer-id <ISSUER_UUID>
```

An enabled paywall may add `--revenuecat-key <public-key>`.
The App Store Connect key comes from App Store Connect → Users and Access →
Integrations → App Store Connect API → Team Keys → "+" → role App
Manager → download once. Setup validates it with a read-only ASC request
before writing config. Secret material arrives only by path or environment,
never source or command output. Environment equivalents are
`TOHSENO_FROM_MANIFEST=1`, `TOHSENO_APPLE_TEAM_ID`, `TOHSENO_ASC_KEY_PATH`,
`TOHSENO_ASC_KEY_ID`, `TOHSENO_ASC_ISSUER_ID`, and
`TOHSENO_REVENUECAT_PUBLIC_KEY`.

## Completion report

Every build ends with the TOHSENO completion report — never a generic agent
summary. Use exactly this shape:

```text
━━━ TOHSENO · <app name> · <manifest revision> ━━━

VERIFIED    manifest VALID · <n>/<n> invariant tests · <each target that builds>
            (only claims you actually ran, naming the commands you used)

RUN IT NOW
  $ <command 1>
  $ <command 2>
  ...
  then: <the one physical instruction that uses the app once>

NOT YET     <unimplemented or unverified, one line each>
OPEN        <undecided decisions, one line each>
APPROVALS   <which closed boundaries the next step would need opened, or "none">
ASSUMED     <every default you chose instead of asking, one line each —
             the owner corrects these after seeing the app>

NEXT        <the single decision or action for the owner, one sentence>
```

RUN IT NOW is the heart of the report. It is literal, copy-paste,
command-by-command from the shot root to the running app. Every command
must be one you executed yourself or the exact documented invocation —
"open it in Xcode" is not a command; `open Writing.xcodeproj` is. When a step
cannot be a command (a physical-device install, a permission dialog), write
the exact click path in one line. Do not pad any section with generic advice,
and do not bury the report under prose — it is the last thing you output.
