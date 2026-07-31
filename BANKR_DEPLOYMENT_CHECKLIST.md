# BANKR DEPLOYMENT CHECKLIST — $TOHSENO

The vision putting on shoes. Every line below is enforced by code in
`cli/src/bankr_launch.rs` and `cli/src/studio_server.rs`; nothing here is
aspirational. Work top to bottom. Stop at the first unchecked box.

Current state at the time this file was written: the running Studio reports
`configured: false, deploy_enabled: false`. That is the correct starting point.

---

## 0 · What will actually happen

- **Signer / deployer:** the Bankr wallet that owns your `BANKR_API_KEY`.
  Not jpfraneto.eth. Not this Mac. Studio only sends one HTTPS request to
  `https://api.bankr.bot/token-launches/deploy`.
- **Token identity:** derived from the Shot, not chosen. Name = the Shot's
  app name. Ticker = its alphanumerics, uppercased, max 11 — a Shot named
  `tohseno` deploys **$TOHSENO**.
- **Creator rights:** pinned in the binary. Fee recipient is
  `jpfraneto.eth` (`0xed21735DC192dC4eeAFd71b4Dc023bC53fE4DF15`), sent as ENS.
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

The key lives only in the Studio process's environment. It is held zeroized
in memory, sent only as an `X-API-Key` header over TLS, and never written to
disk — receipts do not contain it. Keep it out of your shell history too:

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

## 5 · Dress rehearsal (deploy stays locked)

- [ ] Restart Studio with the key but **without** the deploy unlock:

      BANKR_API_KEY="$(security find-generic-password -s bankr-api-key -w)" tohseno studio

- [ ] Select the Shot → "Launch Appcoin for this Shot, via Bankr".
- [ ] Fill the decided parameters → **Simulate with Bankr**.
- [ ] Simulation must report success and show:
      predicted token address · resolved recipient · configuration digest ·
      fee distribution. A simulation never returns a transaction hash —
      Studio verifies that.
- [ ] Read the fee distribution. If anything surprises you, stop here.
      Nothing has been broadcast.

## 6 · The live run

- [ ] Restart Studio with both locks open:

      BANKR_API_KEY="$(security find-generic-password -s bankr-api-key -w)" TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY=1 tohseno studio

- [ ] Simulate again (approvals are single-use and expire after
      **10 minutes**; changing any parameter invalidates the approval).
- [ ] Tick the acknowledgment, then type the exact phrase Studio shows:

      DEPLOY $TOHSENO FOR SHOT <shot_id> ON <ROBINHOOD|BASE> TO JPFRANETO.ETH AT <predicted_address>

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
- [ ] Remove the deploy unlock: next Studio starts **without**
      `TOHSENO_ALLOW_BANKR_TOKEN_DEPLOY=1`.

## Known limits of this personal surface (by design, revisit before anyone else uses it)

- Fee recipient is hard-pinned to jpfraneto.eth in the binary.
- The recorded association symbol is hard-coded `TOHSENO`
  (`studio_server.rs → record_bankr_shot_association`) — correct for this
  launch, wrong for any other Shot. Generalize before a second coin.
- The deploy button copy says "$TOHSENO" regardless of Shot.
