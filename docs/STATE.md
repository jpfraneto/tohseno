# State of this repository

Written 2026-07-30, amended through 2026-08-15. This is the plain-language
answer to “what is going on here” for someone returning after time away. When
something below stops being true, update this file in the same change.

## Current source: recording beside the filesystem

ADR 0014 defines the current product. TOHSENO is an app-local recording layer,
not an app factory. The visible app directory is the working tree and remains
usable by any editor, coding agent, build system, or deployment tool. TOHSENO's
state lives inside that directory at `.tohseno/`.

`tohseno create <name>` initializes a new app directory or adopts an existing
one by adding its embedded history. It performs no inference and requires no
harness, model, route, Xcode installation, signing identity, Simulator, or
iPhone. `tohseno evolve [name]` snapshots the directory's current ordinary
files as its next Version, optionally retaining an exact note. Snapshotting
excludes `.tohseno/` and `.git/`; it does not silently omit application files
because of their name or purpose.

Studio is the loopback-only view of the same boundary. It lists app folders and
Versions, initializes or adopts a folder, opens the selected folder, and
records its current state. It does not present intention composition, coding
harnesses, models, routes, cost, build or installation controls, protocol
graphs, public registries, Bankr, or token launch.

Creating or recording an app never builds, signs, installs, launches, publishes,
or contacts a network. Those actions belong to the user's independent tools.
TOHSENO can record their resulting filesystem changes without becoming part of
how those changes were produced.

New recording-layer histories are explicitly distinguished from historical
protocol lineages. Current source refuses to append simplified Versions to an
app containing historical protocol history. Historical accepted directories,
commitments, signatures, and verification rules are unchanged.

## Relationship to ADR 0012 and ADR 0013

ADR 0012 specified an intention-led app factory with conception, structured
planning, implementation, and multi-dimensional acceptance. ADR 0013 made that
factory an unattended transaction ending in iPhone installation and launch.
ADR 0014 supersedes both as descriptions of the current `create`, `evolve`, and
Studio experience.

Their protocol facts and historical evidence have not been reinterpreted or
deleted. Records produced under those flows continue to verify under their own
recorded provenance. The conception, harness, delivery, and repair machinery
may remain where historical verification or advanced recovery still depends on
it, but it is not the current product contract.

## Published release versus repository source

The pinned public installer still identifies immutable release 0.8.5. That
release predates ADR 0014 and contains the older intention-led factory and
web-to-local handoff behavior. The source described above has not been
published, tagged, deployed, or substituted into the installer by this change.
Installed 0.8.5 users therefore do not receive the recording-layer interface
until a separate release action occurs.

The production relay evidence for the 0.8.5 web intention handoff remains in
`release/WEB_INTENTION_HANDOFF_ACTIVATION.md`. A Browser Draft, relay record,
Pending Relay Intention, and Local Pending Intention were transport state, not
canonical protocol records. That distinction remains historically true even
though intention handoff is outside the current product boundary.

## Protocol and contract history retained

`protocol/` remains normative over every prose document. Its schemas, canonical
byte encodings, conformance rules, and test vectors are unchanged. Frozen v0.7
inputs and accepted history remain available for exact offline verification;
the v0.7 contract generation was retired and was never deployed.

The remediated 0.8 contract generation was deployed to Robinhood Chain mainnet
as a candidate on 2026-08-01 and activated by the recorded owner ceremony on
2026-08-02. Its policy and activation evidence remain under `release/` and
`contracts/`. Public Builder identity, registry publication, and receipt flows
remain separate from the local recording layer. Ordinary `create` and `evolve`
perform no contract, registry, payment, publication, or token action.

No frozen protocol encoding, accepted record, contract source, deployment
evidence, release artifact, or installer pin is modified as part of the ADR
0014 product-boundary change.
