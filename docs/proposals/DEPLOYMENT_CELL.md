# Proposal: deployment cell

- Status: **Proposed**
- Open: multi-tenancy trust tier, shared-ingress boundary, backup encryption
  key custody, and the exact resource defaults
- Does not implement: any image, Compose file, VPS provisioning, or backup
  system; nothing here creates paid infrastructure

## Product shape

A generated backend ships as one package that one owner-run command takes from
source to a running, healthy deployment. The unit of deployment is a **cell**:
one application's container, volume, network, Unix identity, resource limits,
and operational scripts, defined together and removable together. The owner
should be able to point at one directory and say "that is my application's
entire server-side existence."

This proposal covers the generated backend only. Native mobile binaries are
built and signed through platform toolchains, distributed through stores or
direct install, and are never part of the cell; the cell is what those
binaries talk to.

## One package, one command

The generated backend is one Bun package: source, migrations, static assets,
tests, a Dockerfile, a Compose project definition, and operational scripts.
One command — something in the spirit of `./cell up` — must:

1. build (or pull, once registries exist) the pinned OCI image;
2. verify required secrets are present outside the image, refusing to start
   with placeholders;
3. apply pending migrations transactionally against the persistent volume,
   refusing to run a newer schema than the code understands;
4. start the container and wait for the health check to pass;
5. report a content-addressed release identity (image digest plus source
   commit) that joins the release-revision history.

The same entry point provides `down`, `status`, `backup`, `restore`,
`rollback`, and `eject`. No step may require editing files inside a running
container or memorizing provider-specific consoles.

## Cell anatomy on a VPS

One Compose project per application, one directory per cell:

```text
/srv/cells/<app>/
  compose.yaml        # the only definition of this cell
  .env                # secrets, mode 0600, never in the image or Git
  data/               # the single persistent volume (SQLite, WAL, artifacts)
  backups/            # encrypted, rotated snapshots
  releases/           # release identity records and rollback references
```

Per cell:

- **Image**: one OCI image, pinned by digest, containing code and static
  assets only — no secrets, no data, no customer content.
- **Secrets**: injected at start from the cell's `.env` or a secret store;
  rotating a secret is a restart, not a rebuild.
- **Data**: exactly one volume. Everything durable lives there, so backup,
  restore, and ejection are statements about one directory.
- **Unix identity**: a dedicated non-root user per cell, both inside the
  container and as the ownership of the cell directory.
- **Network**: a per-cell network; the container publishes nothing directly
  and is reachable only through ingress. Egress is deny-by-default with named
  exceptions (payment provider, email provider) matching the manifest's
  declared boundaries.
- **Resources**: explicit CPU, memory, PID, and storage bounds so one cell
  cannot starve its neighbors. Exact defaults are open; their existence is
  not.

Health is the existing `/healthz` contract wired into the container health
check. Backup is a scheduled snapshot of the data volume (SQLite online
backup, then encrypt), restored and verified on a scratch cell as part of the
runbook, not assumed. Rollback is re-pointing the cell at the previous image
digest plus, when a migration is not backward-compatible, a documented
restore; a rollback is recorded as a new release revision, never as erased
history.

## Shared ingress, if it stays a clean boundary

A single reverse proxy per VPS may terminate TLS and route hostnames to
cells. It remains acceptable only while it is a pure connection router: no
shared authentication, no request rewriting beyond standard forwarding
headers, no logging of bodies, no cross-cell state. The application already
derives client identity and canonical-host behavior from forwarding headers
behind `TRUST_PROXY`; the ingress must set those honestly and nothing else.
If the proxy ever needs application knowledge, that is the signal to give the
cell its own IP or host instead. Which ingress implementation to standardize
on is open.

## When containers are not enough

Containers share the host kernel. Cgroups and namespaces bound resources and
visibility; they are not a security boundary against a hostile neighbor with
a kernel exploit. The tiers are:

1. **One owner's applications on one VPS**: containers per cell are
   sufficient; the threat is fault isolation, not malice.
2. **Multiple customers whose code TOHSENO generated and reviews**:
   containers plus hardened runtime settings are defensible, but this is a
   trust statement about the compiler and review process, and it must be
   made explicitly.
3. **Untrusted or customer-modified code**: containers are not a sufficient
   boundary. Each tenant needs a hardware-virtualized boundary — separate
   VPS/VM per tenant, or a microVM runtime — before co-residency is offered.

Anything that holds financial or external-action keys sits in tier 3
regardless of who wrote the code. Selling multi-tenant hosting below the
matching tier would be a silent ownership and disclosure decision, which is
why the tier choice is an open question, not a default.

## Ejection

The cell is the ejection unit. `eject` produces: the package source, the
image reference and digest, the Compose definition, the data volume snapshot,
the decrypted-by-owner backup, the evolution history the owner is entitled
to, and a runbook that stands alone on any Docker-capable host with no
TOHSENO control-plane dependency. A cell that cannot be ejected this way is a
defect, per the ejectable-from-birth contract.

## Acceptance evidence

- one command takes a clean host from source to passing health check;
- the image contains no secret, credential, or customer data (scanned, not
  assumed);
- kill and restart lose no committed data; the volume alone reconstructs the
  application on a fresh host;
- backup, restore, and rollback are each exercised in a rehearsal, and a
  rollback appears as a new release revision;
- a cell's user cannot read another cell's directory; a cell at its resource
  limit does not take a neighbor's health check down;
- egress outside the declared allowlist fails;
- ejection output runs on a second machine with TOHSENO absent.
