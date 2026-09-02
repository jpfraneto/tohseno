---
title: Identities and keys
description: BuilderID, DeviceKey, recovery, InstallationKey, ShotID, ExpressionID, and the identities that must stay distinct.
---

## Protocol authorities

| Identity | Definition | Purpose |
| --- | --- | --- |
| BuilderID | `eip155:4663:` plus the 20-byte BuilderAccount address | Stable chain-scoped Builder controller |
| Builder DeviceKey | Authorized P-256 public key; key ID is `Keccak-256(x32 || y32)` | Sign bounded Builder actions |
| Builder recovery | BIP-39/BIP-44 secp256k1 authority at `m/44'/60'/0'/0/0` | Separate recovery path |
| InstallationKey | App-installation-scoped P-256 key; ID is `SHA-256("TOHSENO-INSTALLATION-ID-V1\0" || x32 || y32)` | App-local continuity |

Apple development/distribution certificates are a fifth external identity. Private Companion pairing keys and workspace keys are another private transport system. None may be substituted for another.

Private device or installation key material must never enter records, logs, bundles, reports, or pairing requests.

## Object identities

**ShotID** is 32 cryptographically random bytes created once. It is never derived from a name, path, bundle, Builder, token, handle, server, or content.

**ExpressionID** is a random stable 32-byte identity independent of expression names and platforms.

**VersionID** binds one expression state:

```text
SHA-256(
  "TOHSENO-VERSION-ID-V2\0" ||
  ShotID32 || ExpressionID32 || u64be(ordinal) ||
  genome_digest32 || source_digest32
)
```

An adopted living project also has a private random `project_<uuid>` used by the product. It is not a ShotID or protocol digest.

## BuilderAccount prediction

The initial DeviceKey deterministically predicts the chain-4663 BuilderAccount through CREATE2. The salt is `SHA-256("TOHSENO-BUILDER-SALT-V1\0" || device_key_id)`, and the address depends on the exact factory, creation bytecode, salt, and P-256 coordinates.

Prediction is arithmetic, not deployment or authorization evidence. Public clients still verify the signed active generation, live code and state, current DeviceKey authority, exact action, and receipt.

## Rotation and recovery

Generation 0.8 uses distinct `ChangeRecovery`, `InitiateRecovery`, and `CancelRecovery` actions. Recovery initiation has a three-day delay; an active device admin can cancel before finalization. Permissionless finalization succeeds only for the exact pending digest after the delay and installs one all-permissions key in a new device epoch.

Frozen v0.7 local verification accepts only the initial DeviceKey. It does not pretend incomplete historical rotation/recovery evidence is valid.
