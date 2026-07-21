---
name: continuity-app
description: Build an iOS continuity app from one line of prompt by mutating the TOHSENO base app. Use when a coding agent in a TOHSENO workspace must turn a person's sentence into a running app with seed-phrase identity, local persistence, flag-gated modules, and a prepared TestFlight path — fast, with every assumption recorded.
---

# Build a continuity app

You are in a workspace that already contains a compiling, running iOS app —
the base writing app. Your job is to mutate it toward the owner's prompt and
get it running on their phone, measured in minutes. Speed is the product;
the rigor below is what makes the speed real.

## The lazy-first protocol

**1. Input is one line.** A sentence in the conversation is enough. If the
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

**5. Write the manifest silently.** Update `continuity.manifest.json` as the
machine-readable record of what you built, and validate it from the pinned
rails checkout with `bun run validate <path-to-manifest>` — it exits non-zero
with pathed errors. That command is the only validation that counts; running
`validate.ts` directly executes nothing and proves nothing. The owner never
fills out a manifest. If a requested feature cannot be expressed as a valid manifest
field, it is unsupported: say so and name the smallest supported alternative.
That bounded feature space is why one-shots land — do not improvise around it.

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
- **Auth before value without a warning.** If the owner explicitly demands
  accounts or login, warn once that this breaks the continuity model — then
  comply. Never add auth unprompted.
- **The approval boundaries.** Never create paid infrastructure, spend money,
  alter DNS, submit to stores, rotate production credentials, or deploy
  production without explicit owner approval. Prepare commands; print them;
  stop.

## Verify before reporting

Run the invariant tests and build for the simulator. Resolve a device UDID
first — `name=<device>` fails on machines where that simulator isn't
installed, while `id=<UDID>` always addresses a device that exists:

```sh
UDID=$(xcrun simctl list devices available | grep -oE '[0-9A-F-]{36}' | head -1)
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination "platform=iOS Simulator,id=$UDID" test
```

A claim in the report must be something you ran. If the environment lacks the
iOS toolchain, say so plainly in NOT YET and put the exact verification
commands in RUN IT NOW instead.

The finish line of a one-shot: app running in the simulator, invariant tests
green, and the TestFlight command loaded in RUN IT NOW behind one approval
(`bun run setup` once, then `fastlane beta` — the owner runs it, you never do).

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
command-by-command from the workspace root to the running app. Every command
must be one you executed yourself or the exact documented invocation —
"open it in Xcode" is not a command; `open Writing.xcodeproj` is. When a step
cannot be a command (a physical-device install, a permission dialog), write
the exact click path in one line. Do not pad any section with generic advice,
and do not bury the report under prose — it is the last thing you output.
