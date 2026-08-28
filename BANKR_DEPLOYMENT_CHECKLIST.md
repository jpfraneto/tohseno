# BANKR DEPLOYMENT CHECKLIST — $TOHSENO

> **Retired historical token-launch checklist.** The referenced Studio launch
> implementation no longer exists, and this file authorizes no simulation or
> broadcast. ADR 0025's managed-compute Bankr integration is a separate,
> server-only, least-privilege LLM Gateway path; it cannot launch tokens or use
> a wallet. Follow `docs/runbooks/MANAGED_COMPUTE.md` for that service. Keep the
> material below only as history and do not follow its live-run steps.

The prose below described a removed implementation and is not enforced by the
current tree.

Current verified state on 2026-08-01: the Keychain entry is a Bankr user key,
and a configured Studio reports `configured: true`. The `tohseno` Shot is
conformant protocol v2 and has no Token Association. That is the correct
pre-simulation state. There is no process-level deploy lock: the
deployment ceremony itself — fresh single-use simulation, acknowledgment, and
the exact typed phrase — is the only thing between a simulation and a
broadcast.

---

## 0 · What will actually happen

- **Signer / deployer:** the Bankr wallet that owns your `BANKR_API_KEY`.
  Not jpfraneto.eth. Not this Mac. Studio only sends one HTTPS request to
  `https://api.bankr.bot/token-launches/deploy`.
- **Token identity:** derived from the Shot, not chosen. Name = the Shot's
  app name. Ticker = its alphanumerics, uppercased, max 10 — a Shot named
  `tohseno` deploys **$TOHSENO**.
- **Creator rights:** chosen in Studio as an ENS name, wallet address, X
  account, or Farcaster account. Simulation resolves the identifier to an
  address; the live deployment is pinned to that simulated address.
- **One coin per Shot:** if the Shot already has a token association,
  Studio refuses to deploy a replacement.
- **Afterwards:** a receipt lands in the app folder and a signed, private
  Token Association is recorded. No publication, no registry write, no
  ownership transfer.

## 1 · Bankr side (outside this repo)

- [ ] Create a **user** API key at <https://bankr.bot/api-keys>.
      It must begin with `bk_usr_`. Partner keys are rejected by Studio on
      purpose — this is a personal launch surface.
- [ ] Confirm the Bankr account behind that key can launch on the chain you
      pick (funding / credits are Bankr's side of the fence — check there,
      not here).
- [ ] If you want a token image: host it at a stable HTTPS URL first
      (e.g. `https://tohseno.com/token.png`). HTTP or data URIs are rejected.

## 2 · Key custody on this Mac

The modal asks for a Bankr user API key when Studio has no environment key.
The submitted key is held zeroized in Studio process memory for the single-use
approval, sent only as an `X-API-Key` header over TLS, and never written to
disk or included in a response or receipt. Closing the modal clears the field;
using, replacing, or ending Studio clears the in-memory approval. An
environment key remains supported for operators who prefer Keychain-backed
startup. Keep either form out of shell history:

- [ ] Store it in the macOS keychain once:

      security add-generic-password -a "$USER" -s bankr-api-key -w 'bk_usr_…'

## 3 · Decisions (decide now, not in the dialog)

- [ ] **Chain:** `robinhood` (4663) or `base` (8453).
      Robinhood Chain is the ideological home (`eip155:4663` is the pinned
      contract-definition target); Base has the deeper liquidity and tooling
      today. Pick one and write it here: ____________
- [ ] **Creator vesting:** `fifteen_percent` (15%, one year, 30-day cliff)
      or `none` (100% enters the pool). Write it here: ____________
- [ ] **Creator fees:** `mixed` (launched token + quote token) or
      `quote_only`. Write it here: ____________
- [ ] **Creator fee and vesting recipient:** choose its identifier type and
      exact value. Write both here: ____________ / ____________
- [ ] **Description:** 1–500 characters. The default in the dialog is
      "Persistent computational identity for coherent human intentions."
- [ ] **Optional URLs:** image / website / launch post. HTTPS only, no
      embedded credentials, ≤ 2048 bytes each.

## 4 · The Shot

- [ ] The `tohseno` Shot exists in Studio with a verified v2 identity
      (the launch surface refuses a Shot without one — its ShotID goes into
      the confirmation phrase and the signed association).
- [ ] The Shot has **no** existing token association
      (Shot continuity panel → "Token association (v2)" → None).

## 5 · Dress rehearsal (nothing broadcasts)

A simulation alone never deploys — Studio always calls Bankr in `simulateOnly`
mode, and the deploy button stays disarmed until you tick the acknowledgment
and type the exact phrase. Rehearse without touching either.

- [ ] Start Studio. Either enter the key in the launch modal or supply it
      from Keychain at startup:

      BANKR_API_KEY="$(security find-generic-password -s bankr-api-key -w)" tohseno studio

- [ ] Select the Shot → "Launch Appcoin for this Shot, via Bankr".
- [ ] Enter the API key if requested, fill the recipient and remaining
      parameters, and confirm that a supplied image URL renders in the public
      preview → **Simulate securely with Bankr**.
- [ ] Simulation must report success and show:
      predicted token address · resolved recipient · configuration digest ·
      fee distribution. A simulation never returns a transaction hash —
      Studio verifies that.
- [ ] Read the fee distribution. If anything surprises you, stop here.
      Nothing has been broadcast.

## 6 · The live run

- [ ] Simulate again in the same Studio session if the rehearsal approval
      expired (approvals are single-use and expire after **10 minutes**;
      changing any parameter invalidates the approval).
- [ ] Tick the acknowledgment, then type the exact phrase Studio shows:

      DEPLOY $TOHSENO FOR SHOT <shot_id> ON <ROBINHOOD|BASE> TO <TYPE>:<RECIPIENT> AT <predicted_address>

- [ ] Press deploy **once**. Keep the window open until a receipt appears.

**Failure discipline — the one rule that matters:** if the deploy request
errors after sending (network drop, 5xx), the transaction may still have
reached Bankr. Studio will say so. **Do not retry.** Check Bankr's recent
launches for $TOHSENO first; only simulate again once you have confirmed
nothing deployed.

## 7 · After it lands

- [ ] Receipt exists (mode 0600) at:
      `<tohseno working tree>/.tohseno/token-launches/tohseno-<chain>-<0xaddress>.json`
- [ ] The deployed address matches the simulated address (Studio verifies
      and surfaces warnings if Bankr drifted).
- [ ] The private Token Association is recorded — Shot continuity panel
      shows it; availability stays `intentionally_private`.
- [ ] Open the transaction in the explorer link Studio offers; save the URL.

## Known limits of this personal surface

- The irreversible deploy step has no process-level lock.
  Its gates are the ceremony itself: a Bankr user key, a fresh single-use
  simulation approval, the acknowledgment, and the exact typed phrase.
- The post-deploy Token Association is private local lineage. It does not
  publish the Shot, write the TOHSENO registry, or make the token an identity
  or ownership credential.
- A session-entered key lives in the local Studio process only while its
  single-use simulation approval is pending; Studio deliberately has no key
  persistence feature.
