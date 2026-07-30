# Contract activations

This directory intentionally contains no production activation record and no
release-authority trust root.

`contracts/generations/0.8.0/generation.json` is an immutable build definition.
It proves canonical source/compiler/artifact facts and conditional CREATE2
arithmetic only. It does not prove that the deployer exists on Robinhood Chain,
that either predicted address has code, that the observed code is trusted, or
that the generation is active.

The closed `tohseno.contract-activation/1`,
`tohseno.release-authority-policy/1`, and
`tohseno.signed-contract-activation/1` formats define the minimum evidence and
threshold-signature laws. They bind the protocol major, chain, immutable
generation-definition digest, authority-policy digest, exact observed factory
and registry addresses and runtime code hashes, canonical activation block and
block hash, transaction evidence, fresh actual-target EIP-7951 probe digest,
predecessor activation, and replay-protected activation sequence. Signatures
come from a dedicated offline release-authority policy—not a Builder DeviceKey,
Shot owner, installation identity, relayer, or deployer merely because it
broadcast a transaction.

Those neutral formats prove only a threshold under a supplied policy. They do
not decide that the policy is trusted, and no instance is committed here.

Until that policy, its trust root, and a real signed activation instance are
committed, clients must resolve generation 0.8.0 as inactive. Do not add mock
keys or placeholder activation coordinates here.
