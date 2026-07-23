# Release and deployment

There are three distinct external actions: publishing the managed CLI artifact,
deploying the public site, and deploying an owner’s shot. None occurs from a
test, factory command, or coding-agent launch without explicit owner approval.

## Public topology

`apps/site` is one stateless Bun process serving the landing page, docs,
privacy, `GET /healthz`, the canonical `/install.sh`, and the legacy
`/oneshot.sh` migration notice. It has no database, volume, account, form,
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
GitHub release tag: cli-v0.3.0
asset:              tohseno-cli-0.3.0.tar.gz
metadata:           tohseno-cli-0.3.0.json
```

The tarball is a deterministic source distribution containing the launcher,
factory source, shot runtime/playbooks, manifest tooling, and iOS base. The
installer separately acquires pinned Bun 1.2.18 and, when missing,
cloudflared 2026.5.2. Every download is SHA-256 verified before extraction.

Prepare locally:

```sh
bun run check
bun run tohseno:release
shasum -a 256 dist/tohseno-cli-0.3.0.tar.gz
cat dist/tohseno-cli-0.3.0.json
```

Build a second time into another temporary path and compare bytes before
release. `apps/site/public/install.sh` must contain the same complete SHA-256
digest as the metadata. The automated installer test proves local installation,
no-preinstalled-Bun behavior, idempotency, runtime acceptance, and checksum
rejection without contacting public infrastructure.

## Published release

CLI 0.2.2 is **Implemented** and published from commit
`f26a31e4456875f3886ec4967073fdfbee4e5739`. It adds the optional Bankr token
launch operations. Its public tarball is byte-for-byte identical to the
deterministic local artifact, with SHA-256
`91b75d064b36f7d24ba903000b5cd81ae355b87ac233570cf4a39f91de887cc7`.
That artifact remains available, but the prepared installer now targets the
unpublished 0.3.0 artifact. Do not deploy the prepared installer until the
matching release is public.

CLI 0.3.0 is **Prepared**, not yet published. It adds the shared creation
service, private portable creation provenance, Tohseno Studio, structured
progress, filesystem observation, native Simulator run/preview services, and
the pinned `serve-sim` browser bridge. The final two local builds were
byte-identical at
SHA-256
`0482ac2e9f80468528f272ac43c7e4ad4bc60ef8947733dfe077784861ce7d43`.
`apps/site/public/install.sh` pins that exact prepared digest.

For the next release:

1. Land the implementation commit containing the CLI and exact factory inputs.
2. From that exact source, run `bun run check` and `bun run tohseno:release`.
3. Compare the generated checksum with `CLI_SHA256_DEFAULT` in
   `apps/site/public/install.sh`; if it differs, update it and repeat the gate.
4. With explicit publishing approval, create the versioned GitHub release and
   upload the two unmodified artifact files.
5. Download the public tarball, verify its SHA-256, and run the installer in a
   temporary home.
6. Only a follow-up commit may change `TOHSENO_PIN` or turn `/oneshot.sh` into
   a thin delegator to an already-published CLI.
7. With separate site-deployment approval, run `railway up` and verify
   `/healthz`, `/install.sh --help`, and an isolated install.

The prepared 0.3.0 publication command is:

```sh
gh release create cli-v0.3.0 \
  dist/tohseno-cli-0.3.0.tar.gz \
  dist/tohseno-cli-0.3.0.json \
  --target <release-commit> \
  --title "TOHSENO CLI 0.3.0" \
  --notes "Local-first Tohseno Studio, one shared shot factory, portable private provenance, structured progress, and native Apple Simulator run and interactive preview."
```

No package registry publication is required by this design.

## Legacy oneshot boundary

`apps/site/public/oneshot.sh` retains the exact last published rails-creator
pin and creates nothing. Default invocation prints the canonical installer and
exits `2`; `--help` exits `0`. It remains `must-revalidate`.

The pin always trails the serving commit by one. CLI 0.2.2 is published, but
this endpoint remains an intentional migration notice and its legacy creator
pin is unchanged. Shell must never regain its own template copier, manifest
validator, shot creator, or agent launcher.

Before a site deployment:

```sh
bash -n apps/site/public/oneshot.sh
bash apps/site/public/oneshot.sh --help
bash apps/site/public/oneshot.sh; code=$?; test "$code" -eq 2
bun run check
```

## Shot production boundary

The public site does not deploy generated apps. Inside a shot, the implemented
read-only operation is:

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
