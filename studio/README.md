# TOHSENO Studio 1.0.0

Studio is the loopback-only browser window into the persistent Local Workspace
Service. It is not a separate backend: every creation and evolution passes
through the same Rust application service used by the CLI and paired
Companions.

```bash
tohseno studio
```

The command ensures the installed service is healthy, opens its verified
`127.0.0.1` origin, and returns. Studio remains available after the Terminal
closes. The service does not bind `0.0.0.0`, and a phone never connects to this
loopback HTTP API.

## The product law

Studio exists to serve exactly one sentence:

**App → Intent → App on your iPhone.**

There is no dashboard. There is no factory-control surface. **Create App** is
the one branded creation action; Shot identities, Expressions, Versions,
Executions, inference routes, Feedback records, marketing notes,
lineage, and pairing internals remain behind the normal path.

## Four views, one workspace

The normal desktop surface keeps a compact icon rail on the left. Each app is
one icon with its short name underneath, like an iPhone home screen, plus a
status-colored dot. Selecting an
app places its one intent composer or one human state in the middle and its
latest accepted first-screen capture in an iPhone frame on the right. When no
capture exists, the frame says so and shows the app icon; it is a preview, not
a fake interactive simulator. Narrow windows collapse these regions without
changing the route or creating a dashboard.

At desktop widths the shell is exactly one viewport tall. The app rail,
composer/state surface, and preview each own their scrolling, so a long app
library never pushes either adjacent column off screen.

```text
/                 Your Apps          compact app rail, plus Create App
/create           the composer       optional name · intent · images · harness/model · Create App
/shots/{id}       one app            the same composer, or its one human state
/settings         Settings           revoke and diagnostics
```

`tohseno create` with no name or intention opens `/create`; the intent box is
primary and the model names the app from its purpose. `tohseno create paper`
still opens `/create?name=paper` with the explicit name already filled in.
`tohseno evolve paper` opens that app's composer asking *What should change?*.
Both then follow the same short lifecycle.

If a `./MASTER_PROMPT.md` is present, the CLI imports its exact bytes through
the local pending-intention store and opens `/create?pending=…` unless an
explicit name was supplied.
The composer is prefilled and read-only, and nothing is built until the person
presses **Create App** once. TOHSENO never starts a build merely because a file
exists.

Images use the same safe reference machinery as before: up to eight validated
reference images, 64 MB each and 160 MB combined, submitted as exact bytes with
their canonical base64url encoding.

Creation also shows one compact **Build with** dropdown. It contains only
installed, currently usable coding harnesses and each harness's associated
models. The configured pair is selected by default; the service validates and
durably records the resolved choice before the one bounded implementation run.
Routes, credentials, command arguments, and raw output remain private.

## One human state

The service publishes a `presentation` for every app, and Studio renders it
verbatim. The browser does not interpret internal phases:

```text
waiting          Waiting to build…
building         Building your app…
ready_for_phone  Your app is ready.  Plug your iPhone into this Mac and I’ll
                 install it automatically.
installing       Installing on your iPhone…
installed        paper is on your iPhone ✓
failed           Couldn’t build your app.   [Retry]  [Show live details]
```

`ready_for_phone` deliberately has no Install, Resume, or Continue button. The
persistent service resumes delivery by itself when the configured iPhone
becomes available, and holds no expensive local resource while it waits.

`Show live details` is the pressure-release valve: a live, privacy-safe feed
from the durable execution journal, the harness-reported cumulative token
count, exact status, internal execution phase, execution and app identities,
accepted Version, coding harness, and inference route. Raw harness output,
source files, and private prompts never enter the browser; the bounded raw log
remains available through `tohseno service logs`.

`Delete App` always opens an app-specific **Are you sure?** dialog. Confirming
removes an installed copy from the connected iPhone and hides the retired app
from Studio. Source, immutable Shot history, and private receipts stay on the
Mac; an active build cannot be deleted.

## Evolution binds the base for you

Submitting an evolution binds the exact accepted Expression and Version at that
moment. Nobody selects a Version, and there is no separate Feedback ceremony:
writing what should change and pressing **Evolve App** is the whole operation.
If the base genuinely moved first, the service refuses the command — it never
silently rebases — and Studio says:

```text
This app changed while this request was waiting.
Review your request and try again.
```

The exact protocol reason stays available under `Show details`.

## Genesis and entitlement replacement surfaces

Before genesis, Studio replaces the normal views with exactly one current
instruction: cable, Xcode, Trust, Developer Mode, Apple Account, Companion
installation, secure pairing, or the first Shot. It never renders the factory
behind a dismissible modal.

After genesis, the full factory is available during the trial. On the fifth
distinct successful day Studio shows the Pro decision with $9.99 monthly,
$99 yearly, the annual saving sentence, and Not now. A seven-day expiry with
fewer than five days shows only that the trial ended and everything is
preserved. Create/Evolve enforcement is below this browser.

## Settings

Pairing is a one-time cable-genesis concern. The old QR dialog and Add iPhone
button remain deleted; the underlying capabilities remain granular, signed,
and revocable. Diagnostics show the service version, loopback origin,
workspace identity, local coding harness, inference route, and private-channel
state.

## Browser boundary

Studio bootstraps a same-origin session, verifies the service `instance_id` and
origin, and sends an anti-CSRF token with every mutation. It renders through
`textContent` only, never `innerHTML`, and the service serves a strict
no-inline Content-Security-Policy. Workspace changes arrive over the service
event stream instead of polling.

## Tests

```sh
node --check studio/app.js
node --test studio/tests/static_assets.test.mjs
```

Those tests are the guard rail against rebuilding the dashboard: they assert
the four views, the absence of protocol vocabulary on the normal path, the
absence of an extra install button, and upper bounds on the size of each asset.
