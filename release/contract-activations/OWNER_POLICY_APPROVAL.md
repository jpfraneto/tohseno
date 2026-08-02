# Owner approval — release-authority policy digest

On 2026-08-02 (UTC), in a Claude Code session in this repository, the owner
(git identity Jorge Pablo Franetovic Stocker, jpfraneto@gmail.com) wrote,
verbatim:

> I approve policy digest 0xf14410692ebe34f6855b8dbec5cb08733aa737f1cd86f385694e4fb575df943c

The approved policy is the 2-of-3 `tohseno.release-authority-policy/1`
instance at `release-authority-policy.json` in this directory, issued at
2026-08-02T01:35:29Z, whose RFC 8785/SHA-256 digest was independently
reproduced by `scripts/prepare-release-authority-policy.py` and
`protocol/examples/verify_release_authority_policy.rs` before approval.

Custody note (owner-accepted deviation): all three authority keys were
generated on the owner's Mac in `~/tohseno-authority-keys/` rather than on
three separate offline devices. The owner explicitly delegated generation in
the same session ("i give you power"). The 2-of-3 threshold therefore
protects against operational mistakes, not against compromise of this
machine; a successor-policy ceremony with separated custody can rotate this
later per the runbook's rotation rules.

This file is ceremony evidence and travels with the policy into
`release/contract-activations/` at the activating release commit.
