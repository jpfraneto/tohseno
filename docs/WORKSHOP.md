# The TOHSENO workshop

The workshop is the small communal loop around an app:

```text
existing or new source -> accepted Evolution -> share source -> local rebuild
                                                     |
Builder admits feedback <- exact-version packet <- tester experiences it
```

It is intentionally not a marketplace, public protocol publication, App Store
submission, ownership transfer, descendant, or fork. The signed Shot and
Version records retain their protocol meanings. The workshop capsule and
feedback packet are noncanonical transport envelopes around those records.

## 1. Adopt existing source

An existing repository can become a TOHSENO Shot, but `adopt` is the sealing
boundary—not a claim that every Xcode project already conforms unchanged. The
current Apple profile requires iOS 17+, SwiftUI, Apple frameworks only, no
third-party runtime packages or executable build phases, the exact pinned
Apple Fascia, and exact declarations for evidenced capabilities and network
use.

The current repository folder, `.xcodeproj`, shared scheme, app target/product,
and resulting `.app` must use one lowercase TOHSENO-safe slug. A different
human-facing name may remain in `CFBundleDisplayName`.

Before adoption, the source must contain:

- exactly the five pinned Swift files under `TohsenoFascia/`, all in the app target;
- `TOHSENO/fascia.json` and `TOHSENO/embedded-provenance.json` as app resources;
- an app initialization call to `InstallationIdentity.shared.prepare()` or `descriptor()`;
- `TOHSENO/capabilities.json` when the built source evidences capabilities beyond local storage;
- no private keys, recovery phrases, telemetry, tracking, undeclared endpoint, or unsupported build machinery.

From the repository root:

```sh
tohseno adopt
```

`adopt` creates the private `.tohseno/` ledger and seals the first immutable,
signed Evolution after running its verification gates. Do not manufacture or
edit `.tohseno/` yourself. If a gate fails, repair the reported source or Xcode
condition and retry. Studio selects the app by its local library name and
verifies it automatically; bare `tohseno verify` inside the app folder is an
advanced fallback, not a normal product step.

## Product surface and registry boundary

The intended user flow belongs in Studio:

1. The Builder selects an app by its local library name.
2. **Submit for feedback** verifies the latest accepted Version and publishes
   its licensed source submission.
3. Another developer opens that registry entry, builds it locally, and tries it.
4. Their exact-version response returns to the Builder's Studio for review.

The current source implements the local capsule, local rebuild, and reviewed
feedback primitives described below. It does not yet implement registry RPC,
artifact hosting/catalog lookup, or publication receipts, so the CLI transport
must not be described as a completed registry submission.

The on-chain `ShotRegistry` cannot be the downloadable app catalog by itself.
Under ADR 0006 it stores only Shot identity, controller, a public-checkpoint
head, checkpoint count, and nonce. It deliberately stores no source, app
listing, feedback, private intention, or runtime data. The one Studio action
therefore needs two coordinated results: a content-addressed off-chain catalog
entry for the licensed workshop source, and the narrow on-chain public witness.
Studio should report success only after the required receipts verify.

## 2. Low-level source submission primitive

Add and review a top-level `LICENSE`, `LICENSE.md`, `LICENSE.txt`, `COPYING`,
`COPYING.md`, or `COPYING.txt`, then seal that source in an Evolution. Sharing
fails closed without one of those files.

```sh
tohseno share <app-name> --output <app-name>.tohseno-workshop
```

The one-file capsule contains:

- the exact files committed by the latest accepted source-tree digest;
- the reviewed top-level license;
- the signed Shot record, conformance report, and embedded Version metadata.

It excludes:

- the private `.tohseno/` ledger, prompts, feedback, plans, logs, and terminal history;
- Builder identity keys or recovery material;
- retained or publisher-built `.app` binaries.

The capsule is bounded, canonical JSON with per-file lengths and SHA-256
digests. Its extension is `.tohseno-workshop`; the extension does not make it a
new protocol record.

## 3. Verify, rebuild, and try locally

The tester runs:

```sh
tohseno try <app-name>.tohseno-workshop --output <app-name>-workshop
```

Before Xcode runs, TOHSENO checks the capsule's canonical encoding, Builder
authorization, signature, record bindings, conformance state, license, safe
paths, source sizes, each file digest, the complete source-tree commitment,
and the required Apple project anatomy. It then builds that verified source on
the tester's Mac and installs the tester-built `.app` in Simulator.

`--no-launch` stops after verification and materialization for source review.
The tester never has to trust or execute a publisher-supplied binary. A local
receipt binds the materialized directory to the exact Shot, Expression, and
Version; changing the source invalidates feedback creation from that receipt.
Trying an app never acquires ownership or creates a descendant.

## 4. Return exact-version feedback

The tester creates a small packet from the unchanged materialized workshop:

```sh
tohseno feedback \
  --workshop <app-name>-workshop \
  --text "What I observed" \
  --author "Display name" \
  --output response.tohseno-feedback
```

The packet is bound to the receipt's exact Shot ID, Expression ID, Version ID,
and ordinal. Its author name is explicitly `self_declared`; the packet is not
an authority signature.

After reading it, the Builder may admit it:

```sh
tohseno feedback <app-name> --packet response.tohseno-feedback
```

Admission fails if any identity or Version binding differs. A successful
admission creates the Builder's private signed Feedback action. The Builder can
select that returned action commitment during a later Evolution. This review
boundary keeps the loop communal without letting an arbitrary packet mutate a
Shot automatically.

## Why source, not a shared binary?

A publisher-built binary is convenient, but testers cannot establish from the
binary alone that it corresponds to the offered source without a reproducible
Apple build and a trusted distribution/signing path. The workshop avoids that
claim: it authenticates the accepted source, then each tester builds the binary
locally. App Store or other binary distribution remains a separate future
boundary.
