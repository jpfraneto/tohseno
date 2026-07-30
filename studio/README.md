# TOHSENO Studio

Studio is a localhost-only view and intake surface over the same engine and
protocol used by the CLI. It does not define a second Shot model and it does
not read Shot folders directly in browser JavaScript.

## Local Shot execution

Studio loads installed harness adapters from the engine, including detectable
authentication, model choices, payment route, attachment behavior, and honest
additional-cost information. Its primary action prepares the Shot; it does not
start inference.

Preparation persists the private intent package, records a Git tree boundary,
creates a durable local execution identity, and opens a native terminal with a
`tohseno shot run` command in the editable zsh line buffer. The user presses
Enter in that terminal. Codex or Claude Code then runs with inherited terminal
input and output; Studio never renders or proxies the harness conversation.

Studio follows:

```text
.tohseno/executions/<execution-id>/events.jsonl
.tohseno/executions/<execution-id>/completion.json
```

through loopback read APIs. The CLI follows the same files with
`tohseno shot follow` and inspects the same result with
`tohseno shot result`. These records, intention bytes, images, and any harness
transcript remain ignored private material and are never sent to a node.

`tohseno studio` listens on `http://127.0.0.1:8888` by default. Port 88 is a
privileged system port on macOS and would require running TOHSENO as root, so
Studio deliberately uses the closest memorable unprivileged port instead.
Pass `--port 0` to let macOS choose any available loopback port.

## First-run onboarding

An installation with no accepted Shots opens a four-step guide before the
composer:

1. establish the local/private and human-confirmed execution boundaries;
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
The optional `token_association` view is shown separately from the frozen
GENESIS Appcoin receipt. It is explicitly a relationship, never Shot identity
or ownership, and a declared chain anchor is not labeled verified without a
chain-specific check.
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

Studio exposes `$TOHSENO` launch only inside a selected, verified Shot. It is
not a global Studio or onboarding action. The local Rust server independently
resolves the selected app and version to its authoritative `ShotID`; the
simulation approval, exact confirmation phrase, private Bankr receipt, and
signed Token Association all bind to that identity. A Shot that already has a
current token association cannot launch an unbound replacement through this
surface.

The server calls Bankr's `POST /token-launches/deploy` endpoint; the API key is
never embedded in HTML, returned by an API route, written to a receipt, or sent
to browser JavaScript.

The launch identity is fixed:

- token name and symbol: `TOHSENO`;
- fee recipient: `jpfraneto.eth`;
- currently pinned ENS resolution:
  `0xed21735DC192dC4eeAFd71b4Dc023bC53fE4DF15`;
- signer: the Bankr wallet that owns the user API key.

The Bankr receipt is stored privately under the selected Shot's
`.tohseno/token-launches/` directory. After a successful deployment, Studio
appends a public-availability Token Association action to that Shot's signed
local lineage and writes its explicit relay artifact to the normal outbox.
Neither relation changes Shot identity or ownership.

Create a dedicated user key at `https://bankr.bot/api-keys`. It must begin with
`bk_usr_`, have token-launch access enabled, and have read-only mode disabled.
Enable no unrelated API capabilities. In a fresh zsh session, start Studio
without putting the key on a command line:

```sh
read -s "BANKR_API_KEY?Bankr user API key: "
echo
export BANKR_API_KEY
export TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY=1
tohseno studio
```

Unset both values after Studio exits:

```sh
unset BANKR_API_KEY TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY
```

The browser can configure metadata, Robinhood Chain or Base, the documented
15% creator vest or no vesting, and mixed or quote-only creator fees. Studio
always calls Bankr in `simulateOnly` mode first. It rejects the simulation
unless Bankr returns the selected chain and resolves the creator allocation to
the pinned personal wallet. A successful simulation creates one in-memory,
single-use approval that expires after ten minutes. Deployment additionally
requires the exact phrase containing the chain, ENS name, and predicted token
address.

If a deployment response is lost, Studio reports the outcome as unknown and
does not reuse the approval. Check Bankr's recent launches before doing
anything else; never retry an irreversible request merely because the client
timed out.

After a confirmed deployment, Studio stores the Bankr response and exact
approved configuration under the candidate machine root's
`bankr-launches/` directory with private filesystem permissions. The receipt
contains no API credential. Deployment does not automatically make the token
a Shot or create a Token Association; that remains a separate
owner-authorized protocol action.
