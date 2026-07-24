# TOHSENO security research log

Last updated: 2026-07-23

This is the active, content-free security and reliability ledger for TOHSENO.
It records code paths, synthetic test cases, and public infrastructure facts.
It must never contain owner prompts, message bodies, credentials, tokens,
private keys, seed phrases, production data, or other user content.

## Scope and threat model

The review covers the public installer and release artifact, the local CLI and
Studio, shot creation and verification, the continuity manifest, the generated
SwiftUI app, the local development API and tunnel, production inspection, the
public site, GitHub release state, and the Railway service boundary.

The protected assets are a person's cryptographic identity, on-device writing,
private creation inputs, provider credentials, the integrity of generated
repositories, and authority over external or irreversible actions.

The primary attacker models are:

- a remote web client reaching the public site or an explicitly started Quick
  Tunnel;
- a malicious page attempting to drive the loopback-only Studio;
- malformed local project state, archives, manifests, paths, or configuration;
- compromised or substituted release/dependency bytes;
- process death, full or failing storage, and transient Keychain failures;
- a coding agent that receives only its deliberately selected workspace and
  provider environment;
- operator error around production endpoints, releases, and financial actions.

A process already running as the same macOS user can generally read or modify
that user's files and replace executables. TOHSENO still minimizes secret
exposure and refuses unsafe local structures, but it does not claim to sandbox
same-user malware. Apple, GitHub, Railway, Cloudflare, npm, and a builder's
selected coding/provider services remain explicit external trust boundaries.

## Baseline evidence

- Repository state at audit start: clean `main` at `e7ab9b1`, matching
  `origin/main`.
- `bun run check`: passed 146 tests plus strict TypeScript, manifest, static,
  installer, secret, and whitespace gates.
- Public GitHub release `cli-v0.3.0`: published and points to commit `d7f943e`.
  The repository's deployment documentation still described it as prepared.
- Railway project `tohseno-production`: linked to the production service at
  `https://tohseno.com`.
- A legacy Railway volume remains attached at `/data`. The current stateless
  site does not declare or use that mount. It has not been inspected, detached,
  or deleted because ownership and recovery policy are open.
- `bun audit --json`: one moderate advisory against Ajv 8.17.1
  (`GHSA-2g4f-4pwh-qvx6`); the affected `$data` feature is not enabled by the
  manifest validator, but the dependency still needs upgrading.

Passing baseline tests are evidence only for their covered paths. They are not
a security certification.

## Active findings

### TS-SEC-001 — Keychain failure can replace or destroy an identity

- Severity: Critical
- Status: Remediated; focused simulator proof passed
- Surface: `IdentityManager`, `KeychainSecretStore`,
  `SystemSeedPhraseKeychain`
- Preconditions: a Keychain read, migration, restore, or write fails; or a
  stored seed item is malformed.
- Evidence: every Keychain read failure is represented as "missing";
  `loadOrCreate` then creates a new identity. Seed writes delete both existing
  items before attempting `SecItemAdd`. A failed add therefore destroys the
  recoverable prior identity.
- Impact: silent identity fork or permanent loss of the seed that replaces an
  account in every continuity app.
- Required proof: distinguish not-found from read failure, fail closed on
  malformed stored data, update/add without pre-deletion, migrate by
  write-before-delete, and add injected read/write failure tests.
- Resolution: Keychain absence is distinct from read failure; updates never
  pre-delete; failed creation, restore, migration, and malformed bytes preserve
  the prior identity and surface an unavailable state. The key uses
  `kSecAttrAccessibleWhenUnlocked`.

### TS-SEC-002 — Session finalization deletes the recovery draft before commit

- Severity: Critical
- Status: Remediated; focused simulator proof passed
- Surface: `SessionStore.finishDraft`
- Preconditions: storage fails between draft removal and text/sidecar commit.
- Evidence: the method clears memory and removes `draft.json` before either
  committed file is written.
