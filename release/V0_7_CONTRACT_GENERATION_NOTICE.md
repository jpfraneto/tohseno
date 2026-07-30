# v0.7 contract-generation notice

The Solidity contracts, ABIs, BuilderAccount creation bytecode, and predicted
contract addresses included in the TOHSENO v0.7.0 and v0.7.1 release artifacts
are an undeployed design that has been superseded after security review.

That v0.7 contract generation will never be deployed by the TOHSENO project.
Its predicted BuilderAccount addresses must not be treated as durable public
BuilderIDs, ownership evidence, or future deployment coordinates.

Private v0.7 Shot artifacts remain locally verifiable against the exact frozen
v0.7 inputs and immutable v0.7.1 tag/release archive. Main no longer rebuilds
that archive from changing sources, and its v0.7 deployment and release
commands fail closed. A future public BuilderID must use the finalized
successor contract generation. No signed identity-supersession flow is claimed
until such a migration is actually needed and implemented.

This text is the repository source for the notice that must also be added to
the already-published v0.7 release notes by a release operator. This repository
change does not edit or republish an existing external release.
