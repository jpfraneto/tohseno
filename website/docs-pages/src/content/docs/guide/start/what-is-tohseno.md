---
title: What is Menlo?
description: A permissionless network for people to distribute iOS apps through source, their own Macs, and their own Apple identities.
---

Menlo lets people share iOS apps directly with other people. A maker publishes an exact source release and shares its app page. A recipient verifies that source, builds it with Xcode on their own Mac, signs with their own Apple identity, and installs on their intended iPhone.

The person-to-person path does not require App Store submission. Apple's signing, provisioning, Trust, Developer Mode, and operating-system security requirements still apply. This is source-based distribution, not a one-tap website installation or an Apple endorsement.

## Menlo and Tohseno

Menlo is the product name and visual identity. The existing CLI is still `tohseno`, installed with `npm i -g tohseno`. Protocol names, contracts, bundle identifiers, storage paths, and `tohseno.com` URLs retain their existing names. Use `tohseno init` and `tohseno deploy`; there is no `menlo` CLI command.

## Publish, share, receive

1. Bring an existing iOS app and set up Companion on your iPhone.
2. Run `tohseno init` from the project, then `tohseno deploy`.
3. Approve the exact source release on Companion. Its first publication is one **Ship**; later releases are **Updates**.
4. Share the canonical app link. A recipient encounters an exact release through Companion and prepares it on their Mac.
5. The recipient independently verifies, builds, signs, and installs for their own device.

A Claim records an encounter with an exact release. It is not installation evidence, a purchase, or a guarantee that the source is safe.

## Your workshop stays connected

The Mac keeps source and build tools. Companion carries private requests and holds the non-exportable Builder DeviceKey. You can adopt an existing Xcode project without moving it, create a new app with a coding agent, or evolve an app through the same local factory. Private creation and evolution do not publish automatically.

## Upload funding

One upload per Builder is sponsored. Later uploads require ETH funding, including Updates. The subsidy cap is live; the paid-wallet and copyable funding-address interface are unfinished. Do not use a BuilderAccount identity address as a deposit wallet: the current contract cannot receive or spend ETH.

Next: [requirements](/guide/start/requirements/) and [setup](/guide/start/install-and-onboard/).
