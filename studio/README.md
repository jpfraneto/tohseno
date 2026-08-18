# TOHSENO Studio 0.9.0

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

There is no dashboard. There is no factory-control surface. Shots, Expressions,
Versions, Executions, harnesses, inference routes, Feedback records, marketing
notes, lineage, and pairing internals are all real inside TOHSENO, and none of
them appear on the normal path.

## Four views

```text
/                 Your Apps          a restrained list, plus + New App
/create           the composer       name · one intent · optional images · Create App
/shots/{id}       one app            the same composer, or its one human state
/settings         Settings           Add iPhone, revoke, diagnostics
```

`tohseno create paper` with no intention opens `/create?name=paper` with the
name already filled in. `tohseno evolve paper` opens that app's composer asking
*What should change?*. Both then follow the same short lifecycle.

If a `./MASTER_PROMPT.md` is present, the CLI imports its exact bytes through
the local pending-intention store and opens `/create?name=paper&pending=…`.
The composer is prefilled and read-only, and nothing is built until the person
presses **Create App** once. TOHSENO never starts a build merely because a file
exists.

Images use the same safe reference machinery as before: up to eight validated
reference images, 64 MB each and 160 MB combined, submitted as exact bytes with
their canonical base64url encoding.

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
failed           Couldn’t build your app.   [Retry]  [Show details]
```

`ready_for_phone` deliberately has no Install, Resume, or Continue button. The
persistent service resumes delivery by itself when the configured iPhone
becomes available, and holds no expensive local resource while it waits.

`Show details` is the pressure-release valve: exact status, internal execution
phase, execution and app identities, accepted Version, coding harness, and
inference route, plus a pointer to `tohseno service logs`. Raw harness output,
source files, and private prompts never enter the browser.

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

## Settings

Pairing is a one-time setup concern, so **Add iPhone** lives in Settings rather
than on every screen. The dialog shows the service-rendered one-use QR and one
sentence about what it grants; the underlying capabilities remain granular,
signed, and revocable. Diagnostics show the service version, loopback origin,
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