- Impact: the last durable copy of a person's writing can be lost, violating
  crash-safe persistence.
- Required proof: retain the draft until text and sidecar are durable, preserve
  it on each injected failure, reconcile a committed session with a stale
  draft after process death, and use complete file protection.
- Resolution: the draft remains the recovery source until text, sidecar, and
  checkpoint cleanup all succeed. Writes are atomic, use complete file
  protection, and are excluded from backup; stale committed checkpoints
  reconcile without duplicating a session.

### TS-SEC-003 — Setup can expose or redirect secret-bearing configuration

- Severity: High
- Status: Remediated; focused setup tests passed
- Surface: `templates/continuity-app/scripts/setup.ts`
- Preconditions: setup writes `Config/Local.xcconfig`, including the optional
  `DEV_SECRET`, into a permissive umask or across a pre-existing symlink.
- Evidence: `Bun.write` is used directly without a private mode, symlink
  refusal, or a staged atomic replacement.
- Impact: another local account may read a prototype provider secret, or a
  crafted workspace can redirect writes outside the shot.
- Required proof: reject links/non-regular targets, stage and atomically
  replace both setup outputs, enforce mode `0600`, and test rollback and link
  cases.
- Resolution: setup validates the real workspace boundary, refuses linked or
  non-regular inputs/targets, writes private `wx` staging files, flushes and
  renames them atomically, and enforces `0600` on both outputs.

### TS-SEC-004 — Token launch can execute mutable code and retain provider output

- Severity: Critical
- Status: Remediated; focused runtime tests passed
- Surface: `runtime/token.ts`
- Preconditions: `bankr` is absent but `npx` is available, or Bankr returns
  sensitive/error output.
- Evidence: the runtime falls back to unversioned `npx @bankr/cli` for an
  irreversible financial action. It appends raw stdout/stderr to a local log
  and returns raw failure output after replacing only the exact API-key value.
- Impact: mutable remote code executes with user authority; provider output can
  persist secrets or content; a financial operation can run through an
  unreviewed package version.
- Required proof: require an explicitly installed executable, never use
  implicit `npx`, keep logs structured and content-free, validate arguments,
  pass credentials only to scoped Bankr subprocesses, and test hostile output.
- Resolution: token commands require an explicit `bankr` executable, run a
  non-broadcasting simulation before launch, constrain all parameters, scope
  credentials to provider subprocesses, bound captured output, and retain only
  semantic outcomes. Provider economics are linked, not hard-coded or
  guaranteed.

### TS-SEC-005 — Release packaging authenticates dependency labels, not bytes

- Severity: High
- Status: Remediated; focused installer tests passed
- Surface: `packages/cli/scripts/package-release.ts`
- Preconditions: a managed `node_modules/serve-sim` or `node_modules/ws` tree
  is modified while retaining its package name and version.
- Evidence: packaging checks only `package.json` identity before including all
  regular files, including native code, in the checksum-pinned CLI archive.
- Impact: a locally compromised dependency becomes a formally pinned release.
- Required proof: verify a canonical whole-tree digest and test content and
  executable-mode tampering before packaging.
- Resolution: packaging authenticates canonical path, size, executable mode,
  and content hashes for the complete managed `serve-sim` and `ws` trees.

### TS-SEC-006 — Production loopback rules diverge across validators

- Severity: High
- Status: Remediated; cross-language fixtures and simulator proof passed
- Surface: Swift `AppConfig`, shell release gate, TypeScript production
  inspection
- Preconditions: a production endpoint uses an alternate loopback
  representation such as another address in `127.0.0.0/8` or an IPv4-mapped
  IPv6 loopback address.
- Evidence: each implementation recognizes a different, incomplete set.
- Impact: a Release build can accept an endpoint that resolves only to the
  phone itself, while another gate reports it differently.
