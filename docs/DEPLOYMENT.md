# Release and deployment

Publishing the managed CLI artifact, deploying the public site, and deploying
an owner's Shot are three distinct external actions. Tests, factory commands,
and coding-agent launches perform none of them without explicit owner
approval.

## Public site

`apps/site` is one stateless Bun process serving the landing page,
documentation, privacy page, `GET /healthz`, static presentation assets, and
the canonical `GET /install.sh` release installer. It has no alternate
bootstrap route, application database or volume dependency, account, form,
analytics, or generated-app content path.

- Container: repository `Dockerfile`, non-root `bun` user.
- Environment: `NODE_ENV=production`, `PORT`,
  `BASE_URL=https://tohseno.com`, and `TRUST_PROXY=true`.
- Health check: `GET /healthz`.
- Deployment command: `railway up` from the repository root, only after
  explicit owner approval.

Pushing Git does not deploy the site. The installer is served with
`Cache-Control: public, max-age=0, must-revalidate`.

## Managed CLI artifact

The `0.5.0` source artifact is **Implemented** and published at
[`cli-v0.5.0`](https://github.com/jpfraneto/tohseno/releases/tag/cli-v0.5.0).
Its deterministic archive contains the launcher, factory source, canonical
manifest tooling, composition engine, protocol packages, app capabilities,
neutral iOS kernel, templates, and pinned Shot machine rails. The installer
separately acquires its pinned runtime dependencies and verifies every
downloaded archive before extraction.

Release evidence:

- source commit:
  `3c471ad8dc1a42349e445b6e378fd21f0e7613d2`;
- archive SHA-256:
  `9737b8a87b6c203a5275ec5cf4e6c6a616f9e05e7da3dc8821d7f2b4c3111313`;
- authenticated tree SHA-256:
  `dea1607ca84f056061c890f718c242733cbf089cc4e3f4701d88e750eb236367`;
- 204 regular files, 1,740,256 compressed bytes, and 5,405,658
  uncompressed content bytes;
- two independent clean builds and both downloaded public assets matched
  byte-for-byte.

Reproduce the artifact locally from the source commit:

```sh
bun run check
bun run tohseno:release
shasum -a 256 dist/tohseno-cli-0.5.0.tar.gz
cat dist/tohseno-cli-0.5.0.json
```

Rebuilding locally does not publish or replace the immutable release.

## Source provenance

The release builder runs only at the exact Git worktree root. First-party
input comes from the NUL-delimited inventory produced by:

```text
git ls-files --cached --others --exclude-standard
```

That includes tracked files and visible untracked files while excluding
ignored files. Inputs must remain regular, in-repository files throughout the
snapshot. The builder reads each file into immutable bytes, verifies its
identity and content again, and refuses path escapes, symbolic links, unsafe
modes, inventory changes, source-commit changes, or size-limit violations.
Pinned third-party trees are independently authenticated.

Release metadata records the source commit, inventory rule, dirty state,
archive SHA-256, and authenticated internal-tree SHA-256. A dirty artifact is
valid only as local preparation evidence. Publication requires:

- `source.dirty` is `false`;
- `source.commit` is the exact frozen release commit;
- two independent builds from that commit are byte-identical;
- archive and internal-tree hashes match the metadata;
- the versioned public assets are downloaded after publication and match the
  frozen local bytes.

The installer pins the exact archive and internal-tree hashes and fails closed
on mismatch. The installed wrapper authenticates its managed tree and runtime
before execution.

## Ordered publication boundary

Release ordering is part of the trust boundary:

1. Land and push the frozen CLI source commit.
2. Build twice from that exact clean commit and run the full check gate.
3. With explicit publishing approval, create the versioned GitHub release and
   upload the unmodified archive and metadata.
4. Download both assets and compare them with the frozen local files.
5. Only then, in a separately reviewed change, update the canonical installer
   with the published archive and internal-tree hashes.
6. Expose that one installer only after its route and exact bytes pass the
   repository gate.
7. With separate site-deployment approval, run `railway up` and verify
   `/healthz` and the reviewed public routes.

A future versioned publication command must be reviewed with exact values:

```sh
gh release create "cli-v<version>" \
  "dist/tohseno-cli-<version>.tar.gz" \
  "dist/tohseno-cli-<version>.json" \
  --target "<frozen-source-commit>" \
  --title "TOHSENO CLI <version>" \
  --notes-file "<reviewed-release-notes>"
```

Do not run it without explicit owner approval. Package-registry publication is
not required by the current installer design.

## Canonical installer serving boundary

`apps/site/public/install.sh` pins the exact public 0.5.0 archive and tree. Its
reviewed SHA-256 is
`442325c0355ed4b2ba3896367bfd5e143bb0e481c4d84e09df08a702ef9528ca`.
It accepts only a canonical 0.5 managed home, rejects preexisting noncanonical
install roots without mutating them, and contains no migration branch. The
reviewed installer pin is commit
[`1721e139134e1ee78fc32482a20823d06393be59`](https://github.com/jpfraneto/tohseno/commit/1721e139134e1ee78fc32482a20823d06393be59).
The serving change exposes those exact bytes at
`https://tohseno.com/install.sh`; `/oneshot.sh` remains absent and there is no
second installer or workspace creator.

The public installation command is:

```sh
curl -fsSL https://tohseno.com/install.sh | sh
```

## Shot production boundary

The public site and factory do not deploy generated apps. Each Shot declares
production readiness, storage, network behavior, integrations, entitlements,
and irreversible operations in `app.manifest.json`. Credentials remain
owner-managed external inputs and never enter the manifest.
There is no general Shot production deployer.

Production deployment, monitoring, recovery, DNS mutation, infrastructure
provisioning, Xcode distribution signing, TestFlight submission, and App Store
submission remain **Proposed**. No submission lane or deployment command is
implemented.
