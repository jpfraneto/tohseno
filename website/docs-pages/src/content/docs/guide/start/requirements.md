---
title: Requirements
description: What you need before Tohseno can build and install a native iPhone app.
---

## For the normal path

You need:

1. A Mac that can run the published Tohseno release. The current intended baseline is macOS 14 or newer.
2. Full Xcode, opened at least once so its license and additional components are complete.
3. An Apple Account added in **Xcode → Settings → Accounts** and a usable Personal Team or signing team.
4. An authenticated coding agent that Tohseno can detect. Codex is one supported example.
5. An iPhone, a data-capable cable, and the ability to unlock the phone.
6. Trust between the iPhone and Mac, plus Developer Mode on the iPhone.

Apple credentials belong only in Xcode. Tohseno observes whether signing is ready; it does not ask for or store the Apple Account password.

## For an existing app

The project must have an iOS application target in a readable `.xcodeproj` or `.xcworkspace`, with at least one discoverable shared or visible scheme. If multiple schemes remain plausible, Tohseno asks you to choose; it does not guess past ambiguity.

The primary adopted-project path is designed to preserve real owner work, including a dirty Git working tree. Tohseno records the dirty paths it saw before work starts and does not run a broad rollback afterward.

## For Companion

The onboarding path builds, signs, installs, and launches the real Tohseno Companion, then waits for pairing proof. The phone must be reachable, unlocked, trusted, and in Developer Mode. Pairing invitations are one-use and expire after two minutes.

## A coding agent is necessary; a subscription is not

Local or bring-your-own execution is not subscription-gated. The agent itself must already be installed and authenticated according to its own product. Managed inference is a separate optional route with explicit consent, pricing/cap checks, and an available balance; it is not a requirement for the local path.

## Time

The tutorial is ten minutes of instruction. Installing Xcode, accepting Apple-controlled setup, downloading a release, the first implementation, code signing, and a physical install take additional time. A full bounded implementation transition has one shared wall-clock budget; the UI should report real progress rather than pretending the tutorial duration is the build duration.

Next: [install and onboard](/guide/start/install-and-onboard/).
