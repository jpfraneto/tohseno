---
title: Apple build and delivery
description: Simulator probes, physical builds, code signing, CoreDevice selection, installation, and final verification.
---

Apple's security boundary remains intact. Tohseno automates around it; it does not remove it.

## Adoption probe

An adopted project first receives a real unsigned Simulator build for the selected container and scheme. This proves a specific compilation path without using a signing identity. It does not prove that a physical-device build, entitlement set, provisioning profile, or installation will work.

## Physical candidate

After source work succeeds, Tohseno runs a real signed `iphoneos` `xcodebuild`, locates the resulting `.app`, and verifies the code signature with `codesign`. Build and signing failures have separate recorded categories.

For generated Shots, deterministic gates also cover the source-tree and Fascia commitments, required target membership, declared capabilities, bundle identity, build number, dependencies, storage/network declarations, and embedded provenance.

## Device resolution

The system selects a physical target only when exactly one reachable CoreDevice iPhone is eligible. Zero devices becomes a truthful waiting state. More than one fails closed instead of selecting the first enumerated phone.

The person may need to:

- connect the intended phone with a data cable;
- unlock it;
- tap **Trust This Computer**;
- enable Developer Mode and accept a restart;
- disconnect another reachable iPhone.

These are Apple-controlled actions. Apple credentials are entered only in Xcode.

## Installation truth

Tohseno invokes:

```text
xcrun devicectl device install app …
```

but a zero exit is not the final fact. It then queries:

```text
xcrun devicectl device info apps --bundle-id …
```

The exact bundle must appear in the intended device inventory before status is **Installed**. A verified build waiting for the phone is retained as **Ready to install**, and delivery retries without rerunning the coding harness.

## Recipient builds from the network

A recipient independently verifies the public release, safely extracts source, and builds using their own Xcode development team. Tohseno may derive a stable recipient-local bundle namespace through build-setting overrides when the original identifier cannot be registered; it does not silently rewrite downloaded source. Unsupported capabilities fail with an exact reason.

Provisioning expiration remains visible. Refresh rebuilds and signs the same verified release with no AI call and no new Registry checkpoint.
