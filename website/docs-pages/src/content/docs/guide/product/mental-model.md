---
title: Product mental model
description: The few product concepts a Tohseno user needs, and the technical concepts intentionally kept behind them.
---

The normal product is deliberately smaller than the machine underneath it.

## The surface

| Product word | Meaning |
| --- | --- |
| App | A native iPhone app connected to source and history on this Mac. |
| Create | Begin a new app from an intention. |
| Adopt | Connect an existing Xcode app without altering its repository. |
| Evolve | Apply one request to the exact current app state. |
| Build | The visible path from intent through source and Xcode to a verified app. |
| Ready to install | A verified artifact exists; the intended phone still needs an action or connection. |
| Installed | The exact bundle was observed in the physical phone's inventory after installation. |
| Ship | Make the first public release of one Shot. This happens once. |
| Update | Make a later public release of the same Shot. |
| Claim | Publicly record one Tohseno identity's encounter with one exact Shot release. |

The core abstraction is **App → Intent → App on your iPhone**. The person writes what should change; the system binds the exact base and handles the continuity work.

## The machinery behind the surface

Internally, Tohseno uses Commands, Expressions, Evolutions, Versions, Shots, Genomes, DeviceKeys, BuilderAccounts, checkpoints, receipts, and conformance reports. Those are necessary for durability and verification. They do not belong in the normal creation screen.

The old Studio execution dashboard, phase renderer, Feedback and Marketing forms, and manual exact-Version controls were deleted intentionally. Details and diagnostics may expose bounded technical facts, but the normal path does not ask a person to operate the engine.

## Four truths that never collapse

1. **Intent truth:** the exact bytes the person sent and the exact base they addressed.
2. **Execution truth:** source, build, signature, installation, and verification observed by the Mac.
3. **Authority truth:** a scoped Companion DeviceKey approved the exact public action.
4. **Public truth:** activated contracts, canonical receipts, Registry state, signed catalog, and exact source bytes agree.

Keeping these separate prevents attractive but false shortcuts. A relay acknowledgement is not an admitted command. A harness exit is not a build. A build is not an install. A catalog row is not a Registry fact. A pending Claim transaction is not Claimed.

## The working rhythm

Use the app. Notice one thing. Request one change. Let the same app return. Public release is optional and explicit; private evolution is the default.
