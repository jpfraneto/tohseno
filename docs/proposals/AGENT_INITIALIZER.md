# Local agent initializer: Phase 1 implementation record

> Historical record. Phase 1 remains implemented and backward compatible, but
> the current human entrance is the Phase 2 no-argument launcher documented in
> [CLI and machine operations](../CLI.md). Phase 2 adds the managed installer,
> pinned machine runtime, local API/SQLite foundation, Quick Tunnel transport,
> iOS endpoint separation, and production inspection without rewriting the
> work recorded here.

- Status: **Implemented** for the local `tohseno` CLI, iOS shot creation,
  Codex/Claude Code adapters, immutable cached releases, and local linking.
- Status: **Prepared** for later package-registry distribution; package metadata
  exists, but no package was published.
- Status: **Implemented** as a thin pinned compatibility entry point. The old
  `oneshot.sh` creator remains retired; the delegator follows the repository's
  two-commit release rule and verifies the released installer before running
  it.
- **Implemented:** the checksum-pinned managed installer, the published 0.3.1
  CLI source artifact, and the stateless public site.
- **Proposed:** future platform adapters and production deployment cells.
- **Open:** release-signing and external-publication ownership.
- Does not implement: Android, web, backend deployment, secrets management,
  TokenMint, SessionLink, payments, QR authentication, or store submission.

This document began as the initializer proposal. It now records which parts
became the first reusable local factory and which remain future work.

## Implemented experience

A person installs the toolchain once from this checkout and repeatedly creates
independent repositories called shots:

```sh
bun install
bun run tohseno:link
tohseno doctor
tohseno create the-trenches
```

The create flow asks only the two factory choices that affect the immediate
operation: the currently supported platform and the installed coding agent.
The shots directory is configured once or overridden by a flag. The coding
agent, not the initializer, then asks what the owner wants to build.

The CLI never asks for a product prompt or accepts one as an option. It launches
Codex or Claude Code inside the completed shot with the fixed instruction:

```text
Read the local AGENTS.md and begin.
```

This preserves the owner's private idea outside shell history, process
arguments, factory cache, diagnostics, and provenance.

## Factory and shot boundary

The factory prepares a minimal immutable release containing the iOS base,
manifest tooling, coding-agent protocol, shot instructions and verifier,
license, and a SHA-256 inventory. Git provenance includes an explicit dirty
marker, so modified source is never labeled as its clean commit. A verified
active cache can create shots offline even when the source checkout is absent.

Each shot receives copies of everything it needs. It has no symlink or runtime
reference to the TOHSENO installation/cache and begins as its own Git repository
with a baseline commit. `.tohseno/shot.json` pins factory, CLI, template, and
manifest versions without storing the product idea.

The baseline commit always uses the neutral command-scoped author
`TOHSENO Factory <factory@tohseno.local>`. It neither copies the owner's
configured email into generated history nor writes any Git configuration.

Creation is atomic: the CLI assembles, validates, commits, and verifies in a
private sibling directory before one rename publishes the destination. Failures
remove staging and preserve content-free diagnostics. A selected agent's later
nonzero exit does not destroy the already valid shot.

## Implemented agent adapter contract

The Codex and Claude Code adapters provide:

- executable detection on `PATH`;
- an explicit user choice when both are present;
- automatic use only when exactly one supported agent is installed;
- a deterministic non-interactive selection contract;
- launch with inherited terminal I/O and the shot as working directory;
- the same local AGENTS, skill, manifest, privacy, approval, and ejection rails.

TOHSENO adds no telemetry or prompt upload. That statement does not make either
provider local or zero-retention; provider data handling remains a separate
disclosure boundary.

## Command surface

Phase 1 implements:

- `create`: atomic iOS creation and optional agent launch;
- `list`: filesystem discovery, with no fragile central shot registry;
- `open`: a shell-safe absolute-path printer;
- `doctor`: required factory checks and optional agent/iOS warnings;
- `verify`: dispatch to the verifier pinned inside a shot;
- `adopt`: confirmed, in-place installation of only `.tohseno` provenance and
  validation tools into a compatible independent iOS repository.

The exact flags and failure behavior are in [CLI and factory
reference](../CLI.md).

## Web-bootstrap reconciliation

The original entry command was:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

Its two-stage trust model remains sound: an inspectable script, a full pinned
commit, a final `main` invocation so truncation executes nothing, and
`must-revalidate` delivery. Its old implementation, however, copied templates
and initialized Git independently of the CLI. Keeping both would create two
sources of truth.

Because the current pin predates the CLI, the served script now makes no
changes, prints the checkout/link/create migration, and exits nonzero. A future
follow-up release may turn it into a thin pinned CLI installer only after its
pin contains all required CLI assets. Shot creation remains exclusively in
`packages/cli`.

## Privacy and external-action boundary

- `MASTER_PROMPT.md`, credentials, capabilities, private content, contact
  details, and production data never enter cache, metadata, logs, errors, or
  generated Git history.
- Factory bundles reject known credential paths, `.git`, dependencies, and
  symlinks.
- The initializer creates no account and operates no generated-app backend.
- It does not create paid resources, change DNS, rotate production credentials,
  deploy, submit to a store, or publish packages.
- The fastlane `beta` lane remains Prepared and owner-executed.

## Adoption limit

Adoption is recognition, not migration. The compatible project must already be
an independent Git root with the Phase 1 iOS base structure and a valid
manifest. After explicit confirmation, only `.tohseno/` is added atomically; no
app file, AGENTS file, skill, package script, staging area, or history is
rewritten. Projects outside the configured shots directory remain outside
`list`, by design.

## Future platform adapter gate

Android or web becomes a real option only after it has a reusable working base,
minimal release mapping, required-file checks, customization, pinned local
verification, doctor integration, atomic creation tests, and truthful docs.
Phase 1 exposes only iOS rather than presenting placeholder choices.

## Evidence required at release

- clean and dirty release identifiers plus checksum corruption negatives;
- concurrent immutable-cache creation and source-free active-cache reuse;
- interactive, one-agent, multi-agent, no-agent, and non-interactive selection;
- atomic cleanup, existing-destination refusal, and failed-agent preservation;
- generated manifest validity, independent Git baseline, no external symlinks,
  no copied secrets, and paths containing spaces;
- list, open, doctor, pinned verify, and confirmed/cancelled adoption;
- the repository `bun run check` gate and an iOS simulator test when available;
- package publication and production deployment only as separately approved
  external actions.