- Required proof: shared cross-language fixtures for canonical loopback forms
  and equivalent rejection by all three gates.
- Resolution: one JSON corpus now drives Swift, TypeScript, and shell tests for
  IPv4, IPv6, mapped-loopback, port, trailing-dot, DNS-label, and Quick Tunnel
  cases.

### TS-SEC-007 — Public-site access logs retain arbitrary path content

- Severity: Medium
- Status: Remediated; focused HTTP tests passed
- Surface: `apps/site/server.ts`
- Preconditions: a visitor places content or credentials in a URL path.
- Evidence: the structured route field includes the raw pathname for every
  request.
- Impact: user-controlled content is retained in production logs, contrary to
  the repository's content-free logging rule.
- Required proof: log only allowlisted semantic route names and test that
  unknown sensitive-looking paths never reach either logger.
- Resolution: the site records only allowlisted route and method classes;
  arbitrary paths, query strings, headers, and bodies never enter either
  logger.

### TS-SEC-008 — Manifest validator uses a dependency with a known ReDoS advisory

- Severity: Moderate
- Status: Remediated; dependency audit and manifest corpus passed
- Surface: Ajv 8.17.1
- Preconditions: the vulnerable optional `$data` facility must be enabled;
  TOHSENO does not currently enable it.
- Evidence: package audit reports `GHSA-2g4f-4pwh-qvx6`.
- Impact: low practical exposure in the present validator, but avoidable
  vulnerable code remains in the release dependency graph.
- Required proof: upgrade to a patched exact version and rerun the audit and
  manifest corpus.
- Resolution: Ajv is pinned to 8.18.0, the advisory's patched release;
  `bun audit --json` reports no advisories.

### TS-SEC-009 — Development logs are unbounded and tailed into memory

- Severity: Medium
- Status: Remediated; focused runtime tests passed
- Surface: local API/Quick Tunnel process logs and `tailLines`
- Preconditions: the builder explicitly opens a public Quick Tunnel and a
  remote client drives repeated requests, or a child process emits heavily.
- Evidence: logs append without a size cap; `tailLines` reads the entire file
  before selecting the final lines.
- Impact: disk exhaustion or CLI memory exhaustion on the development machine.
- Required proof: bounded/rotated process logs, bounded tail reads, and a large
  synthetic-log test.
- Resolution: runtime logs cap at 5 MiB, tail reads cap at 2 MiB, captured
  subprocess streams cap at 8 MiB, and all log opens refuse links and
  multi-linked files. iOS/token logs contain semantic metadata only.

### TS-SEC-010 — Seed reveal lacks local user-presence verification

- Severity: Medium
- Status: Remediated; focused simulator proof passed
- Surface: in-app identity settings
- Preconditions: an unlocked phone or an exposed app switcher is accessible to
  another person.
- Evidence: a warning is the only gate before rendering all recovery words.
- Impact: recovery phrase disclosure grants durable identity control.
- Required proof: require system user presence before reveal, mark phrase UI as
  privacy-sensitive, hide it when inactive, and exercise cancellation/failure.
- Resolution: phrase reveal requires device-owner authentication, is
  privacy-sensitive, expires on navigation or inactive scene state, and the app
  obscures itself in the task switcher.

### TS-SEC-011 — Production release documentation is stale

- Severity: Low
- Status: Remediated; public release facts verified
- Surface: deployment documentation and public release truth
- Evidence: CLI 0.3.0 is publicly downloadable, while tracked documentation
  labels it prepared and unpublished.
- Impact: operators can choose the wrong release sequence or misreport what is
  implemented.
- Required proof: update the release ledger only from verified GitHub facts.
- Resolution: deployment documentation identifies 0.3.0 and 0.3.1 as
  published and records each exact commit and SHA-256 from verified GitHub
  release facts.

### TS-SEC-012 — Legacy Railway volume has no current ownership decision

