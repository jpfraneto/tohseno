---
title: States and errors
description: The six human states, underlying execution mapping, public lifecycle states, and failure categories.
---

## Human app states

All primary surfaces share `tohseno.presentation-projection/1`.

| Presented state | Meaning | Typical internal states |
| --- | --- | --- |
| Waiting | The durable request has not begun expensive work | `queued` |
| Building | Source/harness and deterministic checks are active | historical `planning`/`conception`, `materializing`, `building`, `testing`, `verifying`, `repairing` |
| Ready for phone | A verified candidate exists but the device gate is open | `waiting_for_device` |
| Installing | The device install/launch boundary is active | `installing`, `launching` |
| Installed | An accepted Version exists after required delivery evidence | `accepted` |
| Failed | Work stopped or was cancelled with recovery text | `failed`, `cancelled` |

Historical internal names remain readable for compatibility. They do not restore removed product phases. New births have no Conception planning round.

## Command states

Private commands move through durable received, validated, accepted/prepared, running or waiting-for-device, and terminal completion/rejection/failure records. A command receipt means durable Mac admission, not accepted software. Retry returns the stable command result when exact bytes match.

## Public lifecycle

| State | Canonical condition |
| --- | --- |
| Private candidate | No accepted public registration for the Shot |
| Shipped | Canonical first Registry registration and catalog/source agreement |
| Updated | Canonical later checkpoint and matching release agreement |
| Claiming | Exact transaction job pending; not yet a Claim |
| Claimed | Canonical `SoftwareClaimed` evidence and token number |
| Claim edition closed | Immutable supply/time boundary reached canonically |
| Preparing | Exact claimed/selected release is durably queued or building on Mac |
| Installed | Local `devicectl` success plus exact bundle inventory observation |

## Error families

- **Input:** empty/oversized intention, invalid image, unsafe name/path, duplicate command bytes.
- **Authority:** bad/revoked signature, insufficient capability, replay, wrong DeviceKey, invalid recovery state.
- **Base:** unknown app/Shot/Expression/Version or stale accepted base.
- **Harness:** unavailable route, authentication, timeout, no source progress, implementation failure.
- **Source/build:** unsafe mutation, compile/test/verification failure, unsupported network source.
- **Apple:** team/signing/provisioning, locked/untrusted device, Developer Mode, device ambiguity, install/inventory failure.
- **Public evidence:** wrong activation/runtime/chain, stale head/nonce/deadline, receipt mismatch, manifest/source mismatch.
- **Operations:** relay capacity/retention, durable-store version/corruption, provider reservation, relayer disabled.

Errors preserve the exact last durable fact and should name one smallest next action without exposing private content.
