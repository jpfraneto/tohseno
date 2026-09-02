---
title: Source and build safety
description: How Tohseno snapshots, scans, classifies, extracts, and builds source from another person.
---

Public source is executable input. Inspectable does not mean safe.

## Deterministic snapshot

The Builder's Mac copies source into a new owner-only temporary root without changing the original. Paths are NFC-normalized, `/` separated, sorted by unsigned UTF-8 bytes, collision-checked, relative and bounded. Files are copied as exact bytes with deterministic archive metadata.

Rejected inputs include:

- symlinks, hard links and special files;
- absolute paths, traversal and archive ambiguity;
- normalized or Apple-case collisions;
- oversized files, path counts or total trees;
- VCS internals, build output, DerivedData, caches and user data;
- environment files, logs and private Tohseno state;
- Apple signing/provisioning material and known secret paths;
- high-confidence detected secrets.

Failure reports name paths without printing secret contents. `.gitignore` is never the security policy.

## Recipient extraction

The recipient downloads by the official URL resolved from the verified catalog, checks byte length and SHA-256, extracts into a new safe root, re-applies path rules, and recomputes the source-tree commitment. Existing directories are not merged blindly.

## Build classification

**Green** permits automatic build only for a narrow ordinary iOS app with pinned dependencies and no arbitrary Run Script phase, custom executable, unsafe build rule or package/compiler plugin, unsupported entitlement, or unsafe archive structure.

**Review** keeps source visible and requires **I Reviewed the Source — Build** before any `xcodebuild` invocation. Reasons are explicit.

**Unsupported** never builds in the current product.

## Local signing does not sanitize code

Using the recipient's own Xcode identity preserves Apple's signing boundary; it does not make malicious source benign. The source classifier, human review, dependency pins, entitlement restrictions, and safe extraction happen first.

Install and Fork share verification but produce different local identities. A Fork creates a new child identity and preserves one exact parent-release relation; it does not grant the parent's Builder authority.
