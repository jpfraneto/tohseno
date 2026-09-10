---
title: Set up and share an app
description: Install the existing Tohseno tools, pair Menlo Companion, and publish an exact iOS source release.
---

## 1. Start on your Mac

You need macOS 14 or later, full Xcode, an Apple signing identity, and an intended iPhone. For an existing Xcode project, install the CLI:

```sh
npm i -g tohseno
cd ExistingApp
tohseno init
```

Menlo is the product name; the package and commands still use `tohseno`. The install has no postinstall download or GUI launch. `init` checks the intended iPhone for the real Companion and its private pairing before adopting the project. It does not restructure your repository.

## 2. Install and pair Companion

If Companion is missing, follow the CLI's instruction:

```sh
tohseno companion install
```

The Mac builds and signs the actual Companion for your intended iPhone. Keep it unlocked, complete Trust and Developer Mode in Apple's UI, and use a cable when Apple requires initial pairing. Later cable and supported local-network reachability are observed transports to the same intended phone; another visible phone is not substituted.

Scan the Mac's one-use pairing QR in Companion. Pairing completes only after the Mac accepts the phone's proof and publishes an authenticated workspace snapshot. A QR scan or reachable relay alone is not pairing. Apple credentials belong in Xcode; Menlo does not collect your Apple password.

## 3. Publish your source

From the adopted project:

```sh
tohseno deploy
```

Review and approve the exact public source on Companion. First publication is **Ship**; later public releases are **Updates**. The returned canonical app link is the distribution destination. An optional `--app-slug your-app` does not bypass the separate approval process for a short global alias.

Menlo sponsors one upload per Builder. Later uploads require ETH funding, but the paid-wallet setup is not ready yet; those uploads currently stop with a funding-required message. See [current status](/guide/reference/current-status/).

## Prefer the Mac application?

[Download Menlo for Mac](https://tohseno.com/download/macos). The production door serves an explicitly labeled release candidate pinned to an immutable HTTPS artifact and exact SHA-256. Open the DMG, drag the included app into **Applications**, then open it through Finder. The bundle retains its technical `Tohseno.app` filename while the interface is Menlo.

The native readiness screen walks through Xcode, Apple signing, the intended iPhone, Companion installation, and pairing. Complete any Apple-controlled action in Apple's own UI. Do not disable Gatekeeper or substitute an unverified artifact.

## Receive an app

Explore [the Registry](https://tohseno.com/registry) and open an exact release in Companion. A canonical Claim can queue that release for your Mac; installation remains separate. Your Mac verifies the exact source, builds with Xcode, signs with your own Apple identity, and installs on your intended phone.

Next: [Ship, Claim, and Update](/guide/product/ship-claim-update/) or [the Mac workshop](/guide/product/mac-app/).
