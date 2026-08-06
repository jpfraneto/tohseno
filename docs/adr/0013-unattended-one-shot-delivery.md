# ADR 0013: Make one Shot one unattended delivery transaction

Status: accepted

Date: 2026-08-05

Supersedes: the interactive launch and permission decisions in ADR 0005, and
the separate local plan-approval wording in ADR 0011. It does not change ADR
0011's transport boundaries or ADR 0012's conception and acceptance law.

## Context

ADR 0012 correctly moved app-specific interpretation ahead of materialization,
but the first implementation exposed that internal ordering as a second user
ceremony. A person took a Shot, waited for an intelligence to produce a strict
conception, returned to approve the Genome, and then started another harness
phase. ADR 0005 also required the person to press Enter in Terminal before
inference started. The engine could finally accept a Version while no phone
was connected and tell the person to run `tohseno refresh` later.

Those boundaries contradict the product act. The person's exact intention and
choice to take the Shot are the authority for one bounded app birth. A Genome
proposal is engine state derived from that intention, not a new product choice
that needs a second authorization. Likewise, “done” cannot mean that an app is
sealed on disk while delivery remains another task.

Protocol acceptance still needs a validated proposal and a signed
`GenomeAcceptance` action. Removing a user ceremony must not remove those
facts, weaken independent verification, turn a failed candidate into a
Version, or broaden authority to publication, chain deployment, payments, or
other irreversible external actions.

## Decision

`tohseno create` and Studio's **TAKE THE SHOT** action start one durable,
unattended factory run:

1. Preserve the exact Intention and references and establish the durable local
   execution boundary.
2. Start a detached local runner immediately. No Terminal window, editable
   command line, Enter press, or macOS Terminal-automation permission is part
   of the golden path.
3. Invoke every supported coding harness through its native non-interactive
   agent mode with approval prompts bypassed. Harness authentication and the
   selected inference/payment route remain the user's existing local setup.
4. Run intelligent conception, validate the strict Birth Plan, app-specific
   Genome, organs, and Experience Contract, and internally accept the exact
   validated proposal. The signed acceptance action remains in lineage, but
   there is no separate human Genome-approval step.
5. Continue directly into materialization, target-user trials, independent
   engine verification, and bounded repair passes. These remain internal
   phases of the same birth, not separate Shots or Evolutions.
6. Wait for a paired iPhone, build and sign the exact verified candidate,
   install it, and launch it. Device delivery happens before the engine signs
   and finalizes the accepted Version. A missing, untrusted, or unready phone
   leaves the Shot in flight; an install or launch failure leaves the candidate
   unaccepted.
7. Report the Shot complete only after protocol conformance, intent fidelity,
   experience verification, and phone delivery all succeed. Durable event,
   result, follow, and cancel controls remain available without a harness
   conversation UI.

`--no-launch` remains an explicit engineering/test escape hatch that stages a
durable execution without starting it. `tohseno shot run` remains a manual
recovery/debugging command. Neither is the product's default path.

The authorization in this decision is narrowly scoped to local app conception
and materialization inside the Shot workflow. Apple trust, Developer Mode,
signing identity, and harness login may still be prerequisites. Existing
explicit confirmations for publication, contract deployment, Bankr/token
creation, recovery authority, destructive removal, or any other external or
irreversible operation are unchanged.

## Security consequences

Unattended harness execution deliberately grants the chosen coding agent broad
local tool authority for the run. Adapters must disclose and pin the actual
non-interactive and permission-bypass flags they use. The exact Intention,
repository boundary, identity binding, structural Apple gates, independent
tests, protocol verifier, and pre-acceptance device delivery constrain what can
be accepted; they do not make arbitrary agent execution harmless. Private
harness output is retained under the ignored execution directory rather than
proxied through Studio.

Studio remains loopback-only with the same Host, Origin, and mutation-header
checks. A Browser Draft, relay record, Pending Relay Intention, and Local
Pending Intention remain transport or pending state, never a Shot. The one
local TAKE THE SHOT action is still required before any imported intention
starts inference.

## Consequences

The user can take a Shot and leave. Conception and repairs may take time, and
the runner may wait for the phone, but it never asks for a Genome decision or
another start action. Returning to a completed Shot means the accepted app is
already open on the paired iPhone. When the run reaches its final outcome —
accepted, unsealed, cancelled, or stalled — the runner announces it with a
native macOS notification; the notification is a courtesy signal and never
part of the durable record.

The low-level engine `record` operation retains its prior best-effort delivery
behavior for verification tools and tests. The unattended Create/Evolve runner
uses a delivery-required recording path so generic engine callers do not hang
waiting for hardware and the product path cannot complete without it.

Historical Version bytes, protocol schemas, canonical encodings, signatures,
and verification semantics do not change. This is a local orchestration and
acceptance-order decision beneath the normative protocol.
