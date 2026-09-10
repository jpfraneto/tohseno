---
title: Requirements
description: What you need before Menlo can build and install a native iPhone app.
---

## For the normal path

You need:

1. A Mac that can run the published Menlo release. The current intended baseline is macOS 14 or newer.
2. Full Xcode, opened at least once so its license and additional components are complete.
3. An Apple Account added in **Xcode → Settings → Accounts** and a usable Personal Team or signing team.
4. For creating or evolving code through the factory, an installed and authenticated coding agent. Sharing existing source and receiving an exact release use the distribution path.
5. An iPhone, a data-capable cable, and the ability to unlock the phone.
6. Trust between the iPhone and Mac, plus Developer Mode on the iPhone.

Apple credentials belong only in Xcode. Menlo observes whether signing is ready; it does not ask for or store the Apple Account password.

## For an existing app

The project must have an iOS application target in a readable `.xcodeproj` or `.xcworkspace`, with at least one discoverable shared or visible scheme. If multiple schemes remain plausible, Menlo asks you to choose; it does not guess past ambiguity.

The primary adopted-project path is designed to preserve real owner work, including a dirty Git working tree. Menlo records the dirty paths it saw before work starts and does not run a broad rollback afterward.

## For Companion

The onboarding path builds, signs, installs, and launches the real Menlo Companion, then waits for pairing proof. The phone must be reachable, unlocked, trusted, and in Developer Mode. Pairing invitations are one-use and expire after two minutes.

## Coding assistance and upload costs are separate

Local or bring-your-own execution is not subscription-gated. The agent itself must already be installed and authenticated according to its own product. Managed inference is a separate optional route with explicit consent, pricing/cap checks, and an available balance; it is not a requirement for the local path.

## Upload allowance

One upload per Builder is sponsored. Additional uploads, including Updates, require ETH funding. The cap is deployed; paid-wallet setup and the copyable funding address are not available yet. See [current status](/guide/reference/current-status/) before attempting another upload.

## Time

Xcode installation, Apple-controlled setup, code signing, and physical installation take real time. Menlo reports observed progress; a completed build is not proof of installation.

Next: [set up and share an app](/guide/start/install-and-onboard/).
