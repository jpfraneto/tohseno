---
title: Commitments and signatures
description: Exact input, source-tree, Fascia, canonical JSON, digest, and compact P-256 laws.
---

## Canonical JSON

Record, payload, action, checkpoint, and continuity commitments use SHA-256 over RFC 8785 canonical JSON where specified. Duplicate keys must be rejected before canonicalization.

A P-256 sidecar signs the declared 32-byte digest as a prehash exactly once. Implementations must not hash that digest again inside the signature operation. Public coordinates and `r`/`s` scalars are fixed-width 32-byte big-endian values; the verifier checks curve membership, nonzero scalars, and low-s.

Compact on-chain P-256 signatures are exactly 129 bytes:

```text
0x01 || x32 || y32 || r32 || s32
```

## Genesis input

Prompt bytes are committed exactly—no Unicode, newline, or whitespace normalization. Image entries use safe NFC filenames and SHA-256 of raw bytes, sort by unsigned UTF-8 filename bytes, and enter a domain-separated binary stream with fixed big-endian lengths.

The domain begins:

```text
"TOHSENO-GENESIS-INPUT-V1\0"
```

## Source tree

The hasher receives one explicit root and never consults Git, ignore files, current directory, environment, or global state. It includes regular files as raw bytes, rejects symlinks and non-regular entries, NFC-normalizes `/`-separated relative paths, rejects normalized and Apple-case collisions, sorts paths by unsigned UTF-8, and commits path length, path, and file digest under:

```text
"TOHSENO-SOURCE-TREE-V1\0"
```

Only the exact self-referential embedded record files are excluded. VCS, build, user-local, log, signing-secret, and environment paths are forbidden inside the source root rather than silently ignored.

## Fascia tree

The reusable Apple Fascia commitment is separate from the app source commitment. It hashes each included path length, path, content length, and raw content in order. Its historical law deliberately has no domain prefix or file count. `.build`, `.swiftpm`, and `Package.resolved` are excluded; symlinks and special files fail.

## Commitments prove integrity, not meaning alone

A matching digest proves that bytes match a declared law. It does not by itself prove key authority, a deployed contract, a canonical receipt, correct Apple signing, physical installation, or public availability. Conformance composes the applicable observations.
