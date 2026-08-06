# TOHSENO Studio

Studio is a localhost-only view and intake surface over the same engine and
protocol used by the CLI. It does not define a second Shot model and it does
not read Shot folders directly in browser JavaScript.

## Imported local pending intentions

`tohseno intent claim --stdin` and `tohseno intent open <file>` validate the
same noncanonical private package in the engine and atomically import it under
the machine data root (`~/.tohseno` by default). That Local Pending Intention
is durable user state, is preserved by uninstall, has no Shot ID, and contains
no relay capability or decryption key. Re-importing the same package returns
the same local pending ID.

The CLI opens Studio with only that opaque local ID. The localhost server
serves the prompt metadata and bounded reference endpoints; private values are
never put in the URL. If readiness is incomplete, Studio shows the actual
onboarding gates and keeps the intention visibly safe. Once ready, it opens
the existing creation surface with exact prompt and reference order, a local
deterministic editable name suggestion, harness/model controls, cost route,
a single TAKE THE SHOT action. Studio never displays a generic Genome as
though it were an app-specific interpretation: no Genome exists until the
selected intelligence has read the exact intention and Apple capability
context, and the engine internally accepts only that validated proposal.

`POST /shots` accepts exactly one source: the existing inline composer fields
or `pending_intention_id`. The server resolves pending content and executes the
same planning, unattended execution, and delivery path. Ambiguous mixed input
is rejected. The record is consumed only after successful preparation and
runner start; onboarding interruption, restart, cancellation, or preparation
failure leaves it ready.

## Local Shot execution

Studio loads installed harness adapters from the engine, including detectable
authentication, model choices, payment route, attachment behavior, and honest
additional-cost information. Its primary action prepares and starts the Shot;
it does not ask for a second authorization.

Preparation persists the private intent package, records a Git tree boundary,
creates a durable local execution identity, and starts a detached runner.
Codex or Claude Code runs in its supported non-interactive mode and first
returns strict conception artifacts. After deterministic validation, the
engine internally accepts the proposal and immediately continues into
materialization, target-user trials, repair, verification, iPhone install, and
launch. The engine, not the harness, owns final acceptance, and it does not
finalize the Version until delivery succeeds. Studio never renders or proxies
the harness output; private output is retained in `harness.log` beside the
execution records.

Studio follows:

```text
.tohseno/executions/<execution-id>/events.jsonl
.tohseno/executions/<execution-id>/completion.json
```

through loopback read APIs. The CLI follows the same files with
`tohseno shot follow` and inspects the same result with
`tohseno shot result`. These records, intention bytes, images, and any harness
transcript remain ignored private material and are never sent to a node.

`GET /api/apps` rediscovers the filesystem ledger on every request and lists
only committed Evolutions. An Evolution that fails its gates is never
committed, so it never appears as accepted; once the corrected Evolution
commits, a plain browser refresh shows the app and its latest Evolution
without restarting Studio, because no library state is cached between
requests.

`tohseno studio` listens on `http://127.0.0.1:8888` by default. Port 88 is a
privileged system port on macOS and would require running TOHSENO as root, so
Studio deliberately uses the closest memorable unprivileged port instead.
Pass `--port 0` to let macOS choose any available loopback port.

## First-run onboarding

An installation with no accepted Shots opens a four-step guide before the
composer:

1. establish the local/private one-action execution boundary;
2. check the real Xcode toolchain and Apple Development signing state;
3. install and authenticate either Codex or Claude Code through its official
   native flow, with a detected `$0.00` subscription route;
4. begin one deliberately small first Shot with a reference image.

The browser cannot mark machine requirements ready. `GET /api/onboarding`
derives them from the same engine gates and harness adapters used by actual
execution. Browser storage remembers only guide position and completion, never
Xcode, signing, identity, or harness authority. The guide can always be
reopened from the `?` control beside the Studio wordmark.

The Genesis one-line installer starts Studio by default and keeps it attached
to the installing terminal. `TOHSENO_START_STUDIO=0` is the explicit
automation/package-inspection opt-out.

The server must continue to bind loopback only, validate the exact `Host`, and
require the exact same-origin JSON headers for every mutation. Private
intention and feedback bytes must never be sent to a node by Studio.

## Optional ontology projection

`GET /api/protocol/shot/{app}/{ordinal}` remains compatible with the frozen v1
response. A current server may add:

```json
{
  "ontology": {
    "status": "verified",
    "shot_id": "0x…",
    "original_intention": {},
    "accepted_genome": {},
    "expression": {},
    "version": {},
    "lineage": {
      "sequence": 8,
      "head": "0x…",
      "verification": "verified",
      "availability": "locally_available"
    }
  }
}
```

The values are protocol records or derived views returned by engine APIs.
Studio renders them as text and does not reinterpret them as canonical state.
The optional `token_association` view is explicitly a relationship, never Shot
identity or ownership. A declared chain anchor is not labeled verified without
a chain-specific check.
If the projection is absent, Studio labels the selected record as compatible
v1 history and does not fabricate intention, genome, or exact Version facts.

## Private feedback

When the projection contains an exact Expression ID, Version ID, and positive
version ordinal, Studio may submit:

```json
{
  "app_name": "example",
  "version_ordinal": 1,
  "text": "The exact experience observed in this build."
}
```

