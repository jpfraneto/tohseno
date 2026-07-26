# Signer

The signer interface accepts opaque bytes. Signatures bind the identity,
suite, key ID, public key, and message under a package domain. Verifier sets
allow future suites without weakening existing validation.

`LocalEd25519Signer` keeps an OS-generated private key only in memory and is
for tests and local protocol exercises. This package deliberately provides no
key persistence, production custody, recovery, Apple signing, or external
action authority.
