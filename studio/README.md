# TOHSENO Studio 0.9.0

Studio is the loopback-only browser window into the persistent Local Workspace
Service. It is not a separate backend: creation, evolution, feedback,
marketing notes, snapshots, and execution state all pass through the same
Rust application service used by CLI and paired Companions.

```bash
tohseno studio
```

The command ensures the installed service is healthy, opens its verified
`127.0.0.1` origin, and returns. Studio remains available after the Terminal
closes. The service does not bind `0.0.0.0`, and a phone never connects to this
loopback HTTP API.

## Factory surface

The primary regions are:

```text
YOUR SHOTS        INTENT / SHOT ACTIVITY        CURRENT APP / EXECUTION
```

Shot summaries distinguish `factory_shot` from `recording_only` folders and
show stable identities, accepted Version state, real icon state when
available, execution state, and supported actions. Studio shows truthful
states such as Planning, Building, Testing, Waiting for iPhone, Installing,
Accepted, and Failed. A harness exit is not acceptance.

An intentionless interactive command such as:

```bash
tohseno create tohseno
```

opens `/create?name=tohseno`. The creation surface has the prefilled name, one
intention editor, up to eight validated reference images, the owner-configured
local harness and inference-route summary, and one primary **TAKE THE SHOT**
action. It is not a chat interface. Each image is submitted as its exact bytes
with a bounded `studio-file:<filename>` origin descriptor; the browser does not
invent or disclose a local filesystem path.

Feedback is bound to the selected Shot's exact Expression, Version ID, and
Version ordinal. Evolution additionally binds its exact base and selected
Feedback action commitments. If Studio observes a new accepted Version while a
draft is open, it marks that draft stale and requires the explicit **Use current
Version** action. It never silently rebases a draft. Marketing ideas remain
private, append-only, Shot-bound notes.

## CONNECT IPHONE

The upper-right **CONNECT IPHONE** action creates a new pairing session only
when pressed. The modal:

1. renders a standard QR code inside the TOHSENO orange pairing seal while
   preserving the QR quiet zone;
2. shows the approximately two-minute expiry and “Waiting for iPhone…”;
3. reacts to the service event stream and narrowly refreshes only the active
   pairing session while the modal is waiting;
4. names the paired device after key possession and capability checks pass;
5. sends the first encrypted workspace snapshot; and
6. changes to a success state.

Pairing needs no account, email, password, copied token, local IP address, or
second authorization screen. Opening the seal from the explicit button is the
owner authorization. The QR contains no secret key or permanent bearer
credential and cannot select an arbitrary relay URL.

## Paired devices

The paired-device surface shows the device display name and abbreviated ID,
actual pairing and last-seen timestamps, granted capabilities, current sync
state, and **REVOKE**. Revocation is enforced before subsequent command
admission or event delivery and does not delete the phone's recovery phrase.
A restored phone must pair again.

## Local HTTP boundary

The persistent API uses bounded JSON bodies and headers, strict Host and exact
Origin validation, anti-CSRF tokens for mutation, no permissive CORS, and a
streaming event endpoint. Studio first fetches the same-origin, no-store
`/api/v1/studio-session` document. Every JSON mutation then carries its token
in `X-Tohseno-CSRF`; a service-instance mismatch fails identity verification.

Workspace changes arrive over `/api/v1/events` and trigger a debounced
reconciliation. Studio does not poll the whole workspace or run a pairing
request interval. The service reconciles a pending relay rendezvous in the
background and emits a local event; Studio then fetches only that explicitly
opened session. Its local countdown performs one final expiry refresh.

Creation routes use `/create?name=<shot-name>`. Stable Shot links use
`/shots/<shot-id>`, so CLI admission receipts can open the intended local Shot
rather than whichever Shot happened to be selected previously.

Paths and user-controlled file reads reject traversal, symlinks, non-regular
files, and oversized data. An intention exists in the page only while its owner
is editing or submitting it; workspace snapshots and execution events never
send stored prompts back. Studio JavaScript does not receive harness
credentials, private service keys, source code, or raw harness output.

## Static verification

Run the dependency-free browser contract suite from the repository root:

```bash
node --test studio/tests/static_assets.test.mjs
```

The suite checks JavaScript syntax, DOM bindings, strict-CSP compatibility,
the Local Workspace Service endpoint contract, CSRF use, exact Version
bindings, bounded reference handling, SSE behavior, pairing, revocation, and
recording-only compatibility.
