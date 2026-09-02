---
title: Install and onboard
description: Install the verified Mac release, prepare Xcode and the phone, then pair Companion.
---

## 1. Use the published Mac download

Open [tohseno.com](https://tohseno.com) on the Mac. Use the download only when the site presents an activated release or explicitly labeled release candidate. The site is required to point to one immutable HTTPS DMG and exact SHA-256 digest.

Open the DMG, drag **Tohseno** to **Applications**, and open it from there. Do not disable Gatekeeper or substitute an unverified copy. If the route is unavailable, the relevant release gate is closed.

## 2. Let the readiness screen tell the truth

Tohseno checks the concrete prerequisites and shows the smallest next action:

- finish Xcode installation or accept its license;
- add an Apple Account inside Xcode;
- install or authenticate a supported coding agent;
- connect and unlock exactly one iPhone;
- tap **Trust This Computer** on the phone;
- enable **Settings → Privacy & Security → Developer Mode**;
- disconnect extra iPhones when selection is ambiguous.

It must not paint a ready state over a missing tool, missing identity, locked phone, or multiple-device ambiguity.

## 3. Install the real Companion

The primary onboarding path builds, signs, installs, and launches the actual Tohseno Companion on the connected iPhone. It does not substitute the disposable compatibility readiness app. If Apple needs a manual account or trust action, complete it in Apple's UI and return.

## 4. Pair

The Mac displays a one-use QR invitation valid for two minutes. In Companion, scan it. The phone proves possession of its signing and agreement keys; the Mac grants scoped authority and publishes an authenticated encrypted workspace snapshot. Pairing is complete only after both sides have that proof.

Phone keys live in iOS Keychain. The Mac workspace identity lives in macOS Keychain. The invitation, a QR scan, or relay reachability alone is not pairing.

## 5. Confirm presence

Tohseno remains available as a normal Mac app and a menu-bar item. Settings can list, rename, revoke, or begin pairing additional personal Companion devices.

Next: [create a new app](/guide/start/create-an-app/) or [adopt an existing app](/guide/start/adopt-an-app/).
