# Product and protocol proposals

These documents organize behavior that is not yet implemented or accepted as an
architecture decision. A proposal can contain a preferred direction without
quietly choosing an unresolved identity, privacy, ownership, cost, or external-
authority policy.

| Proposal | Status | Boundary |
|---|---|---|
| [Application evolution history](APPLICATION_EVOLUTION_HISTORY.md) | Proposed | Owner intent → manifest → release provenance |
| [Phone-to-browser bridge](PHONE_BROWSER_BRIDGE.md) | Proposed / Open | QR pairing, scoped browser delegation, typed operations |
| [External action rails](EXTERNAL_ACTION_RAILS.md) | Proposed / Open | Optional chain, exchange, and brokerage operations |
| [Agent initializer](AGENT_INITIALIZER.md) | Proposed / Open | Pinned local one-line creation and agent selection |
| [Deployment cell](DEPLOYMENT_CELL.md) | Proposed / Open | One-package, one-command containerized backend per application |

The [POST route authority audit](../POST_AUTHORITY_AUDIT.md) is the factual
companion to the bridge proposal: it classifies every implemented mutating
route by the authority that admits it today.

When a proposal is ready to become a decision, follow the process in
[Open questions](../OPEN_QUESTIONS.md): name the exact behavior, manifest and
contract effects, privacy and ownership consequences, executable tests,
compatibility, rollback, and staged release. Only then add or supersede an ADR.
