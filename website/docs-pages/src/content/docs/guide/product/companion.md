---
title: Tohseno Companion
description: The iPhone product, private remote control, and holder of human authorization.
---

Companion presents its two roles as the pocket side of the same Living
Workshop. The Mac factory is visibly remote, this iPhone is the keeper, app
objects sit on one shelf, and Network, Updates, and Keeper are truthful
destinations rather than a claim that the phone runs a second factory.

## Private connection to the Mac

Companion shows the apps connected to a paired Mac, their bounded status and history, and **Evolve App**. A person can type, use native speech transcription, and attach up to eight images. The SDK persists the signed command and encrypted outbox bytes before send returns, so an offline phone or sleeping Mac does not erase the request.

**One Shot** opens the existing creation route. The app name remains optional;
the exact intention and reference bounds are unchanged. Mac-offline state is
shown explicitly and never presented as a failed or completed build.

The relay is content-blind. It transports opaque mailboxes and ciphertext but has no project authority, content key, source, or Apple signing identity. Delivery, admission, and execution are distinct receipts.

## Human authority for public actions

Companion holds the Builder DeviceKey: a protocol-compatible P-256 key whose private scalar stays in the iPhone's strongest compatible non-exportable, this-device-only Keychain or Secure Enclave mechanism. It signs already computed 32-byte protocol digests exactly once and normalizes signatures to low-s form.

Before Ship, Update, profile, alias, or Claim authorization, Companion receives the complete structured action, recomputes its canonical digest, validates closed fields and active-generation facts, presents a bounded human summary, and asks for explicit approval. It never signs an opaque digest supplied by the Mac or server.

## Pairing and revocation

Initial pairing uses a signed, encrypted, one-use invitation that expires after two minutes. Pairing completes only after the Mac accepts the phone's proof and publishes an authenticated workspace snapshot.

Settings on the Mac can rename and revoke paired devices. Revocation changes local authority first, increments its generation, and revokes both relay mailboxes. Future signed commands from that phone fail admission even if stale ciphertext remains in transport.

## Claim ritual

When Claims is operationally enabled, Companion centers the exact artifact and asks the person to draw one forgiving circle. The completed stroke is normalized, arc-length resampled to exactly 64 points, quantized into `tohseno.claim-mark/1`, and SHA-256 committed. Timing, force, motion, and behavioral biometrics are not retained. An accessible hold gesture emits a distinct canonical accessibility mark; it does not fabricate handwriting.

A completed gesture is still not a Claim. Only canonical on-chain mint evidence changes the state to **Claimed**.
