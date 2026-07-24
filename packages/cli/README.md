# `@tohseno/cli`

The TOHSENO executable is the terminal door, local Studio, and deterministic
factory for independent native iOS shots.

```sh
tohseno
tohseno studio
```

Both doors normalize private input, use the selected coding agent for a strict
sanitized plan, resolve the same released kernel/template/skill catalog,
compose and lock the repository, run the coding agent, and verify the result.
An unavailable or invalid planner produces the Blank fallback without
switching providers.

Automation uses cwd-independent commands:

```sh
tohseno create my-app --no-launch --no-interactive
tohseno create --file intention.md --reference sketch.png
tohseno my-app
tohseno verify my-app
tohseno run my-app
```

The ordinary handoff never requires the owner to install Bun or know a
repository-relative command. Low-level ejected machine operations remain
embedded in each shot and are authenticated against that shot’s pinned factory
release.

Generic shots use `app.manifest.json`, `tohseno.skills.json`,
`tohseno.skills.lock`, `SHOT.md`, and `DONE.md`. Metadata-v1 continuity shots
retain their historical manifest and runtime; the CLI dispatches by metadata
and never silently migrates them.

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
[`docs/CLI.md`](../../docs/CLI.md) for the complete command and compatibility
reference.
