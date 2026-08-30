# ADR 0031: Clean-Mac acceptance may use an explicit public release candidate

Status: accepted

Date: 2026-08-30

Supersedes:

- The native distribution runbook only where it requires clean-Mac acceptance
  before any public download origin exists. Stable promotion, exact-byte
  verification, fail-closed configuration, and every product acceptance gate
  remain unchanged.

## Context

ADR 0030 makes the public website's direct download the consumer installation
door. Testing a privately transferred DMG does not test that door, its redirect,
or the bytes delivered by the public origin. Requiring clean-Mac acceptance
before the public path exists therefore makes the real installation path
impossible to accept.

## Decision

After explicit owner authorization, the operator may publish an exact native
candidate as a public GitHub prerelease and temporarily point
`/download/macos` at it for independent clean-Mac acceptance when all of these
are already true:

- the candidate was built from a recorded clean commit;
- its Developer ID identity and nested signatures verify;
- Apple notarization was accepted and the ticket validates;
- the exact DMG SHA-256 is recorded and matches a fresh origin round trip;
- the release is tagged and labeled as a release candidate rather than stable;
- the website metadata and visible download detail say **Release candidate**;
  and
- the protected stable tag remains unpublished.

The machine-readable distribution response and redirect header identify the
active channel as `release-candidate` or `stable`. A release-candidate
activation is distribution for acceptance, not stable promotion and not proof
that clean-Mac acceptance passed.

If acceptance fails, the operator disables the download and preserves the
candidate as rejected evidence. It never replaces bytes at the existing tag or
URL. A repaired build uses a new release-candidate tag and repeats signing,
notarization, hashing, origin verification, and acceptance.

Stable promotion uses the exact accepted digest, publishes the protected stable
tag only after the remaining release gates pass, switches the channel to
`stable`, and repeats the public download and post-activation checks.

## Consequences

A clean Mac can test the same `tohseno.com` button, redirect, DMG, Finder drag,
Gatekeeper open, and first-run path that a future stable user will use. The
website remains truthful while that test is in progress.

This decision authorizes only the bounded release-candidate acceptance channel.
It changes no protocol encoding, schema, Shot, Evolution, Builder, Registry,
identity, receipt, billing, managed inference, or stable-release gate.