- Severity: Open
- Status: Preserved; owner decision required
- Surface: Railway production service, mount `/data`
- Evidence: a volume remains attached although the current site is stateless
  and declares no data path.
- Impact: unnecessary retained state and cost are possible, but detaching or
  deleting it without determining whether it contains recoverable legacy data
  would be destructive.
- Required proof: owner-approved retention or recovery/deletion decision. This
  audit will verify that the current application neither reads nor writes it;
  it will not inspect or delete the volume unprompted.

### TS-SEC-013 — Progress journals can follow links and retain command output

- Severity: High
- Status: Remediated; focused Studio tests passed
- Surface: `ShotProgressReporter`, creation failure events
- Preconditions: a malformed or raced workspace places a link at a predicted
  journal path, or a failing child command returns a large/content-bearing
  diagnostic.
- Evidence: append mode followed an existing link; the declared 2 MiB limit
  was enforced by readers but not writers; raw exception messages were
  persisted.
- Impact: a journal write could alter another owner file, retain sensitive
  output, or create an oversized private record.
- Resolution: journals are exclusively created, opened with no-follow,
  inode/link-count checked, mode `0600`, bounded on write and read, and their
  messages are single-line and size-limited. Persistent failure events are
  generic; immediate command diagnostics remain ephemeral.

### TS-SEC-014 — Forged runtime state can claim unrelated processes and redirect health

- Severity: Critical
- Status: Remediated; focused runtime acceptance passed
- Surface: shot-local `state.json`, start-lock ownership, `dev status`, and
  `dev stop`
- Preconditions: malformed local shot state names a live same-user PID and
  supplies empty or generic `commandContains` fragments.
- Evidence: an empty array passed `every`, making any live PID appear owned;
  health URLs were accepted from the state without reconstructing the
  loopback contract.
- Impact: `dev stop` could signal an unrelated process and `dev status` could
  make an attacker-selected request.
- Resolution: the entire state is now schema-, UUID-, path-, role-, URL-,
  transport-, and exact-command-shape validated before use. Ownership rejects
  empty/generic records, the API URL is reconstructed from its bounded
  loopback port, response bodies are bounded, and malformed state cannot signal
  the synthetic unrelated process in regression coverage.

### TS-SEC-015 — Manifest promises artifact export that the app did not expose

- Severity: High
- Status: Remediated; focused simulator proof passed
- Surface: base manifest and `SessionDetailView`
- Evidence: both canonical artifacts declared `owner-selected` export and
  content recovery declared `manual-export`, but the only share action emitted
  a rendered image.
- Impact: ejection and recovery claims were false; exact writing and its
  sidecar could not be taken out through the app.
- Resolution: the session view now offers an explicit two-file system share
  action for the exact UTF-8 text and JSON sidecar. Export revalidates both
  regular files, the record, and character count and fails closed after
  substitution.

### TS-SEC-016 — SQLite storage follows pre-existing link targets

- Severity: High
- Status: Remediated; focused backend tests passed
- Surface: base shot API database initialization
- Preconditions: the database, WAL, or shared-memory path is a symbolic link.
- Evidence: Bun SQLite opened the configured pathname before the application
  established or authenticated a private regular file.
- Impact: starting a shot API could alter or chmod another local file.
- Resolution: initialization canonicalizes the parent, establishes the database
  with exclusive/no-follow semantics, verifies existing database/WAL/SHM files,
  and enforces private modes. Synthetic target contents and mode remain intact.

### TS-SEC-017 — An existing installed CLI is trusted by marker alone

- Severity: High
- Status: Remediated; focused installer acceptance passed
- Surface: public `install.sh` idempotent-install path
- Preconditions: installed CLI files are accidentally or locally modified
  while `.artifact.sha256` retains the published archive digest.
- Evidence: rerunning the installer reports “already verified” after checking
  only the marker text, not installed bytes or modes.
- Impact: corrupted or substituted factory/CLI code continues executing under
  a misleading verification claim.
