# Architecture decision records

Architecture decision records capture choices that implementation must preserve. They are not a list of aspirations: each record below is accepted for the first TOHSENO foundation.

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-stable-event-identity.md) | Accepted | Stable continuity event identity is separate from artifact hashes |
| [0002](0002-sealed-artifacts-are-immutable.md) | Accepted | A sealed continuity artifact is immutable |
| [0003](0003-reflections-are-separate-records.md) | Accepted | Reflections are separate records linked to events/artifacts |
| [0004](0004-versioned-actual-route-signatures.md) | Accepted | Signed envelopes are versioned and bind actual method/path |
| [0005](0005-separate-identity-recovery-from-restoration.md) | Accepted | Practice-identity recovery is separate from other restoration/ownership |
| [0006](0006-content-hash-plus-independent-capability.md) | Accepted | Customer Markdown uses a content hash and independent capability |
| [0007](0007-capability-authorizes-hash-identifies.md) | Accepted | Capabilities authorize; content hashes identify integrity |
| [0008](0008-ejectable-from-birth.md) | Accepted | Every generated app is ejectable from birth |
| [0009](0009-control-plane-metadata-only.md) | Accepted | Control plane stores operations, not end-user continuity content |
| [0010](0010-ai-at-manifest-boundary.md) | Accepted | AI interprets meaning at the manifest boundary; runtime invariants are deterministic |

Use a new ADR to change or supersede an accepted decision. Do not edit an accepted decision into a different meaning after implementation depends on it. Open product questions remain in [OPEN_QUESTIONS.md](../OPEN_QUESTIONS.md) until evidence and product ownership support a decision.
