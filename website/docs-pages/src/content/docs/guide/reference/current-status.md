---
title: Current status
description: What is deployed as Menlo, what the upload subsidy covers, and what remains unfinished.
---

> Checked September 9, 2026 against production responses and the recorded release evidence. Live availability may change; use the linked status endpoints for the current response.

## Menlo is deployed

The [public website](https://tohseno.com) uses the Menlo identity and explains permissionless iOS source distribution. It displays real verified Registry entries. The Mac and Companion have the new paper, ink, and forest-green appearance; the public Mac candidate includes the updated Companion source. CLI commands, bundle identifiers, protocol names, and existing domains retain Tohseno identifiers.

## Mac download

[Download Menlo for Mac](https://tohseno.com/download/macos). Production serves **1.2.1-rc.1**, build **10008**, on the explicitly labeled release-candidate channel.

The universal app was built from native source commit `9bd350d221b9cb04e632d9d40045d83adc87a3de`. App and DMG were Developer ID signed, notarized, stapled, and accepted by Gatekeeper. The public website download matched the published artifact byte-for-byte:

```text
94a2a9a6ac0d6765a8127a169a1c7037df4822a55a310bc6fdbf2d0ad5002dd7
```

[Live download metadata](https://tohseno.com/api/distribution/v1/macos) · [Release and artifact](https://github.com/jpfraneto/tohseno/releases/tag/v1.2.1-rc.1)

This is not a stable promotion or a claim of clean-Mac or physical-iPhone acceptance of the new candidate.

## Public network

Production [Registry status](https://tohseno.com/api/registry/v1/status) reports generation **0.8.0**, chain ID **4663**, and the constrained Registry relayer available. One real public app card was observed during the Menlo deployment check.

Production [Claims status](https://tohseno.com/api/registry/v1/claims/status) reports activation and contract code verified, indexing enabled, and its relayer enabled and funded. This replaces the old August snapshot that reported disabled writes. Those service responses do not prove a recipient's physical Claim, local build, or installation.

Claim semantics and the deployed `TohsenoClaimsV1` address remain unchanged:

```text
0x5012703d48d99224ac0035d58bc373de9e8b1934
```

## Upload subsidy and ETH funding

**One upload per Builder is sponsored.** The reservation is durable and exclusive; concurrent publication jobs cannot each receive a free upload. Retries of the same job reuse its reservation. Existing catalog uploads count toward the allowance. The reservation is not an unlimited free retry across newly created jobs.

Later uploads, including Updates, require ETH funding. **The paid-wallet setup and funding-address Copy interface are not implemented yet.** Those uploads currently stop before additional sponsored Registry gas is spent and report that funding setup is unavailable.

The BuilderAccount identity contract cannot receive or spend ETH. No funding address is advertised while a usable wallet mechanism is absent. This upload limit does not change Claim fees, Claim editions, private local creation, or local/BYO execution.

## Remaining evidence

- Paid-wallet funding, a real address with Copy in the native apps, and a paid second upload.
- Browser verification of the new responsive surfaces; no browser was connected during the initial rebrand checks.
- Clean-Mac acceptance of the exact new candidate and physical Companion/iPhone acceptance.
- A second person's complete receive, verify, build, sign, install, and subsequent Update path.

Local tests and fixture renders support implementation claims; they do not substitute for those observations. See the [Menlo evidence record](https://github.com/jpfraneto/tohseno/blob/main/docs/MENLO_REBRAND.md) and [repository state](https://github.com/jpfraneto/tohseno/blob/main/docs/STATE.md).