to `POST /api/feedback` with the normal Studio same-origin headers. The server,
not the browser, resolves the accepted Version record, validates the Feedback
record, and stores it under the private version-bound Shot layout.

The response distinguishes the payload from the signed lineage reference:

```json
{
  "feedback_id": "0x…",
  "action_commitment": "0x…",
  "private": true,
  "version_ordinal": 1
}
```

When the user checks “Select this signed observation for the next evolution,”
Studio carries that exact `action_commitment` in the subsequent `/shots`
evolution request. The engine—not browser state—then proves that the action
exists and binds the current exact Expression and Version. Saving Feedback
alone does not silently select it.

## Contract generation status

`GET /api/protocol` reports the committed, reproducible contract build
definition for generation `0.8.0`, including its definition digest, exact
runtime code hashes, source commit, target chain, and EIP-7951 requirement.
That definition is intentionally `inactive`, and `active_generation` is
`null`: no trusted release-authority root or signed activation exists.

A build definition is not deployment evidence. Studio performs no TOHSENO
witness-chain RPC, does not deploy or broadcast TOHSENO witness contracts,
exposes no predicted address as authority, and has no public Shot publication
path. Its separately confirmed Bankr token-launch workflow does not activate a
TOHSENO contract generation or publish a Shot. Studio also exposes no
ShotRelations, handle, App Store attestation, Appcoin, or device-pairing
surface. A future release must add separately authorized activation evidence
before Studio may represent a public witness generation as active.

## Optional node status

`GET /api/node` may return `configured`, `reachable`, `identity`,
`protocol_version`, `replicated_shots`, and `detail`. A missing route is
equivalent to no configured node. Node unavailability never disables local
creation, evolution, verification, or feedback.

The bundled server reads a local node only when `TOHSENO_NODE_ROOT` names an
absolute, existing node store. It opens that store through the node's own
validator and reports integrity-derived holdings; it does not infer network
completeness or copy private Shot material into the node.

Run the static contract checks with:

```sh
node --test studio/tests/static_assets.test.mjs
```

## Shot-bound programmatic Bankr launch

Studio exposes Appcoin launch only inside a selected, verified Shot. It is
not a global Studio or onboarding action. The local Rust server independently
resolves the selected app and version to its authoritative `ShotID`; the
simulation approval, exact confirmation phrase, private Bankr receipt, and
signed Token Association all bind to that identity. A Shot that already has a
current token association cannot launch an unbound replacement through this
surface.

The server calls Bankr's `POST /token-launches/deploy` endpoint. When no
environment key is configured, the modal asks for a user key and sends it once
over the loopback Studio request; Studio holds it zeroized in process memory
for the resulting single-use approval. A key is never returned by an API
route, written to a receipt, persisted in browser storage, or saved to disk.

The launch identity is derived from the Shot, while the person launching
chooses the recipient and metadata:

- token name: the Shot's own name (`exhale`);
- token symbol: that name upper-cased, separators removed, bounded to 10
  characters (`EXHALE`) — one Appcoin per Shot, never a shared ticker;
- fee recipient: one ENS name, wallet address, X account, or Farcaster account;
- resolved recipient address: returned by Bankr simulation and pinned for the
  approved deployment;
- signer: the Bankr wallet that owns the user API key.

The Bankr receipt is stored privately under the selected Shot's
`.tohseno/token-launches/` directory. After a successful deployment, Studio
appends an intentionally private Token Association action to that Shot's
signed local lineage. It creates no relay artifact, registry transaction, or
public Shot claim. Neither the token deployment nor this private relationship
changes Shot identity or ownership.

Create a dedicated user key at `https://bankr.bot/api-keys`. It must begin with
`bk_usr_`, have token-launch access enabled, and have read-only mode disabled.
Enable no unrelated API capabilities. Enter it when the launch modal requests
it, or, for operator-managed startup, supply it from a fresh zsh session
without putting the key on a command line:

```sh
read -s "BANKR_API_KEY?Bankr user API key: "
echo
export BANKR_API_KEY
tohseno studio
```

Unset the key after Studio exits:

```sh
unset BANKR_API_KEY
```

The browser can configure metadata, recipient, Robinhood Chain or Base, the
documented 15% creator vest or no vesting, and mixed or quote-only creator
fees. A valid HTTPS image URL updates the public token preview. Studio always
calls Bankr in `simulateOnly` mode first. It rejects the simulation unless
Bankr returns the selected chain and a valid resolved creator address; a raw
wallet recipient must resolve to that exact wallet. A successful simulation
pins that address in one in-memory, single-use approval that expires after ten
minutes. Deployment additionally requires the exact phrase containing the
chain, recipient type/value, and predicted token address, and its response must
preserve the simulated recipient address.

If a deployment response is lost, Studio reports the outcome as unknown and
does not reuse the approval. Check Bankr's recent launches before doing
anything else; never retry an irreversible request merely because the client
timed out.

After a confirmed deployment, Studio stores the Bankr response and exact
approved configuration under the selected Shot's `.tohseno/token-launches/`
directory with private filesystem permissions. The receipt contains no API
credential. Studio then attempts the separate owner-authorized private Token
Association action using the Shot-derived symbol and reports a warning if that
local action cannot be recorded. The token is never made into the Shot, and no
public witness write follows from the deployment.
