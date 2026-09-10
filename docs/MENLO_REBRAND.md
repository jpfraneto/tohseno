# Menlo implementation record

The owner’s September 9, 2026 instruction expands the supplied landing-only
brief to the Mac and Companion appearance, production delivery, and a separate
one-upload subsidy followed by ETH-funded uploads. That explicit instruction
supersedes the ZIP’s landing-only restriction. Existing protocol encodings,
contract addresses, package/CLI names, bundle identifiers, pairing and Apple
signing authority remain unchanged.

The supplied `menlo-website-handoff-v1.zip` is the visual source. The homepage
uses its original artwork and isolated paper/ink/green tokens. It reads verified
Registry records, links to canonical app pages, retains real network activity,
and explains recipient-local Xcode signing. Empty and unavailable feeds do not
remove setup instructions. The native views, visible names and app artwork use
the same identity. Registry and app detail routes retain their existing styling.

Local evidence: website TypeScript check and 35 Registry/HTTP tests passed;
command-copy success and failure were exercised with the shipped script.
Companion’s 58 tests passed. Mac’s 45 tests ran; branding expectations were
updated and the affected test rerun. Native workshop renders were inspected.
These are fixture renders, not physical installation evidence. Browser discovery
returned no available browser, so the specified responsive screenshots and
browser keyboard/clipboard walkthrough remain unverified.

Paid upload funding is not implemented yet. Inspection confirmed
that the current Registry relayer pays transaction gas, while BuilderAccount
has neither a payable receive function nor a spending entrypoint. Its address
must not be presented as an ETH funding wallet. The owner has been asked for
any existing funding design. A usable funding mechanism and the native funding-address/copy interface remain work
in progress. No price, deposit address, or payment success is invented here.

Production and native artifact evidence will be appended after verification.

The visual source is committed on `main` at `9bd350d`; `d3fb225` adds versioned
homepage assets after a real CDN negative-cache observation. Railway deployment
`c0aa1ff8-5fa4-4d7e-8ae2-b6ba911fc8f0` served the Menlo homepage and one real
Anky card. Health, canonical app detail and Registry responses were fetched;
the two non-landing pages retain Tohseno styling. The versioned hero bytes
matched the supplied asset at SHA-256
`b246f403e06084356dab09056ec4f818e1a00eedee4779b1174531f7055d821e`.
The asset-cache fix deployment is `a7d17621-e026-4e02-b1b5-b3bdb0151fc1`.

A universal Mac bundle was assembled from the native source at `9bd350d` and
passed the unsigned package-integrity check. Packaging metadata was set to
version 1.2.1, build 10008, with `TohsenoSourceCommit=9bd350d`. Its Developer ID
signature verified, and notarization submission
`8d6475e2-ac03-4804-b807-ebd2c9877809` was submitted. Submission alone is not
notarization acceptance or public artifact activation.

The server now reserves one upload subsidy per verified Builder address before
any relayer gas spend. An exclusive, synced reservation survives retries and
concurrent jobs. Existing catalog uploads count against the allowance; history
read failures do not grant gas. The same job may continue, but later uploads
stop with a clear funding-required explanation while the paid path is absent.
Four subsidy tests plus 15 Registry trust tests pass. This is implemented/local
evidence until the corresponding production deployment is recorded.

The app notarization and the separate DMG notarization both completed Accepted.
DMG submission: `1229529b-2eb9-476b-8339-8e4048fac653`. Both tickets were stapled
and validated, and Gatekeeper accepted the app and disk image. Mounting the
finished DMG verified the enclosed signed app, Menlo display name, build 10008,
and native source stamp. The GitHub prerelease `v1.2.1-rc.1` is published with
`Menlo-1.2.1-rc.1.dmg`. Its downloaded origin bytes match SHA-256
`94a2a9a6ac0d6765a8127a169a1c7037df4822a55a310bc6fdbf2d0ad5002dd7`.
This remains a release candidate; no clean-Mac or physical-iPhone acceptance
has been inferred from these checks.

Railway deployment `a7d17621-e026-4e02-b1b5-b3bdb0151fc1` succeeded and served
the versioned asset URLs. Subsidy deployment
`680f948c-1287-4730-b521-1501eb4d80b3` also succeeded. All 134 website tests
passed, including subsidy and Registry checks. No second-upload transaction
was attempted with a real Companion, and the paid-wallet path remains absent.

Production download activation was observed after deployment
`9da19cfe-0c94-4257-93a9-408093172440`: `/api/distribution/v1/macos` reports
version `1.2.1-rc.1`, build `10008`, the release-candidate channel and the exact
GitHub artifact URL/digest above. A fresh download through
`https://tohseno.com/download/macos` matched that same SHA-256. The final
homepage response contained the Menlo title, versioned hero and command, plus
one canonical app card; `/healthz` returned healthy. The deployed CSS and
JavaScript also matched local bytes.

Completion remains open: selecting/implementing paid-wallet custody and
funding, displaying its real address with Copy in the native apps, responsive
browser acceptance (no browser connected), and physical native acceptance.
Do not treat the published visual candidate or the subsidy cap as completion
of the full paid-upload request.

## Public documentation follow-up

Commit `f72986f` updates the standalone docs to Menlo: product branding and
palette, distribution-first introduction, actual CLI/setup instructions,
current candidate and live service status, and the explicit distinction
between the deployed subsidy cap and unfinished paid-wallet UI. It also
updates README and the leading STATE snapshot; frozen protocol files and
technical identifiers are unchanged.

Astro check reported zero errors or warnings. Build and the verifier passed
for all 40 guide pages, internal links, Pagefind, sitemap and AI-readable feeds.
Cloudflare Pages production deployment `8a205d2f` completed for `tohseno-docs`.
Fresh requests to `docs.tohseno.com` verified the Menlo homepage, current-status
page, setup instructions and `llms.txt`. This is live response evidence, not a
claim of browser screenshot or physical-device acceptance.
