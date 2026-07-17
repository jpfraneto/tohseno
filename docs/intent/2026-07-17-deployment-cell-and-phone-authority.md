# Intent distillation: deployment cell and phone authority

- Public record ID: `intent-public-2026-07-17-deployment-cell-and-phone-authority`
- Recorded: 2026-07-17
- Parent record: `intent-public-2026-07-17-software-seed`
- Status: distilled into proposals and an audit; not approved runtime behavior
- Canonical private source: not committed to this public repository

## Owner direction

The generated backend must be as contained as possible: one package, built and
deployed with one owner-run command, plug-and-play. The likely shape is one
Docker/OCI container per generated application on a VPS, potentially with a
bounded CPU, memory, storage, and network "cell" allocated per application.

Every POST to the backend should be authorized by the phone-held key,
including requests that originate in a browser.

## Distillation

The one-package direction is distilled into
[Deployment cell](../proposals/DEPLOYMENT_CELL.md): one OCI image per generated
backend, secrets outside the image, one persistent data volume, one
Compose/project definition per application, migrations, health, backup,
rollback, and ejection included, per-application network, Unix identity, and
resource limits, and an explicit statement of when separate VMs are required
because containers are not a sufficient boundary for untrusted multi-tenant
isolation. The containerized backend is distinct from native mobile binaries,
which are built and distributed through platform toolchains, not the cell.

The signing direction is audited literally in
[POST route authority audit](../POST_AUTHORITY_AUDIT.md) and refined in
[Phone-to-browser bridge](../proposals/PHONE_BROWSER_BRIDGE.md) into the
defensible invariant:

> Every application-authorized mutation after contextual identity exists must
> have a verifiable authorization chain rooted in the phone.

Public bootstrap and pairing requests exist before a phone key does, and
provider webhooks are authorized by the provider's own signature; neither can
be phone-signed, and the record says so rather than overclaiming.

## Boundaries preserved in the distillation

- Seed and recovery material never enter a QR code, browser, extension,
  server, URL, or log. A mnemonic is recovery material, not one universal
  private key.
- Practice identity, owner/update authority, browser delegation, release
  signing, and financial or network execution keys remain distinct roles.
- The choice between literal phone co-signing of every request and a
  phone-signed scoped delegation with step-up approval is recorded as an open
  owner decision, not resolved here.
- One cell serves one application; the control plane still observes health and
  order state, not end-user continuity content.
- No paid resource, DNS action, production credential change, deployment,
  package publication, or external network action is authorized by this
  record.

## Organized work produced from this intent

1. Classify every existing POST route by its true authority and keep the
   classification current as routes change.
2. Specify the deployment cell: image, Compose/VPS layout, resources, secrets,
   migrations, ingress, health, rollback, backup, and ejection.
3. Decide co-signing versus scoped delegation plus step-up (open question 16)
   before implementing verification middleware.
4. Then implement contract-first: operation definitions, delegation and
   envelope contracts, typed audit events, a production verifier, durable
   replay protection, and negative fixtures.
