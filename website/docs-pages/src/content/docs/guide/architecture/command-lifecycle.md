---
title: Command lifecycle
description: The boundary-by-boundary path from a Companion tap to an accepted Version.
---

## 1. The phone makes a durable command

Companion copies the displayed app identity and exact base into the request. The SDK validates the grant and references, signs canonical command bytes with the phone identity, encrypts command and reference chunks to the Mac, and persists encrypted state and outbox files **before** reporting receipt to the UI.

Transport failure therefore means queued, not lost. Relaunch and foreground reconciliation retry the same semantic command.

## 2. The content-blind relay stores opaque envelopes

Reference envelopes upload before the command envelope. The relay validates mailbox capabilities, routing metadata, sizes, sequence watermarks, expiry, and capacity, then atomically stores ciphertext. Equal envelope ID and digest is idempotent; conflicting reuse or non-increasing sender sequence fails.

The relay acknowledgement proves storage, not command admission.

## 3. The Mac authenticates and admits

The service polls relay mailboxes even when no UI is open. It decrypts, verifies envelope and phone signatures, applies replay protection and revocation, reconstructs references, checks the capability grant, then rechecks the exact app and base.

Only then does opaque transport become authenticated private plaintext. Missing chunks, invalid signatures, revoked authority, replay, expiry, unknown app, or stale base reject without execution.

## 4. The application service prepares stable execution

`admit_with_files` writes `request.json`, canonical `payload.json`, exact `inputs/`, and atomic `status.json` under the command journal. After semantic validation, `prepare_for_command` writes the exact intent/reference package and execution record under the app before the status becomes running.

Recovery begins from these committed bytes, not from a client retry.

## 5. A detached runner earns acceptance

The runner claims the stable execution, takes the factory lease, invokes the bounded harness, and applies deterministic build, test, verification, recording, signing, installation, launch, and acceptance gates. One mutating runner can own an execution.

Harness success is not acceptance. Completion and the landed Version are reconciled independently before the command journal becomes terminal.

## Receipt order

The Mac publishes a signed command receipt only after durable application admission and stable execution preparation. It stores its local mailbox cursor before ACKing the relay. The phone deletes its outbound command only after verifying that Mac receipt.

Crashes at either edge produce idempotent replay—not duplicated semantic work.
