# Evolutionary execution report — 2026-09-03

Repository base inspected: `0d011397a2adde75d476c064876b7a407fa94109` on
`main`. Final source milestone: `3a4f85160c3746c9cd32d2da83a6fd0c47a91e35`,
created separately from pre-existing owner working-tree changes. This report is
the report-only descendant of that source state.

The requested pass has one deliberate irreversible R2 boundary and the physical
product acceptance boundaries still owner-attended. The axes remain separate:

- **R2 source:** implemented. Filesystem/R2 backends, exact immutable keys,
  create-only writes, fresh-stream bounded retries, full readback verification,
  public range streaming, retry-safe publication ordering, and local
  inventory/migration/audit tooling are present.
- **R2 local evidence:** TypeScript, focused/full tests, migration dry-run/apply
  fixtures, rollback selector, and network E2E pass.
- **R2 external evidence:** the dedicated bucket is private, the scoped
  credential is present only in the local/production secret boundaries, and
  the mounted-production dry-run and apply passed for the one 461,076,480-byte
  Anky blob with zero failures. Production selects R2. Direct R2 full/range
  smoke and the unchanged external public URL both matched the signed digest
  and length. A real selector rollback served the same full digest from the
  retained filesystem copy; production then returned to R2 and passed health
  and range checks. No chain action or publication was performed. Only the
  irreversible `sha256/` Bucket Lock confirmation remains owner-attended. See
  `docs/R2_REGISTRY_BLOB_STORAGE_REPORT.md`.
- **Living Workshop:** ADR 0039, Mac shell/onboarding/One Shot/keyboard model,
  Companion pocket shell, Tohseno's state-driven keeper actor, accessibility,
  reduced motion, haptic Shot acknowledgement, 22-scenario truth catalog, and
  deterministic Mac/phone rendering are implemented and locally verified.
  The source is not a signed/notarized release and has no new physical pairing,
  build/install, second-human, Ship, or Claim acceptance evidence. See
  `docs/LIVING_WORKSHOP_REPORT.md`.

Local evidence on the final source: website TypeScript and all 152 Bun tests,
all 37 Mac tests, all 41 Companion tests, the standalone documentation check
(zero diagnostics), all 21 Studio static/deletion tests, and
`./scripts/test-network-e2e.sh` pass. The refreshed Mac and Companion workshop
fixtures were visually inspected. A later source edit would invalidate these
counts.

Single next owner action: in an attended Cloudflare session, add an indefinite
Bucket Lock rule named `registry-final-objects` for the exact `sha256/` prefix of
the dedicated Registry bucket. Do not lock `pending/` or the whole bucket.
Review the provider confirmation before accepting it; the website credential
must not be granted authority to create or weaken this rule.
