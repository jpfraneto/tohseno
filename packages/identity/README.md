# Identity

This package defines the public `BUILDER` protocol role that authorizes Shot
records. Builder identity is not an app-runtime account, Apple credential,
wallet, or release-signing identity.

`deriveDeterministicTestIdentity` exists for fixtures and local tests. It
derives only a public identifier from non-secret input. It is not a production
identity, key-custody, or recovery design.