- Required proof: ship an internal canonical file inventory, verify every
  installed file and executable mode on reuse and wrapper startup, reject
  links/extras, and exercise content/mode/link tampering.
- Resolution: the deterministic archive now authenticates every path, byte,
  executable bit, and inventory file. Installation and every wrapper launch
  reject missing, extra, linked, hardlinked, writable, mode-changed, or
  content-changed CLI files. Managed Bun and cloudflared binaries receive
  verified binary markers and are checked before execution. Installer tooling
  runs under a fixed system `PATH`, managed directories are owner-only, and
  unsafe shell-profile targets are skipped.

### TS-SEC-018 — Studio exposes a reusable bearer and leaves private reads unguarded

- Severity: High
- Status: Remediated; focused Studio acceptance passed
- Surface: local Studio shell and HTTP routes
- Preconditions: another local OS account/process, browser extension, or
  locally reachable client can connect to the loopback port while Studio runs.
- Evidence: the reusable mutation bearer was embedded in the publicly served
  HTML shell. Read routes for intentions, reference images, screenshots, shot
  locations, and event streams required Host checks but no session authority.
- Impact: a local deputy could obtain private creation input or invoke actions
  without being the owner-launched browser session.
- Resolution: a mode-`0600` temporary launcher carries the one-time token in a
  fragment. The browser consumes and removes it, then bootstraps an HTTP-only,
  SameSite-strict cookie scoped to an unguessable API path. The served shell,
  process arguments, and printed base URL contain no bearer. Every private read
  and mutation requires the session; bootstrap and mutations additionally
  require exact same-origin request context. Shutdown deletes the launcher.

### TS-SEC-019 — Agent output can copy private creation input into the public worktree

- Severity: Critical
- Status: Remediated; focused creation and end-to-end acceptance passed
- Surface: post-agent verification and creation failure path
- Preconditions: a selected coding agent copies an intention/reference outside
  `.tohseno/provenance`, creates a private-boundary link, or exits nonzero after
  doing so.
- Evidence: structural verification rejected tracked private path names but did
  not compare the public worktree with private input bytes. A nonzero agent
  exit occurred before the post-agent verifier, precisely skipping the gate on
  the failure path.
- Impact: an owner prompt or reference could enter ordinary source, a later Git
  commit, or an archive while TOHSENO still reported a preserved shot.
- Resolution: the pinned verifier walks a bounded worktree, refuses special,
  linked, and multi-linked files, compares exact reference and intention
  digests, and searches for the complete normalized intention embedded in
  another file. It checks required ignore rules and runs after every agent exit,
  including failure. A failed post-agent gate atomically moves the result to an
  explicitly unsafe hidden sibling rather than publishing the canonical slug.
- Residual limit: this is deterministic leak detection, not semantic DLP.
  Paraphrased intentions, excerpts shorter than the false-positive threshold,
  and transformed image content cannot be identified reliably. The selected
  coding provider remains an explicit trust boundary.

### TS-SEC-020 — Factory and release trees accept ambiguous filesystem identities

- Severity: High
- Status: Remediated; focused release acceptance passed
- Surface: factory source, immutable cache, trusted-tool snapshots, and public
  release packaging
- Preconditions: a source/cache root is replaced by a link, a file is
  hardlinked outside its boundary, metadata declares an excessive inventory,
  or cleanup traverses a substituted directory.
- Evidence: several walkers rejected child symlinks but not a linked root or
  hardlinks; release metadata and hashing had no file/count/aggregate size
  ceilings; read-only cleanup chmodded by pathname.
- Impact: bytes outside the selected source can enter a formally authenticated
  release, verification can consume unbounded memory, or cleanup can alter an
  unintended target.
