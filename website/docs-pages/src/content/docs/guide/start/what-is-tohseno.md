---
title: What is Tohseno?
description: Tohseno keeps a native iPhone app connected to its exact Mac source and its next change.
---

Tohseno is a native Mac-and-iPhone system for living software. It takes a plain-language intention, works through one configured coding agent, checks the result with Xcode, and carries the verified app to your iPhone. Later, you can describe one change and send it through the same path.

Its primary job is continuity: the app in your hand remains connected to the source, accepted history, exact starting state, and machinery that can change it again.

## Two ways in

**Adopt Existing App** is the primary path. Choose an existing `.xcodeproj` or `.xcworkspace`. Tohseno inspects it, records an external private connection, performs a real Simulator build, and leaves the repository untouched.

**Create a First App** is the retained factory path. Write what the app should do, optionally attach up to eight PNG or JPEG references, and send. The factory creates an ordinary local Git repository and builds the app for your phone.

Both paths lead to the same product loop:

```text
App → describe one change → source → checks → App on your iPhone
```

## What Tohseno is responsible for

- Preserving the exact request and the state it was made against.
- Running one configured implementation harness within a bounded transition.
- Separating pre-existing work from files observed after the request.
- Performing deterministic build, signature, device, install, and inventory checks.
- Keeping commands and status durable across app or service restarts.
- Pairing Companion devices with scoped cryptographic authority.
- Keeping creation, publication, Claim, Ship, and Update as separate acts.

## What it is not

Tohseno is not a prompt wrapper that treats generated files as success. It is not an App Store, a wallet-connect product, a generic cloud IDE, a source-hosting shortcut, or an automatic publisher. It does not collect your Apple password. It does not turn a relay record into a Shot, a build into an install, or a button press into a public Claim.

The small cast woven through these docs keeps that system legible: Mac holds the local workshop, Orbit represents the coding agent, Tink keeps the job coherent, Companion carries private intent, Tick watches truthful state, Hearth guards the public boundary, and Ione is where the app must finally become real.

Next: [requirements](/guide/start/requirements/).
