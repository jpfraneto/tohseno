# TOHSENO CLI 0.9.0

The CLI is a client and local administration surface for the same
`ShotApplicationService` used by Studio and the Companion. It does not contain
an independent creation or evolution pipeline.

## The two commands

```bash
tohseno create my-app
tohseno evolve my-app
```

With nothing else, each ensures the Local Workspace Service is healthy and
opens the one screen where an intent is written: `/create?name=my-app` for a
new app, and that app's composer for an evolution. Nothing is built until the
person presses **Create App** or **Evolve App** once.

If an exact regular `./MASTER_PROMPT.md` is present, `tohseno create` imports
its bytes into the durable pending-intention store and opens the composer
prefilled and read-only. The file's own modification time is used as the
package timestamp, so repeating the command reuses the same record instead of
accumulating duplicates. Its presence never starts a build by itself.

## Scriptable forms

```bash
tohseno create <name> --prompt "..."
tohseno create <name> --prompt-file MASTER_PROMPT.md
cat MASTER_PROMPT.md | tohseno create <name>
tohseno create <name> --prompt-file MASTER_PROMPT.md --image reference.png
tohseno create <name> --prompt-file MASTER_PROMPT.md --wait
tohseno --json create <name> --prompt-file MASTER_PROMPT.md
```

Creation resolves its exact intention in this order:

1. `--prompt`;
2. `--prompt-file`;
3. bounded UTF-8 piped standard input;
4. otherwise, the composer described above.

A non-interactive command with no intention fails without creating a partial
Shot. Repeat `--image` for up to eight exact reference images; normal size,
regular-file, symlink, path, and image-byte checks apply.

The durable receipt identifies the command and execution, plus the Shot as
soon as it is safely reserved. The detached service owns work after the CLI
returns. `--wait` waits for deterministic acceptance or failure; it does not
treat generated files or harness exit as completion.

Evolution has the same exact-intention intake and may still select exact
Feedback actions:

```bash
tohseno evolve <name> --prompt "..."
tohseno evolve <name> --prompt-file EVOLUTION_INTENT.md
cat EVOLUTION_INTENT.md | tohseno evolve <name>
tohseno evolve <name> --feedback-action <commitment>
tohseno evolve <name> --wait
```

The request binds the Shot's exact current Expression and accepted base
Version. A changed base is rejected as stale; it is never silently rebased.
Stable command IDs make a retried request one semantic operation.

Expensive local work is serialized by one advisory factory lease, so a second
command admitted while the Mac is busy waits in its durable `queued` state and
starts by itself. Nothing needs to be re-sent.

## Explicit recording capability

```bash
tohseno init <name>
tohseno record [name] --note "..."
tohseno record [name] --note-file note.md
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

Pair opens Studio's standard one-use pairing code, the same one reachable from
Studio Settings → **Add iPhone**. Revocation changes local
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