- Resolution: source and cache roots are canonical real directories; regular
  inputs have one link; all reads are no-follow, identity-checked, and bounded;
  inventories have file/per-file/aggregate ceilings; trusted snapshots enforce
  the same rules; cleanup changes directory modes through authenticated file
  descriptors. Release output staging is exclusive, flushed, and atomically
  renamed.

### TS-SEC-021 — Ambient Git and SSH configuration expands process authority

- Severity: High
- Status: Remediated; focused CLI tests passed
- Surface: Git subprocesses and coding-agent environment
- Preconditions: inherited environment/configuration supplies alternate Git
  directories, hooks/filters/fsmonitor behavior, provider secrets, or an SSH
  agent socket.
- Evidence: earlier filtering removed selected Git variables but retained most
  ambient process state, user/system Git configuration, and
  `SSH_AUTH_SOCK` for coding agents.
- Impact: repository inspection or baseline creation can execute unintended
  helpers, act on another repository, expose credentials, or let an agent use
  unrelated SSH authority.
- Resolution: Git receives a small environment, no system/global config,
  disabled hooks/fsmonitor/attributes/excludes, no prompt, and bounded output.
  Coding agents receive only terminal and provider-configuration locations;
  inherited application/provider secrets and the SSH agent socket are absent.

### TS-SEC-022 — Local control files and subprocess output are unbounded or link-following

- Severity: High
- Status: Remediated; focused CLI/runtime/setup tests passed
- Surface: config, manifest, metadata, provenance, runtime state, release
  records, screenshots, setup input, and captured child streams
- Preconditions: malformed local state is oversized, invalid UTF-8, linked,
  hardlinked, replaced during open, or continuously emits output.
- Evidence: multiple paths used whole-file `readFile`/`Bun.file().text()` and
  several subprocesses accumulated both streams without ceilings.
- Impact: memory/disk exhaustion, reads across a local boundary, ambiguous
  validation, or retained content-bearing output.
- Resolution: security-relevant readers now enforce explicit byte ceilings,
  fatal UTF-8 where text is required, no-follow descriptors, one-link and
  inode/device identity, including growth-after-stat checks. Captured streams
  terminate at fixed ceilings. Provenance filenames reject control characters,
  screenshots are signature/identity/size checked, and persistent failures use
  generic content-free events.

### TS-SEC-023 — Canonical redirects can be influenced by protocol-relative paths

- Severity: Medium
- Status: Remediated; focused HTTP tests passed
- Surface: public-site canonical host/protocol boundary
- Preconditions: a request to the alias or forwarded HTTP origin uses a path
  beginning with `//`.
- Evidence: resolving the request path as a relative URL against the canonical
  origin can reinterpret it as an authority.
- Impact: the trusted domain can issue an attacker-selected cross-origin
  redirect.
- Resolution: redirect destinations begin as the fixed configured origin and
  receive pathname/query as fields. Non-GET/HEAD requests cannot redirect, the
  public server caps request bodies at 1 KiB, and regression coverage includes
  protocol-relative paths.

### TS-SEC-024 — The generated backend trusts paths and metadata outside its local contract

- Severity: High
- Status: Remediated; focused backend/runtime tests passed
- Surface: base shot API, development SQLite, ready/stop files, and logs
- Preconditions: configuration points development storage outside the shot or
  local metadata/control paths are linked, malformed, or oversized.
- Evidence: the backend accepted a resolved development database path without
  a shot boundary and read metadata/control JSON normally. Startup failure and
  request logs could retain arbitrary messages/methods.
- Impact: local startup can mutate external storage, consume hostile files, or
  retain content beyond the operational logging contract.
- Resolution: development SQLite must remain within the canonical shot;
  database/WAL/SHM and JSON controls are no-follow single-link files with
  private modes and bounds; ready writes are private/exclusive/flushed;
  instance/metadata shapes are constrained; request bodies are capped and logs
  retain only semantic method/route/status fields. Production still requires
  an explicit absolute storage path and backup declaration.

