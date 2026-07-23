# `@tohseno/cli`

The TOHSENO executable is an agent-first launcher, local Studio, and
deterministic factory for independent iOS **shots**.

Take another one.

The human entry is:

```sh
tohseno
```

It asks whether to take a new shot or continue one, shows only the implemented
iOS platform, selects an installed Codex or Claude Code, enters the independent
shot, and launches that agent with one constant instruction. Product intention
for this interactive door stays in the conversation.

Two other implemented doors call the same application-level creation engine:

```sh
tohseno create --file intention.md --reference sketch.png
tohseno studio [--port 4747] [--no-open] [--shots-dir <path>]
```

`create --file` accepts one UTF-8 Markdown intention and repeatable image
references. Studio accepts a typed intention, one optional Markdown file, and
up to eight image references. Typed text comes first when both text and
Markdown are supplied. With no explicit name or slug, the shared
concurrency-safe allocator assigns the next `shot-NNN`. Both doors apply the
same pinned release, atomic publication, independent Git baseline, verification,
progress journal, and provenance format under the configured shots directory.

Studio binds only to `127.0.0.1` (port `4747` by default), opens the browser
unless `--no-open` is passed, and treats `/shots` as its source of truth. It
permits one heavy create/run/preview/verify operation at a time and observes
shots made by a separate CLI process. Closing Studio does not affect CLI
operation or ownership of a completed shot. The Studio server does not upload
shot input; the selected coding agent uses it under that agent's own provider
and privacy settings.

Simulator doors share one runner:

```sh
tohseno run <shot>
tohseno preview <shot>
tohseno doctor
```

`run` starts the shot’s pinned development runtime, builds, installs, and
launches the app in a real Apple Simulator, then attempts a gitignored
screenshot without making capture failure fatal.
`preview` adds a local interactive browser stream and remains in the foreground
until `Ctrl-C`; it is not an iOS emulator.
The stream requires macOS on Apple Silicon, a native arm64 Node.js 20 or newer, Xcode tools, an
available iPhone Simulator, and the exact pinned
[`serve-sim` 0.1.45](https://github.com/EvanBacon/serve-sim). A paid Apple
Developer Program membership is not required for Simulator use. `doctor`
reports each requirement as ready or as a warning. An explicit unsupported
`preview` returns an actionable error; Studio’s contact sheet, creation, and
non-preview CLI commands remain usable.

Explicit creation inputs are normalized into
`.tohseno/provenance/intention.md`, `provenance.json`, `events.jsonl`, and
internally named reference copies. Hashes and original reference filenames are
recorded. This directory and `.tohseno/artifacts/` are private and gitignored;
the tracked shot metadata carries only the content-free creation summary and
input digest.

Coding agents use the machine namespace:

```sh
tohseno machine operations --json
tohseno machine dev start --json
tohseno machine ios launch --json
tohseno machine verify --json
tohseno machine production inspect --json
```

Global commands authenticate the selected shot's embedded release inventory
and dispatch through a private read-only snapshot of its pinned machine.
Direct `bun .tohseno/machine.ts ...` remains the independently ejectable local
door. New shots pin their runtime, manifest validator, verifier, instructions,
playbook, and factory provenance, so a later global CLI upgrade cannot silently
change their critical behavior.

The compatibility commands `create`, `list`, `open`, `doctor`, `verify`, and
`adopt` remain implemented for automation. `studio`, `run`, and `preview` are
application-level adapters over the same factory and Simulator services.
Existing Phase 1 shots are never silently rewritten and retain legacy pinned
verification.

From the repository:

```sh
bun run tohseno --
bun test packages/cli/tests
bun run --cwd packages/cli typecheck
bun run tohseno:release
```

`release:build` creates a deterministic source distribution for the managed
installer. Publishing it is an external owner-approved action. The complete
launcher, machine protocol, config, installer, and compatibility reference is
in [`docs/CLI.md`](../../docs/CLI.md).
