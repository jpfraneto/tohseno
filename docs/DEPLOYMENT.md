# Release and deployment

There are three distinct external actions: publishing the managed CLI artifact,
deploying the public site, and deploying an owner’s shot. None occurs from a
test, factory command, or coding-agent launch without explicit owner approval.

## Public topology

`apps/site` is one stateless Bun process serving the landing page, docs,
privacy, `GET /healthz`, the canonical `/install.sh`, and the legacy
`/oneshot.sh` thin pinned delegator. It has no database, volume, account, form,
analytics, or generated-app content path.

- Container: repository `Dockerfile`, non-root `bun` user.
- Environment: `NODE_ENV=production`, `PORT`,
  `BASE_URL=https://tohseno.com`, `TRUST_PROXY=true`.
- Health: `GET /healthz`.
- Deployment command: `railway up` from the repository root, only after owner
  approval. Pushing Git does not deploy this site.

## Managed CLI artifact

The installer expects one immutable artifact:

```text
GitHub release tag: cli-v0.4.0
asset:              tohseno-cli-0.4.0.tar.gz
metadata:           tohseno-cli-0.4.0.json
```

The tarball is a deterministic source distribution containing the launcher,
factory source, shared skills engine, catalog descriptors and overlays,
generic and legacy manifest tooling, shot runtime/playbooks, neutral iOS
kernel, templates, and the legacy continuity base. The
installer separately acquires pinned Bun 1.2.18 and, when missing,
cloudflared 2026.5.2. Every download is SHA-256 verified before extraction.
The CLI archive also contains a canonical checksum and executable-mode
inventory. The installed wrapper authenticates that complete tree, Bun, and
any managed cloudflared binary before every execution.

Prepare locally:

```sh
bun run check
bun run tohseno:release
shasum -a 256 dist/tohseno-cli-0.4.0.tar.gz
cat dist/tohseno-cli-0.4.0.json
```

Build a second time into another temporary path and compare bytes before
release. `apps/site/public/install.sh` must contain the same complete SHA-256
digest as the metadata. The automated installer test proves local installation,
the first-use trust copy, no-preinstalled-Bun behavior, idempotency, generic
Blank composition and verification, and checksum rejection without contacting
public infrastructure.

## Published release

CLI 0.3.0 is **Implemented** and was published on 2026-07-23 from commit
`d7f943e5a61eb81c3ecbce1733a80f244bb2e0bb`. Its public tarball is
byte-for-byte identical to the deterministic local artifact, with SHA-256
`0482ac2e9f80468528f272ac43c7e4ad4bc60ef8947733dfe077784861ce7d43`.
It added the shared creation service, private portable creation provenance,
Tohseno Studio, structured progress, native Simulator run/preview services,
and the pinned `serve-sim` browser bridge.

CLI 0.3.1 is **Implemented** and was published on 2026-07-24 UTC from commit
`48bada35f885216c8c2bf3ab4d51d0c935e2e01e`. It hardens identity and session
durability, authenticates installed and third-party bytes, requires private
Studio sessions for reads and writes, scans the public worktree for copied
private input after every agent exit, isolates unsafe results, constrains
runtime state and logs, and aligns the manifest, backend, setup, and production
endpoint gates. Three clean builds from the frozen source produced byte-identical
artifacts: archive SHA-256
`a8cbee45aacb658083c435298c4e83be062f0daa45c73951c837bc130ef37a5e`
and authenticated internal-tree SHA-256
`8d24dfc235f5a187264297576544368f4b9937c1f59fbf69d287e1302c34ed4f`.
Both public assets were downloaded and matched the frozen files exactly. An
isolated install from the public artifact completed with CLI 0.3.1 and managed
Bun 1.2.18 without changing shell profiles.

