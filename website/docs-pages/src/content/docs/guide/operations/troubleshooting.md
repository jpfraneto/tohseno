---
title: Troubleshooting
description: Walk durable boundaries in order without losing source, replaying intelligence, or manufacturing success.
---

Start from the last durable fact, not the last screen animation.

## A Companion request is not moving

1. Confirm Companion persisted the request and shows queued/reconnecting rather than a local validation failure.
2. Confirm relay reachability and mailbox capability without printing ciphertext or secrets.
3. On the Mac, inspect paired-device/revocation state, relay cursor, envelope result and command receipt.
4. Inspect `~/.tohseno/service/command-journal/<command-id>/status.json`.
5. Inspect the stable app-local execution only after command admission exists.

A relay ACK is not Mac admission. Do not delete the phone outbox manually merely because ciphertext reached the server.

## A job says Waiting to build

Another source job may hold the factory lease. The waiting command is durable and starts automatically. A verified artifact waiting for a cable releases the lease, so a long wait can also indicate a live runner or failed service reconciliation.

Use the explicit service surface:

```text
tohseno service status
tohseno service logs
tohseno service restart
```

Logs are bounded and content-free. Do not add raw prompts, keys or reference bytes for convenience.

## Ready to install does not advance

Check, in order:

1. exactly one intended physical iPhone is connected;
2. it is unlocked;
3. the phone trusts the Mac;
4. Developer Mode is enabled;
5. Xcode still has an appropriate account/team;
6. the retained `.app` signature verifies;
7. `devicectl` can see the device and query the exact bundle.

Do not rerun the coding harness for a device-only failure.

## An evolution is stale

Another accepted change advanced the exact base. The refusal is correct. Reopen the current app, read the current state, and submit a new explicit request. Do not edit the journal or force the old command onto new source.

## Source is partially changed after failure

For adopted repositories, Tohseno intentionally has no general rollback because pre-existing owner work may be present. Inspect the recorded baseline and changed-file observation. Decide manually what to keep. Avoid destructive Git commands.

## Public Install, Fork, Ship, or Claim fails

Walk activation → runtime/constructor state → current Builder authority → canonical receipt/block → current Registry head → signed manifest → source bytes → safety classification → local Apple build. For Claim also check edition policy, account/Shot uniqueness, nonce, deadline, exact current checkpoint and constrained relayer availability.

Never patch an index row to make a public action look canonical.
