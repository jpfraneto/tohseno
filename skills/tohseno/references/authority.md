# Who authorizes what, and why

The skill is instructions for an agent, not a credential or a new authority.
Use the installed CLI's existing service authentication. Do not extract its
runtime tokens or impersonate the signed native Mac client.

## Your Mac

The Mac holds the source and runs the tools. The persistent local service owns
workspace state and durable commands. The agent can edit owner-authorized
source; it does not obtain publication authority from filesystem access.
Preserve the app-local `.tohseno/` boundary and unrelated working-tree changes.
Never blanket-ignore that directory or manually rewrite its protocol records.

## Your iPhone Companion

Companion recovery entropy is stored in its own non-synchronizing,
WhenUnlockedThisDeviceOnly Keychain item. Its BIP-39 phrase restores Companion
identity, not the non-exportable Builder DeviceKey or a revoked Mac capability;
restoration requires a new pairing. Generated apps must never receive it.

Pairing establishes the private Mac/phone association. Durable requests cross
signed, encrypted, idempotent mailboxes through a content-blind relay. The
Builder DeviceKey remains non-exportable on the phone. Public source release
requires the human to approve the exact action in Companion; neither this skill,
the Mac, nor the relay can substitute for that approval.

The relay carries messages, not Xcode builds or Apple installation traffic.
`https://companion.tohseno.com/healthz` reports relay readiness and push status.
A healthy relay proves neither current pairing nor successful command delivery.
Without APNs, foreground reconciliation can still work. The nearby Workshop
Session is separate ephemeral connectivity; a pulse is not installation or
publication evidence. Use `tohseno companion status`, `devices`, and
`relay-status` for supported inspection; avoid printing private capabilities.

## Apple signing and the destination

Each generated app also has an independent InstallationIdentity and explicit,
audience-scoped continuity. The requested new-app recovery default and its
implementation boundary are defined in [app-experience.md](app-experience.md).
Neither a common Builder nor a recovery phrase grants automatic cross-app access.

Apple identity is separate from the Tohseno Builder identity. Xcode signs the
local app using the owner's Apple setup. Trust, Developer Mode, account prompts,
entitlements, provisioning expiration, and device limits remain real.

Delivery must resolve the intended phone using the existing private association.
USB and Xcode local-network reachability are transports for that same target.
Do not substitute another visible device. A successful install plus an exact
bundle inventory check is required before saying Installed. If the phone is
absent or locked, retain the build and give the exact next action.

## Public provenance

The Registry witnesses public checkpoints. A signed catalog binds the exact
source bytes to the authorized release; recipients verify it and use their own
Mac and Apple identity. This supports person-to-person distribution without
App Store submission, while preserving Apple's signing boundary. Provenance is
not a promise that downloaded software is safe.

One Shot Ships once, then receives Updates. A Claim is a separate exact-release
human action and non-transferable receipt, not an install. Never synthesize
Claims, signatures, receipts, or chain state. Closed activation gates require
their actual authority, not a skill-level workaround.

For work inside the Tohseno repository, `protocol/` remains normative, accepted
ADRs govern architecture, and `docs/STATE.md` is a snapshot rather than live
operational evidence. This skill does not amend protocol or release authority.
