# `@tohseno/cli`

The TOHSENO executable is an agent-first launcher and deterministic factory for
independent iOS **shots**.

Take another one.

The human entry is:

```sh
tohseno
```

It asks whether to take a new shot or continue one, shows only the implemented
iOS platform, selects an installed Codex or Claude Code, enters the independent
shot, and launches that agent with one constant instruction. Product intention
stays in the conversation, never in CLI arguments or factory metadata.

Coding agents use the machine namespace:

```sh
tohseno machine operations --json
tohseno machine dev start --json
tohseno machine ios launch --json
tohseno machine verify --json
tohseno machine production inspect --json
```

These dispatch to `.tohseno/machine.ts` inside the selected shot. New shots
pin their runtime, manifest validator, verifier, instructions, playbook, and
factory provenance, so a later global CLI upgrade cannot silently change their
critical behavior.

The compatibility commands `create`, `list`, `open`, `doctor`, `verify`, and
`adopt` remain implemented for automation. Existing Phase 1 shots are never
silently rewritten and retain legacy pinned verification.

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
