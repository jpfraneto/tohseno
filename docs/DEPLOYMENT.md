# Deployment (the public site)

The site is one stateless Bun process serving static pages and the pinned
oneshot script. No database, no volume, no secrets beyond the base URL.

These instructions prepare or execute a deployment only when the owner
explicitly chooses to do so. Nothing here deploys production, alters DNS,
creates paid resources, or submits applications to stores on its own.

## Topology

- One container from the repository `Dockerfile` (non-root `bun` user).
- Environment: `NODE_ENV=production`, `PORT`, `BASE_URL=https://tohseno.com`,
  `TRUST_PROXY=true` behind Railway's proxy.
- Health check: `GET /healthz`.
- `railway.toml` configures the Dockerfile builder, health check path, and
  restart policy.

## Releasing

1. Land the release commit; `bun run check` must be green.
2. Land the pin-bump commit (see "Release discipline for the oneshot pin" in
   `AGENTS.md`) — the pin always trails the serving commit by one.
3. Push the pinned commit to the public GitHub repository (the oneshot clones
   it) and deploy the site with `railway up` from the repo root.
4. Smoke-test: `curl -fsSL https://tohseno.com/oneshot.sh | bash -s -- --dry-run`.

Generated apps deploy through their own prepared TestFlight path
(`bun run setup`, then `fastlane beta` run by the owner) — never through this
site. With explicit owner approval, an agent may prepare local config with
`bun run setup --from-manifest --team auto`; App Store Connect credentials are
accepted only by path/environment and validated read-only before config is
written.
