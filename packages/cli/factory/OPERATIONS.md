# Shot operations

These are pinned coding-agent and automation rails. Normal owners use
cwd-independent commands such as `tohseno verify <shot>` and
`tohseno run <shot>`. After ejection, the repository-local low-level machine
remains available:

```sh
bun .tohseno/machine.ts operations --json
```

JSON mode writes one protocol document to stdout. Diagnostics go to stderr.
Exit codes are stable: `0` success, `2` invalid configuration, `3` missing
dependency, `4` unhealthy service/readiness failure, and `5` internal failure.
Never turn agent prose into evidence.

## Generic app shots

Metadata-v2 shots use `app.manifest.json`. Its `operations` object names the
Xcode project, scheme, and product; the machine validates those names before
use. The neutral kernel has no required backend or development service.

Verify the manifest, exact composition lock, immutable installed capability
files, acceptance metadata, private ignore rules, independent Git boundary,
and absence of copied private input:

```sh
bun .tohseno/machine.ts verify --json
```

Inspect or launch the native app:

```sh
bun .tohseno/machine.ts ios inspect --json
bun .tohseno/machine.ts ios launch --json
```

`ios launch` requires full Xcode and an available iPhone Simulator. It builds
the declared project and scheme into gitignored DerivedData, boots the exact
Simulator UDID, installs the built app, reads the bundle identifier from the
actual app bundle, launches it, and returns structured evidence. It does not
require a paid developer account for Simulator use.

The global owner-facing equivalents work from any directory:

```text
tohseno verify <shot>
tohseno run <shot>
```

Do not hand a normal owner the repository-local Bun commands.

## Legacy continuity shots

Metadata-v1 repositories retain their historical continuity manifest and
pinned release behavior. Their `dev start` operation owns the Bun/SQLite
development service, migrations, health checks, endpoint injection, structured
logs, and optional Quick Tunnel. `ios launch` requires that service to be
healthy because those apps deliberately depend on it.

Do not inject generic templates or app skills into a legacy shot. Do not remove
its BIP39, writing, persistence, backend, or production declarations merely to
make it resemble metadata v2.

Quick Tunnels remain public development reachability, not authentication or
production infrastructure. Never use one in Release, production, a store
archive, or DNS.

## External authority

Generic manifests may declare irreversible operations, but this machine does
not invent or execute them. Accounts, credentials, purchases, deployment,
signing, TestFlight, App Store submission, DNS changes, token launch, and
other external consequences require explicit owner approval.

Legacy releases may expose their historical owner-approved token operation.
Follow the legacy manifest and pinned instructions exactly; never request or
relay OTPs, private keys, or credential contents, and never treat a build
request as financial authorization.

Production inspection for legacy continuity shots is read-only. Generic app
shots currently declare production readiness in `app.manifest.json`; broad
provisioning, monitoring, recovery, and store submission remain unimplemented.
