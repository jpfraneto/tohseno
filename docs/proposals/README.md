# Product and protocol proposals

These documents separate implemented behavior from designs that still need
product, security, ownership, or executable evidence. Historical proposals may
describe the intake product preserved on `archive/intake-product`; current main
contains no intake or generated-app backend.

| Document | Status | Boundary |
|---|---|---|
| [Application evolution history](APPLICATION_EVOLUTION_HISTORY.md) | Proposed | Owner intent → manifest → release provenance |
| [Phone-to-browser bridge](PHONE_BROWSER_BRIDGE.md) | Proposed / Open | QR pairing, scoped browser delegation, typed operations |
| [External action rails](EXTERNAL_ACTION_RAILS.md) | Proposed / Open | Optional chain, exchange, and brokerage operations |
| [Local agent initializer](AGENT_INITIALIZER.md) | Historical Phase 1 record; superseded | Backward-compatible factory foundation now extended by the agent-first launcher |
| [Deployment cell](DEPLOYMENT_CELL.md) | Proposed / Open | One-package containerized backend per application |

The implemented factory contract is documented in [CLI and machine
operations](../CLI.md) and [System architecture](../SYSTEM_ARCHITECTURE.md).
Proposals do not authorize paid resources, credential changes, deployment,
package publication, or private-data disclosure.

When a proposal becomes an accepted architectural decision, add or supersede an
ADR rather than rewriting the historical decision record. Use the repository's
documentation vocabulary precisely: **Implemented**, **Prepared**,
**Proposed**, and **Open** are not interchangeable.
