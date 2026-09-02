---
title: Fail-closed rules
description: Conditions that stop Tohseno instead of guessing, bypassing, or manufacturing success.
---

| Condition | Required behavior |
| --- | --- |
| Stable DMG URL/digest unavailable | Keep download unavailable; do not bypass Gatekeeper |
| Xcode/account/license incomplete | Ask for the Apple-controlled action |
| Zero reachable phones | Keep verified artifact Ready to install |
| Multiple reachable phones | Refuse selection; ask to disconnect ambiguity |
| Phone locked, untrusted, Developer Mode off | Name the smallest action and wait |
| Coding harness missing or unauthenticated | Refuse implementation; never switch secretly |
| Command ID reused with different bytes | Conflict; never mutate the accepted command |
| Evolution base changed | Reject stale; never silently rebase |
| Harness stops after partial mutation | Record failure; do not equate files with acceptance |
| Whole-Mac restart loses an active harness runner | Fail rather than repeat intelligence on unknown state |
| Build succeeds but install is unverified | Never report Installed |
| Public archive has unsafe path/link/secret | Refuse publication or extraction |
| Source requires review | Wait for explicit review; do not auto-build |
| Unsupported capability | Refuse build and name the reason |
| Catalog, receipt, head, activation, runtime, or bytes disagree | Refuse Claim/Install/Fork/public verification |
| Pending transaction | Keep pending; never report Shipped, Updated, or Claimed |
| Claim head becomes stale or edition closes | Contract/action fails; no reservation fiction |
| Managed balance reservation fails | Do not call provider or spend |
| Schema has unknown/duplicate fields | Reject before semantic processing |

## Why the rules are strict

Most boundaries are irreversible or ambiguous: source mutation can overlap owner work; a public signature authorizes a permanent fact; a Claim receipt is non-transferable; a code signature identifies a local Builder; and a chain transaction cannot be reworded later.

Fail-closed behavior keeps a missing observation from becoming a false fact. It is not a generic “something went wrong” policy. Each refusal should preserve durable input and the last verified artifact when safe, then state the smallest recovery action.

## No bypass vocabulary

Product and operator surfaces may not invent an installation, Claim, receipt, activation, source review, physical test, or release acceptance. There are no administrator rows that substitute for cryptographic or physical evidence, no generic transaction relayer, and no release flag that overrides the required gates.