### TS-SEC-025 — Manifest app names can inject Xcode configuration syntax

- Severity: High
- Status: Remediated; manifest and setup corpus passed
- Surface: `application.name` to `APP_DISPLAY_NAME` generation
- Preconditions: an app name contains line breaks, assignment/expansion,
  comment, or continuation syntax interpreted by `.xcconfig`.
- Evidence: the schema required only non-whitespace and setup rejected just
  newlines/equals.
- Impact: a product display name can alter unrelated build settings, including
  endpoint or secret seams.
- Resolution: schema, semantic validator, setup, and generated-shot tests reject
  Xcode configuration syntax consistently before interpolation.

### TS-SEC-026 — Identity restore can replace the account-equivalent seed without user presence

- Severity: High
- Status: Remediated; final 40-test Simulator gate passed
- Surface: recovery phrase restore
- Preconditions: another person has a temporarily unlocked device.
- Evidence: restore validated the words but immediately overwrote the
  synchronizable Keychain identity.
- Impact: durable identity takeover across devices while local writing remains,
  creating a hard-to-notice split between content and identity.
- Resolution: restore starts with the phrase hidden, requires device-owner
  authentication immediately before the Keychain write, disables concurrent
  attempts, clears phrase/authorization state on success, dismissal, or
  inactive scene, and reports authentication failure without changing identity.

### TS-SEC-027 — Setup private-key and local-config handling trusts ambient paths

- Severity: High
- Status: Remediated; focused setup tests passed
- Surface: App Store Connect `.p8`, `app.config.json`, `Local.xcconfig`, and
  Apple team auto-detection
- Preconditions: a key/config is linked, multi-linked, permissively readable,
  oversized, outside the workspace, or a system helper emits excessive data.
- Evidence: setup used ordinary text reads/writes and PATH-resolved helpers.
- Impact: secret disclosure, redirected writes, memory exhaustion, or execution
  of a substituted helper.
- Resolution: `.p8` input must be an owner-only, bounded, single-link regular
  file and is read no-follow; setup inputs/outputs stay in the canonical
  workspace; outputs are private, exclusive, flushed, and atomically replaced;
  only absolute Apple system tools run with a minimal environment and bounded
  output.

### TS-SEC-028 — Identity-changing Keychain writes and private artifacts lack consistent protection

- Severity: Medium
- Status: Remediated; focused CLI and simulator proofs passed
- Surface: Keychain accessibility, Studio staging, provenance, runtime
  directories, setup outputs, database, and Simulator capture
- Evidence: pre-existing permissive directories/files could retain their mode,
  and identity items used a broader accessibility default.
- Impact: another local account or a locked-device window can gain avoidable
  access to private material.
- Resolution: private directories are revalidated and forced to `0700`; private
  files use `0600`; Keychain items use `WhenUnlocked`; app files use complete
  protection and backup exclusion; unsafe links/hardlinks are rejected before
  access.

### TS-SEC-029 — Public development health reports the chosen shot slug

- Severity: Medium
- Status: Remediated; focused backend test passed
- Surface: generated API `/health` and `/ready` responses through an optional
  Cloudflare Quick Tunnel
- Preconditions: the builder explicitly starts a public development tunnel.
- Evidence: the health document included the product-chosen slug from
  `.tohseno/shot.json`.
- Impact: a remote client can learn a private idea-derived name even though the
  API otherwise exposes operational compatibility data only.
- Resolution: the wire response now reports platform, release/schema
  compatibility, persistence class, and uptime metadata without the slug.
  Regression coverage starts from valid metadata containing a distinctive
  private-looking slug and proves it is absent from the response.

## Confirmed defensive boundaries

These survived the evolved manual review and remain subject to regression
testing:

- Studio binds to loopback, checks exact Host/Origin/fetch context, keeps its
  bearer outside the served shell and process list, session-gates private reads
  and mutations, and renders private strings with text APIs rather than HTML.
