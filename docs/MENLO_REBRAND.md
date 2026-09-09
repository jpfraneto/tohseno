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

Upload funding is not implemented by this visual change. Inspection confirmed
that the current Registry relayer pays transaction gas, while BuilderAccount
has neither a payable receive function nor a spending entrypoint. Its address
must not be presented as an ETH funding wallet. The owner has been asked for
any existing funding design. A usable funding mechanism, exact one-upload
subsidy enforcement, and the native funding-address/copy interface remain work
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
