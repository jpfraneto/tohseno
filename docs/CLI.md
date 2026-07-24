# CLI and machine operations

## Human flow

Run:

```sh
tohseno
```

On first use TOHSENO explains where repositories are created, what stays
private, which coding-agent trust boundary applies, and that Apple tools are
needed to run iOS. With an empty contact sheet it asks directly for one
intention. With existing shots it offers Take another or Continue.

The proposed plan shows app name, slug, bundle ID, template, ordered skills,
data strategy, identity strategy, and the first definition of done. Enter
accepts it; Edit changes composition; Blank selects the safe starting template;
Cancel creates nothing.

The final handoff reports evidence for agent, repository, source/lock, skills,
native build, Simulator, and capture. It gives exactly one next action and
cwd-independent later commands.

## Commands

```sh
tohseno <shot>
tohseno continue <shot>
tohseno create <slug> [--agent codex|claude] [--no-launch]
tohseno create --file <intention.md> [--reference <image> ...]
tohseno list
tohseno open <shot>
tohseno verify <shot>
tohseno run <shot>
tohseno preview <shot>
tohseno studio
tohseno doctor
```

`--platform ios` remains accepted for compatibility; iOS is assigned
automatically. `--no-interactive` never prompts and requires an explicit agent
only when a coding agent will launch.

`create --file` and Studio store normalized raw input in gitignored private
provenance. They never print it. The tracked plan is sanitized.

## Planner behavior

The planner uses only the selected installed agent. It runs in a private
temporary directory containing the intention and a bounded catalog summary,
with no write or network authority requested. Output must be exactly one JSON
object with known fields and installed IDs. Timeouts, provider failures, and
invalid output return the Blank fallback; they do not try another provider.

## Studio

```sh
tohseno studio [--port <port>] [--no-open] [--shots-dir <path>]
```

Studio binds to `127.0.0.1`, establishes a private path-scoped browser session,
and streams planning, composition, agent, verification, build, Simulator, and
capture progress. It uses the same factory and catalog as the CLI. One heavy
Studio operation runs at a time.

## Automation and ejection

```sh
tohseno machine operations --json [--shot <path-or-slug>]
tohseno machine ios inspect --json [--shot <path-or-slug>]
tohseno machine ios launch --json [--shot <path-or-slug>]
tohseno machine verify --json [--shot <path-or-slug>]
```

Legacy metadata-v1 shots additionally expose their pinned `dev`,
`production`, and optional `token` operations. Those are compatibility
surfaces, not generic app requirements.

Direct `bun .tohseno/machine.ts ...` is the advanced ejected interface. Normal
owner instructions use global `tohseno` commands so they remain independent of
the current directory and internal runtime.

## External authority

No command deploys, publishes, spends money, alters DNS, submits to an app
store, creates an account, or performs an irreversible action without the
specific owner approval required by that operation.