- Uploaded reference files have count, declared-size, magic-byte, extension,
  filename, and private-directory controls; staging is cleaned after use.
- Automated coding-agent environments omit inherited provider/application/SSH
  authority except for deliberately selected agent configuration paths.
- Release trees, installed trees, trusted snapshots, and shot provenance reject
  links, unexpected entries, byte/mode drift, and oversized inventories.
  Global commands execute authenticated private snapshots rather than a mutable
  verifier or machine copied inside the shot.
- The generated API stores operational metadata only and has no route that
  receives app-runtime writing. Its development database stays inside the shot.
- The public site uses a static file allowlist, escaped templates, a restrictive
  content security policy, HSTS, MIME sniffing protection, canonical redirects
  with a fixed destination, bounded request bodies, and semantic logs.
- The installer verifies pinned SHA-256 values before extraction, authenticates
  the archive's complete inventory, and rechecks installed CLI/runtime bytes
  before execution.

## Residual risks and open limits

- The selected coding agent and its provider can process the input the owner
  deliberately gives it. TOHSENO minimizes and verifies local disclosure but
  does not sandbox or make a retention claim for that provider.
- Same-macOS-user malware can generally read process memory and owner files or
  replace executables between checks. The design reduces ambient authority and
  rejects ambiguous structures; it is not a same-user security boundary.
- Deterministic private-input scanning detects exact references and a complete
  normalized intention (exact or embedded). It cannot reliably identify
  paraphrases, short excerpts, screenshots, or transformed images.
- The app keeps the active mnemonic and draft text in process memory while in
  use. Keychain/file protection and scene obscuring protect storage and casual
  screen exposure, not a compromised app process.
- Multi-file session commit is recoverable rather than one filesystem
  transaction: the draft remains authoritative until text, sidecar, and cleanup
  complete, and launch reconciliation resolves the only partial-commit state.
- Quick Tunnels are intentionally public development reachability, not
  authentication or production infrastructure. The baseline API exposes only
  content-free health/readiness routes.
- The legacy Railway volume at `/data` remains the sole owner-decision item in
  this audit. The active stateless site does not use it; no contents were read.

## Release-candidate validation

The frozen 0.3.1 candidate passed:

- `bun run check`: strict TypeScript, 170 tests,
  manifest/static/deployment contracts, installer acceptance, tracked-file and
  secret hygiene, and staged/unstaged whitespace gates;
- `bun run validate templates/continuity-app/continuity.manifest.json`: valid
  0.4.0 manifest with the documented platform-private warning;
- `bun audit --json`: no reported advisories;
- `xcodegen generate`, followed by 40 passing app tests on an iPhone 17 Pro
  running iOS Simulator 26.3.1;
- three byte-identical deterministic CLI builds, archive SHA-256
  `a8cbee45aacb658083c435298c4e83be062f0daa45c73951c837bc130ef37a5e`
  and authenticated internal-tree SHA-256
  `8d24dfc235f5a187264297576544368f4b9937c1f59fbf69d287e1302c34ed4f`;
- `git diff --check` and the repository's content-free secret scan.

Browser-driven visual inspection was attempted, but no in-app browser session
was available in this environment. Semantic HTTP, navigation, accessibility,
responsive-style, and static-surface tests passed; a live visual browser pass
was not claimed.

## Deployment and disclosure boundary

CLI 0.3.1 was published from commit
`48bada35f885216c8c2bf3ab4d51d0c935e2e01e`. The two public assets were
downloaded into a new temporary directory and matched the frozen local bytes;
an install into a new temporary home completed with CLI 0.3.1 and managed Bun
1.2.18 without modifying a shell profile.

The production site deployment is the next separately authorized action and is
not claimed by this pre-deployment ledger state. No DNS action, App Store
action, credential rotation, financial action, volume mutation, or user-data
access occurred. Synthetic fixtures contain no real secrets or user content.
