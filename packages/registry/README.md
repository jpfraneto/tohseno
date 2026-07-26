# Public record registry

The registry verifies and appends signed records, rejects forks relative to its
already accepted history, and derives a deterministic public projection. Given
the same accepted history, another registry derives the same projection.

The first valid genesis attestation accepted for a Shot ID establishes that
registry's local branch. Separately seeded registries can accept different
valid genesis attestations that claim the same Shot ID; choosing a production
trust root or preferred cross-node branch is outside protocol v1 and remains
Open. The in-memory implementation is a reference adapter, not an ownership or
consensus authority.

There is no update or delete operation. The anchor interface is a
chain-neutral seam only. This package contains no chain implementation,
deployment tool, token action, or TOHSENO host.
