# Web-to-local intention handoff activation runbook

The repository owner authorized and completed this activation on 2026-08-03.
The ordered procedure remains the repeatable fail-closed runbook:
`INTENT_RELAY_ENABLED` defaults false, and production refuses relay activation
without `CLAIM_INSTALLER_READY=true`.

## Production activation record

- Reviewed implementation: pull request 1, merged at
  `ba94806fd64ad87db7711a6db36d2d397a3a105d`.
- Immutable release: `v0.8.3`, published by recovery workflow run
  `30840143848` after all release gates passed.
- Public installer activation: pull request 3, merged at
  `f1be7619ce5f8ba80ed1bdd70dcdb757869e58d0`; both public paths were verified
  byte-identical to the released `oneshot.sh`.
- Production deployment: Railway deployment
  `00aed190-9c95-4cb1-80a4-eb2c13aa5952`, serving `https://tohseno.com` from
  the exact merge above.
- Durable relay: `/data/intent-relay`, private mode `0700`, bounded to 1,000
  records and 4 GiB with the reviewed global and per-source request limits.
- Gate order: `CLAIM_INSTALLER_READY=true` preceded
  `INTENT_RELAY_ENABLED=true`.
- Verification: capability discovery, security headers, independent cleanup,
  restart persistence, immutable installer equality, and a real production
  encrypted claim with two ordered image fixtures all passed. The automated
  smoke disabled Studio auto-open and did not launch a coding harness or paid
  model.
- Privacy observation: installer and relay logs contained no token, prompt,
  or reference filename from the smoke. Three aborted smoke records were
  deleted by exact state, size, chunk count, and creation window; one
  metadata-only completion tombstone remained and persisted across restart.

## Required order

1. Review and merge the complete browser, relay, installer, CLI, engine,
   Studio, privacy, and test slice.
2. Publish the claim-capable TOHSENO release through the existing stable
   release workflow. Do not reuse or mutate the immutable 0.8.2 release.
3. Download every immutable artifact and `SHA256SUMS`; verify the tag, release
   manifest, binary/helper versions, checksums, and `tohseno intent claim`
   help from both macOS architectures.
4. Update both public thin-installer files from the reviewed canonical
   installer, pinning only that now-published release, and prove byte identity
   and the existing installer regression gates.
5. Configure an owner-controlled durable volume at an explicit absolute
   `INTENT_RELAY_ROOT`, HTTPS canonical `BASE_URL`, capacity/rate limits, and
   the independent cleanup command. Verify permissions, persistence across a
   server restart, backup exclusion, and storage deletion behavior.
6. Set `CLAIM_INSTALLER_READY=true`, then set
   `INTENT_RELAY_ENABLED=true`. Never reverse these two gates.
7. Deploy the website and confirm capability discovery reports available only
   at the canonical HTTPS origin.
8. From a clean Mac, submit an innocuous prompt and two fixtures, run the exact
   public command, complete real onboarding, inspect the imported reference
   order, approve preparation, and verify the relay tombstone without running
   a paid harness.
9. Monitor only content-free event names, response statuses, bounded byte
   counts, duration classes, capacity, expiry cleanup, and error classes. Do
   not add relay IDs, capabilities, keys, package/content hashes, prompts,
   filenames, or image metadata to telemetry.

If any step fails, leave or return the relay flag to false. Do not make the
website advertise a command whose pinned installer lacks `--claim`.
