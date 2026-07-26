# Shot operations

These are pinned coding-agent and automation rails. After ejection, the
repository-local machine remains available:

```sh
bun .tohseno/machine.ts operations --json
```

JSON mode writes one protocol document to stdout. Diagnostics go to stderr.
Exit codes are stable: `0` success, `2` invalid configuration, `3` missing
dependency, `4` unhealthy service/readiness failure, and `5` internal failure.
Never turn agent prose into evidence.

## App Shots

Shots use `app.manifest.json`. Its `operations` object names the Xcode project,
scheme, and product; the machine validates those names before use. The neutral
kernel has no required backend or development service.

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

After the prepared managed artifact is published and installed, its
owner-facing equivalents work from any directory:

```text
tohseno verify <shot>
tohseno run <shot>
```

The 0.5.0 artifact is not yet published, so source review and ejected operation
use the explicit Bun commands above.

## External authority

App manifests may declare irreversible operations, but this machine does
not invent or execute them. Accounts, credentials, purchases, deployment,
signing, TestFlight, App Store submission, DNS changes, token launch, and
other external consequences require explicit owner approval.

Shots declare production readiness in `app.manifest.json`; broad
provisioning, monitoring, recovery, and store submission remain unimplemented.
