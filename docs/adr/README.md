# Architecture decision records

Architecture decision records capture choices the current implementation must
preserve. The sequence starts with the canonical app-factory and Shot
architecture and contains only active decisions.

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-ejectable-from-birth.md) | Accepted | Every generated app is independently owned and ejectable from birth |
| [0002](0002-private-data-out-of-factory-and-node.md) | Accepted | Private intent and app-runtime content stay out of the factory and public node |
| [0003](0003-ai-at-manifest-boundary.md) | Accepted | AI interprets intent before deterministic manifest and composition boundaries |
| [0004](0004-intention-kernel-template-skill-composition.md) | Accepted | A Shot composes one kernel, one template, and an ordered locked skill set |
| [0005](0005-shot-protocol-and-reference-node.md) | Accepted | Signed Shot records precede deterministic registry and node projections |

Use a new ADR to change an accepted decision. Open product questions remain in
[Things that are not clear yet](../../THINGS_THAT_ARE_NOT_CLEAR_FOR_TOHSENO_YET.md)
until evidence and product ownership support a decision.
