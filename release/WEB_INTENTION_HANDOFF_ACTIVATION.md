# Web-to-local intention handoff activation runbook

This runbook is release preparation, not deployment authorization. Main is
safe before activation: the public installer remains pinned to the last
published immutable release, `INTENT_RELAY_ENABLED` defaults false, and
production refuses relay activation without `CLAIM_INSTALLER_READY=true`.

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
