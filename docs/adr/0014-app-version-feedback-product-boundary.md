# ADR 0014: TOHSENO is an app-local recording layer

Status: accepted

Date: 2026-08-15

Supersedes: ADR 0012 and ADR 0013 as descriptions of the current user-facing
creation and evolution flow. Their historical accepted records, protocol
encodings, signatures, fidelity evidence, and verifier semantics remain
unchanged.

## Context

TOHSENO had become responsible for interpreting an intention, selecting and
driving a coding harness, planning an app, building it, signing it, installing
it, and launching it. That made the recording system part of the act of making
software instead of a durable account of what was made.

The intended boundary is closer to Git's relationship with a working tree:
the app is an ordinary visible folder, while private recording data lives in a
dedicated metadata folder beside it. This analogy concerns placement and
responsibility, not Git's wire format or distributed semantics.

## Decision

The app folder is the working source of truth. TOHSENO owns only the embedded
`.tohseno/` directory and the immutable Versions recorded there.

`tohseno create <name>` initializes a new visible app folder, or adopts an
existing one, by adding its `.tohseno/` recording state. It does not ask for an
intention, select a harness or model, spend inference, require Xcode or an
iPhone, build, sign, install, or launch an app.

`tohseno evolve [name]` records the current ordinary files as the next Version,
with an optional exact note. The snapshot excludes only recording and source
control metadata (`.tohseno/` and `.git/`); application files are not silently
classified as disposable. Each completed Version retains its source snapshot
and integrity material under the app's `.tohseno/` directory.

Editors, coding agents, Xcode, build systems, and deployment tools operate in
parallel with TOHSENO. They may change the working tree however the user
chooses. Recording a Version is a separate, explicit action and never causes
an external publication, build, installation, contract action, registry
write, or token action.

CLI and Studio use the same engine operations for initializing an app and
recording a Version. Studio presents only the local app list, selected app
folder, recorded Versions, and the action to record the working tree. It does
not contain factory, harness, model, route, cost, protocol, network, Bankr, or
token-launch workflows.

New recording-layer history is distinguished from historical protocol
lineages. The engine does not append the simplified record format to an app
that contains historical protocol history. Frozen protocol artifacts remain
verified under the rules that created them.

## Consequences

TOHSENO can describe what changed without becoming the means by which the
change was produced. A person can use any development process, then record the
result in place. Missing harness credentials, Xcode setup, signing state, and
phone connectivity are no longer TOHSENO creation or evolution failures.

ADR 0012's conception system and ADR 0013's unattended delivery transaction
remain important history, but they no longer govern the ordinary product path.
No canonical protocol bytes or historical accepted records are rewritten by
this decision.
