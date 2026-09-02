---
title: Adopt an existing app
description: Connect a real Xcode project to Tohseno without rewriting its repository.
---

Adoption is Tohseno's primary entry point.

## Choose one Xcode container

Select **Adopt Existing App**, then choose an exact `.xcodeproj` or `.xcworkspace`. Tohseno lists schemes and probes application build settings with `xcodebuild`. If it cannot choose one app scheme without ambiguity, it asks you.

## What Tohseno records

The private adopted-project record includes:

- display and product name;
- bundle identifier;
- container, source root, and scheme;
- iOS deployment target and non-secret signing-team setting;
- Git revision and paths already dirty at adoption;
- bounded repository instructions such as `AGENTS.md`, `CLAUDE.md`, `MASTER_PROMPT.md`, README, and existing Tohseno metadata;
- a real unsigned Simulator build result;
- whether the exact bundle is already installed when exactly one phone is reachable.

The stable identity is a random `project_<uuid>`. It is not a protocol digest and is not derived only from a path. Choosing the same canonical container, scheme, and bundle identifier again preserves that identity.

## What adoption does not do

It does not move files, add `.tohseno`, initialize Git, commit, push, publish, change project settings, or clean the working tree. The private record lives outside the repository under the service's `living-projects-v1` store.

If the source folder later moves, the record keeps its former path and reports source unavailable. Automatic relinking is not implemented. Deleting and recreating the record would fabricate continuity, so the product does not present that as a repair.

## Validate the connection

Wait for the Simulator probe to finish. A passed probe means Xcode could compile the selected scheme for a simulator without code signing. It does not prove physical signing or installation. Open the project workspace and confirm the source path, scheme, bundle identifier, and existing dirty paths look right before sending an evolution.

Next: [evolve the connected app](/guide/start/evolve-an-app/).
