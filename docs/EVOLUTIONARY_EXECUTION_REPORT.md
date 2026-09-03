# Evolutionary execution report — 2026-09-03

Repository base inspected: `0d011397a2adde75d476c064876b7a407fa94109` on
`main`. Final source milestone: `3a4f85160c3746c9cd32d2da83a6fd0c47a91e35`,
created separately from pre-existing owner working-tree changes. This report is
the report-only descendant of that source state.

The requested pass has one deliberate production boundary still owner-attended.
The axes remain separate:

- **R2 source:** implemented. Filesystem/R2 backends, exact immutable keys,
  create-only writes, fresh-stream bounded retries, full readback verification,
  public range streaming, retry-safe publication ordering, and local
  inventory/migration/audit tooling are present.
- **R2 local evidence:** TypeScript, focused/full tests, migration dry-run/apply
  fixtures, rollback selector, and network E2E pass. The real production service
  is deployed with the backward-compatible filesystem selector; health,
  Registry status, and the unchanged Anky `HEAD`/first range pass.
- **R2 external evidence:** blocked. No dedicated private bucket or scoped
  credential exists, so no mounted-production dry-run, real migration apply,
  R2 selector activation, R2-backed Anky full download, production rollback, or
  `sha256/` Bucket Lock is claimed. See
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

Single next owner action: create one dedicated private Standard R2 bucket and
one bucket-scoped **Object Read & Write** S3 credential, add only the four
`REGISTRY_R2_*` production secrets while leaving
`REGISTRY_BLOB_STORE=filesystem`, and confirm they are present without exposing
their values. The application must remain on filesystem storage until the real
dry-run/apply audit exists.