CLI 0.4.0 is **Implemented** and was published on 2026-07-24 UTC from commit
`fb838768fda1e6abd45aefab76b6259d422ce67b` (Git tree
`e7c62aaf1d6cdd072f8cd5f391905206b5756552`). It adds the neutral iOS
kernel, deterministic template and skill composition, generic app manifests,
private intention planning, structured CLI and Studio plan visibility,
authoritative handoffs, and the working Daily Game composition. Two clean
builds from that exact commit were byte-identical: archive SHA-256
`c2f855f29024043bcaea1740a68642315de303e6c81a166f28c48e72edfe8004`
and authenticated internal-tree SHA-256
`76ce7071ea668bf9acc87496d11ca96ace2200f038f0055ee61c2f02d5d492b6`.
Both public assets were downloaded after publication and matched the frozen
local files exactly.

The 0.4.0 release followed this sequence:

1. Land the implementation commit containing the CLI and exact factory inputs.
2. From that exact source, run `bun run check` and `bun run tohseno:release`.
3. Build again into a separate temporary directory and compare both archives
   and metadata byte-for-byte.
4. With explicit publishing approval, push the frozen commit, create the
   versioned GitHub release, and
   upload the two unmodified artifact files.
5. Download both public assets and compare them with the frozen files.
6. Only after publication, update `apps/site/public/install.sh` with the exact
   archive and internal-tree hashes and land that as a separate commit.
7. Only a later serving commit may point `/oneshot.sh` at that installer
   commit and its exact installer-file hash.
8. With site-deployment approval, run `railway up` and verify
   `/healthz`, `/install.sh --help`, and an isolated install.

The executed 0.4.0 publication command was:

```sh
gh release create cli-v0.4.0 \
  dist/tohseno-cli-0.4.0.tar.gz \
  dist/tohseno-cli-0.4.0.json \
  --target fb838768fda1e6abd45aefab76b6259d422ce67b \
  --title "TOHSENO CLI 0.4.0" \
  --notes "Intention-driven iOS app factory: neutral kernel, deterministic template and skill composition, generic app manifests, private planning, Studio plan visibility, authoritative handoffs, and a working Daily Game composition. Built from commit fb838768fda1e6abd45aefab76b6259d422ce67b with archive SHA-256 c2f855f29024043bcaea1740a68642315de303e6c81a166f28c48e72edfe8004 and authenticated tree SHA-256 76ce7071ea668bf9acc87496d11ca96ace2200f038f0055ee61c2f02d5d492b6."
```

No package registry publication is required by this design.

## Legacy oneshot boundary

`apps/site/public/oneshot.sh` is a thin compatibility delegator. During the
0.4.0 installer-pin commit it intentionally remains on the published 0.3.1
installer. A later serving commit pins it to the committed 0.4.0 installer and
that installer's exact SHA-256, so the pin trails the serving commit by one.
It remains `must-revalidate`. Shell must never regain its own template copier,
manifest validator, shot creator, or agent launcher.

Before a site deployment:

```sh
bash -n apps/site/public/oneshot.sh
bash apps/site/public/oneshot.sh --help
bash apps/site/public/oneshot.sh --version
bash apps/site/public/oneshot.sh --dry-run --without-cloudflared
bun run check
```

## Shot production boundary

The public site does not deploy generated apps. Generic manifests carry
explicit production readiness and declarations, but no generic deployer is
implemented. Legacy continuity-v1 shots retain this implemented read-only
operation:

```sh
tohseno machine production inspect --json
```

It reports endpoint, persistence, backup, secret-reference, and capability
blockers. The initial production persistence contract is honestly
single-instance SQLite with an explicit absolute path and explicit backup
strategy. It does not imply Postgres or horizontal scaling.

Production deploy, monitoring, recovery, DNS mutation, VPS provisioning, and
store submission remain **Proposed**. Xcode signing setup and `fastlane beta`
are **Prepared**; an agent may explain or print them, but may not run them
without approval for credentials, accounts, cost, and publishing. A
`trycloudflare.com` Quick Tunnel can never satisfy the production contract.
