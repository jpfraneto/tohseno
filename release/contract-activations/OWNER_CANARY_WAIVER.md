# Owner waiver — production canary before activation signing

On 2026-08-02 (UTC), asked explicitly whether to run the 72-hour production
canary required by `release/CONTRACT_0_8_0_PRODUCTION_CANARY_RUNBOOK.md`
before signing the generation 0.8.0 activation, the owner chose:

> Waive it, sign today

with the presented consequence stated plainly: the BuilderAccount recovery
path will not have been exercised on the real chain before real identities
rely on it, and the activation binds the locally instantiated BuilderAccount
runtime hash
`0xb5ff14ddc150b2f64cb2243e6d8c8a0c441007841548f1b0ee8d6e22ad452fc0`
from the deployment audit
(`contracts/audits/robinhood-inactive-deployment-0.8.0-20260801T021920Z.json`,
`activation_conformance_issue.builder_account_locally_instantiated_runtime_keccak256`)
instead of a canary-established production instance hash.

The owner also delegated ceremony execution (key generation, signing,
assembly) to the Claude Code session in the same conversation ("i give you
power"); approvals and decisions above remain the owner's own.

A retroactive canary remains possible and advisable: the drills in the canary
runbook can still be run against the activated contracts, and any defect found
creates a successor generation per the runbook's incident rules.

This file is ceremony evidence and travels with the activation into
`release/contract-activations/` at the activating release commit.
