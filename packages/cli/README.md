# `@tohseno/cli`

The TOHSENO executable is the terminal door, local Studio, and deterministic
factory for independent native iOS Shots.

```sh
bun run tohseno
bun run tohseno -- studio
```

Both doors normalize private input, use the selected coding agent for a strict
sanitized plan, resolve the same released kernel/template/skill catalog,
compose and lock the repository, run the coding agent, and verify the result.
An unavailable or invalid planner produces the Blank fallback without
switching providers.

Source-checkout automation uses:

```sh
bun run tohseno -- create my-app --no-launch --no-interactive
bun run tohseno -- create --file intention.md --reference sketch.png
bun run tohseno -- my-app
bun run tohseno -- verify my-app
bun run tohseno -- run my-app
```

The prepared, unpublished managed artifact provides the cwd-independent
`tohseno` wrapper and its pinned runtime. Low-level ejected machine operations
remain embedded in each Shot and are authenticated against that Shot’s pinned
factory release.

Shots use `app.manifest.json`, `tohseno.skills.json`,
`tohseno.skills.lock`, `SHOT.md`, and `DONE.md`.

Studio binds to loopback, uses a private path-scoped browser session, and
serializes heavy Studio actions. It reads the same `/shots` filesystem as the
CLI and is not a runtime dependency of a completed app.

From this repository:

```sh
bun run tohseno --
bun test packages/cli/tests
bun run typecheck
bun run tohseno:release
```

Publishing a release is an external owner-approved action. See
[`docs/CLI.md`](../../docs/CLI.md) for the complete command reference.
