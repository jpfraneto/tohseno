# Evolutionary execution report — 2026-09-03

Repository base inspected: `0d011397a2adde75d476c064876b7a407fa94109` on
`main`. Implementation commit `80a6e02cc9bbb55ad5291ce41ecaa132660a4b7c`
is pushed to `origin/main`, separately from pre-existing owner working-tree
changes.

The requested pass is source-complete and locally verified, with one deliberate
production boundary still owner-attended.

- Durable Registry blobs now have filesystem/R2 backends, exact immutable keys,
  create-only writes, full readback verification, public range streaming,
  retry-safe publication ordering, local inventory/migration/audit tooling, and
  an explicit rollback/runbook. Production remains on the existing filesystem:
  there is no dedicated Registry bucket or credential yet, so no R2 cutover or
  Bucket Lock is claimed. See `docs/R2_REGISTRY_BLOB_STORAGE_REPORT.md`.
- ADR 0039 and the native source establish One Shot / Living Workshop across Mac
  and Companion while preserving every current capability and authority gate.
  Four rendered fixtures and a 22-scenario truth catalog document the result.
  The source is not a released/notarized build and has no new physical-human
  acceptance evidence. See `docs/LIVING_WORKSHOP_REPORT.md`.

Local evidence on the final source: website TypeScript and all 148 Bun tests,
all 35 Mac tests, all 39 Companion tests, the standalone documentation check,
and visual inspection of all four generated fixtures pass. A later source edit
would invalidate these counts.

Next real action: an authorized owner creates the private Registry R2 bucket and
least-privilege credentials, supplies the four `REGISTRY_R2_*` production
secrets with all writes dark, reviews `registry:r2:migrate --dry-run`, then
authorizes apply/cutover and the known-Anky live read smoke. The application
must remain on filesystem storage until that evidence exists.
