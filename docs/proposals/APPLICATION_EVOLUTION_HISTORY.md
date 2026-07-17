# Proposal: application evolution history

- Status: **Proposed**
- Serves: deterministic AI boundary, manifest-diff discipline, owner approval,
  rollback, and ejection
- Does not implement: storage, encryption, signing, or revision APIs

## Problem

The current intake stores one encrypted `MASTER_PROMPT.md`, while the manifest
and source tree represent only the current application state. `order_events`
record commercial/operator transitions, and `ContinuityEvent` records a person's
practice. Neither is the history of why the owner changed the software.

An evolving continuity app needs a trace from the owner's request to the exact
contract and release that resulted. That history must preserve private intent
without turning public Git, logs, or a chain into a prompt archive.

## Proposed lineage

```text
IntentRevision
    → ManifestRevision
        → ReleaseRevision
            → runtime ContinuityEvent.actionPolicy version
```

These records remain separate because they have different owners, privacy,
deletion, and integrity requirements.

### Intent revision

An intent revision represents the owner's exact request and its disposition.
Candidate fields include:

- stable revision ID and optional parent revision IDs;
- application ID and creation time;
- separately encrypted exact source bytes or an owner-controlled private
  reference;
- private integrity digest and byte length;
- source kind and owner/update authority;
- state: `received`, `distilled`, `proposed`, `approved`, `refused`, `withdrawn`,
  or `superseded`;
- safe reason codes and a deletion tombstone when source bytes are removed.

The stable ID does not derive from the prompt digest. The raw request is never an
`order_events.metadata` value, log field, URL, payment field, email subject,
public-chain payload, or public-repository file.

### Manifest revision

A manifest revision represents a validated product decision, not an agent's
unbounded interpretation. Candidate fields include:

- stable revision ID and parent manifest revision;
- source intent revision IDs;
- exact before/after manifest digests and a deterministic diff;
- accepted, refused, and still-open requirements;
- owner approval record when privacy, authority, cost, or ritual semantics
  change;
- schema and policy versions.

The current manifest is a projection of the accepted history. Replacing the
current file does not erase its ancestors.

### Release revision

A release revision records implementation evidence:

- stable release ID and manifest revision ID;
- source repository and commit;
- generated-file and dependency provenance;
- build, test, migration, backup, rollback, and ejection evidence;
- target environments and deployment artifact identifiers without credentials;
- status such as `built`, `verified`, `released`, `failed`, or `superseded`.

Rollback creates another release revision. It never rewrites the failed release
or pretends the owner did not request the change.

## Agent workflow

For an existing application, an agent should:

1. preserve the exact request only in the owner-approved private location;
2. create a safe evolution-index entry before code;
3. distill the request into one proposed manifest diff;
4. list unsupported and open requirements instead of implementing around the
   schema;
5. obtain owner confirmation for material product, privacy, cost, disclosure,
   or authority changes;
6. implement and attach focused/full verification evidence;
7. record the release or rollback result.

No record should contain hidden model reasoning. The durable evidence is the
owner request, explicit distillation, manifest diff, code diff, tests, decisions,
and outcome.

## Privacy and deletion

Exact intent may reveal unfinished ideas, business plans, private data, or
credentials pasted by mistake. The default is encrypted, owner-controlled,
uncached, unindexed storage with narrow access auditing. A public repository may
contain a sanitized distillation only when it has been reviewed for publication.

Deletion of exact source bytes may be allowed without erasing the structural
fact that a revision existed. A tombstone can preserve parentage and release
integrity without retaining the deleted prompt. The threat model must decide
whether even a public or durable digest creates unacceptable correlation or
dictionary risk.

## Required decisions before implementation

- Where do exact intent bytes live for self-hosted, client-owned, and operated
  modes?
- Which authority may submit, approve, withdraw, or delete an intent revision?
- Is approval a local action, a server capability, a signature, or a combination?
- Which branches and concurrent updates are supported?
- How are prompt retention and ejection handled after the control-plane order
  expires?
- Which safe parts may be published as tutorial material, and through what
  separate consent?

## Acceptance evidence

- parent replacement, orphan, cycle, and concurrent-branch negatives;
- exact-byte/digest fixtures including Unicode without normalization;
- encrypted-at-rest source and content-free logs/errors/WAL inspection;
- deterministic manifest diff and schema validation;
- unauthorized approval, deletion, and release-link rejection;
- rollback as a new record;
- deletion tombstone and ejection round trip;
- release-to-manifest-to-intent trace without needing TOHSENO credentials.

## Non-goals

- treating an agent transcript as application state;
- committing every raw prompt to Git;
- storing model chain-of-thought;
- using the practice identity automatically as the owner/update authority;
- putting private intent or its mandatory canonical history on a blockchain.
