# Contract activations

This directory holds the production activation record for generation 0.8.0
and its release-authority trust root, established by the owner ceremony of
2026-08-02 (UTC).

- `release-authority-policy.json` — the 2-of-3
  `tohseno.release-authority-policy/1` instance. Its RFC 8785/SHA-256 digest,
  `0xf14410692ebe34f6855b8dbec5cb08733aa737f1cd86f385694e4fb575df943c`, is the
  owner-approved client trust root pinned in
  `engine/src/contract_generation.rs`.
- `OWNER_POLICY_APPROVAL.md` — the owner's verbatim digest approval and the
  recorded custody deviation (all three keys generated on the owner's Mac).
- `signed-contract-activation-1.json` — the
  `tohseno.signed-contract-activation/1` envelope: activation sequence 1,
  signing digest
  `0x2b640260595def403343810d0dc4ee231e1faff427581be4f7b40cff4c189d28`,
  approved by 2 of 3 authorities. It binds the exact generation definition
  digest, the policy digest, chain 4663, the observed deployment evidence for
  the factory and registry, the locally instantiated BuilderAccount runtime
  hash, activation block 25511561, and the fresh probe digest.
- `OWNER_CANARY_WAIVER.md` — the owner's explicit waiver of the 72-hour
  production canary before signing, with its consequences stated.
- `p256-probe-20260802T013802Z.json` — the fresh ceremony-bound EIP-7951
  probe evidence (exact 6,900 gas, positive/negative/infinity vectors) whose
  raw-byte SHA-256 the activation binds.
- `independent-verification-1.json` — the independent Python verifier report;
  the Rust implementation
  (`protocol/examples/verify_signed_contract_activation.rs`) reproduced the
  same digests before the trust root was pinned.

The formats remain neutral: they prove a threshold under a supplied policy.
Trust comes only from the engine's compiled-in pin of the policy digest, which
was changed in the same reviewed commit that added these instances. A
successor activation increments the sequence, names this activation's signing
digest as its predecessor, and lands here next to it.

Do not edit committed instances. A defect creates a successor generation or a
successor activation, never a mutation.
