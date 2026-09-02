---
title: Evolve an app
description: Request one concrete change against the exact accepted app state and carry it back to the iPhone.
---

Use the app first. Pick one real point of friction, then describe the current and desired behavior.

> The breathing circle stops without a signal. When the timer ends, give one soft haptic pulse and show “Done” for two seconds.

## Send from Mac or Companion

Open the app and choose **What should change?** on the Mac, or **Evolve App** in the paired Companion. A request may contain text, on-device speech transcription, and up to eight PNG/JPEG references.

The signed Companion payload binds the stable project or Shot identity, current private source-state token or exact accepted base, request bytes, attachment blob references, originating device, timestamp, and optional follow-up relationship.

## Exact-base behavior

The base is selected by the product when you open or submit the composer. You do not choose a Version number. If another accepted change advances the app before admission, the request is rejected as stale. Tohseno never silently rebases it onto different source.

## Bounded implementation

The configured harness receives the exact request, references, repository instructions, current observation, and safety constraints. It is told to inspect first, preserve unrelated work, avoid destructive Git, and never commit, push, publish, or deploy implicitly.

The transition permits one implementation invocation and at most one targeted repair for a concrete code or build defect. Both share one wall-clock budget. Device, signing, provisioning, network, lineage, and protocol conditions never trigger another intelligence pass.

## Acceptance

After the harness exits successfully, Tohseno still requires a real Xcode build and signature verification. For a physical delivery, it installs through `xcrun devicectl`, then queries the phone's application inventory for the exact bundle identifier. Only that final observation is **Installed**.

If the verified artifact is waiting for an unlocked, trusted, unique phone, status is **Ready to install**. Reconnecting the phone resumes from the artifact instead of rewriting the app.

Next: understand the [product mental model](/guide/product/mental-model/) or the full [command lifecycle](/guide/architecture/command-lifecycle/).
