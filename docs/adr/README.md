# Architecture decisions

Accepted ADRs are authoritative architecture decisions beneath `protocol/`.

[ADR 0024](0024-app-local-tohseno-git-boundary.md) makes `.tohseno/` an
integral Git-visible part of each app repository while retaining exact ignores
for intentions, inline-private lineage, references, feedback, executions,
logs, artifacts, and `.tohseno/private/`. Git visibility never substitutes for
an explicit public-registry publication flow.

[ADR 0023](0023-per-shot-harness-and-model-choice.md) adds one compact
Studio creation choice for an installed coding harness and one of its
associated models. The resolved selection is persisted with the private
creation command so recovery cannot switch it, while ADR 0019's one bounded
implementation harness and repair ceiling remain unchanged.

[ADR 0022](0022-optional-model-chosen-app-name.md) makes the app name optional.
When omitted, the factory reserves a local technical slug and gives the one
existing implementation model responsibility for choosing the user-facing
product name from the exact intention, with no extra planning invocation.

[ADR 0021](0021-npm-install-enters-first-run.md) makes a fresh global npm
install enter the existing first-run product surface without a second command.
It preserves ADR 0020's verified native bootstrap and cable genesis while
keeping local dependency installs and already-installed updates free of an
automatic open.

[ADR 0020](0020-cable-genesis-earned-pro-npm-front-door.md) defines TOHSENO
0.9.9's complete product boundary: the first Mac↔iPhone relationship begins
through the cable while the existing cryptographic pairing remains intact;
the complete factory is available for at most seven calendar days; five
accepted physical-delivery days qualify a person for $9.99 monthly or $99
yearly Pro; factory admission is enforced below every UI; Apple membership is
independent; and the npm package is a thin verified native-release front door.
It authorizes no publication or production external activation.

[ADR 0019](0019-bounded-intent-to-usable-app.md) defines the current execution
hot path: one implementation harness, at most one concrete code/build repair,
one shared wall-clock budget, deterministic build and delivery, and one private
State Transition Receipt. CLI, Studio, and Companion remain origins of the
same durable application operation. It supersedes ADR 0017's older supervision
defaults and harness-authored Experience Trial requirement without changing
the public protocol or accepted history.

[ADR 0018](0018-the-companion-links-a-browser.md) records that the website
will never hold an account. When the public terminal can send more than once,
it will be the published Companion linking a browser to a phone: the phone
keeps the identity and issues a scoped, revocable capability, generated apps
stay free of identities and grants, and a linked browser still reaches only
the Mac that phone is paired with. It is accepted and deliberately not
implemented, and it lists what must be true before it can be.

[ADR 0017](0017-the-engine-composes-the-genome.md) defines how a birth runs:
the engine composes and accepts each Shot's Genome and Expression itself, and
the single harness invocation reads the exact human intention. It supersedes
[ADR 0012](0012-intention-led-app-birth.md)'s Conception phase while keeping
ADR 0012's intention-led birth and engine-owned acceptance intact. It also
bounds one unattended harness invocation by stall and total runtime.

[ADR 0016](0016-app-intent-app-on-your-iphone.md) defines the current
user-facing surface: the canonical abstraction is App → Intent → App on your
iPhone, and Studio and the Companion are thin projections over the same durable
local application service. It supersedes ADR 0015 as the description of what a
person sees, deliberately including what was deleted to get there. It does not
change ADR 0015's service, journal, capability, transport, or relay
architecture.

[ADR 0015](0015-persistent-local-factory-private-companion.md) defines the
current internal boundary: one persistent local app factory with CLI, loopback
Studio, and a private paired-companion channel. It supersedes
[ADR 0014](0014-app-version-feedback-product-boundary.md) as the description
of current `create`, `evolve`, and Studio behavior while preserving ADR 0014's
exact app-local recording format through explicit `init` and `record`
commands. Recording-only folders are not silently migrated into factory Shots.

[ADR 0011](0011-encrypted-web-to-local-intention-handoff.md) still defines the
historical web-to-local intention transport.
[ADR 0006](0006-public-witness-and-contract-generation.md) remains
authoritative for public-witness and contract-generation boundaries. None of
these ADR summaries override canonical encodings or validation rules in
`protocol/`.
