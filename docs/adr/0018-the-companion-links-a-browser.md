# ADR 0018: The Companion links a browser; the website never holds an account

Status: accepted, and deliberately not implemented

Date: 2026-08-19

## Context

The public entry point is a terminal that accepts a complete intention before
anything is installed. A person writes what their app should do, attaches
references, and is then offered somewhere to send it. Today exactly one
destination works: the ADR 0011 encrypted handoff, whose authorization is
possession of a one-time claim capability pasted on their own Mac.

That transport is a single-use transfer, not a session. Sending a second
intention repeats the whole ceremony. The obvious remedy — an account on the
website — is the wrong one, and it will keep looking like the right one to
whoever reads this surface next. ADR 0011 already rejected accounts, but it
rejected them for a one-time transfer. A repeat-use surface reopens the
question, and nothing currently written down prevents someone from answering
it with a password field.

The material for a better answer already exists. ADR 0015 and ADR 0016 make
the Companion the single signed remote interface: a recoverable BIP-39 device
identity, granular workspace-scoped grants that are revocable and checked
before both command admission and event delivery, and a scanner that already
accepts a `tohseno://pair/v1/…` invitation from a Mac.

Apps the factory produces are ordinary iOS apps holding no identity and no
grants. Because the released `TohsenoCompanionKit` can be vendored into a
Shot, "sign in with any TOHSENO app" is superficially plausible, which is
precisely why it needs a recorded answer.

## Decision

The website will never have an account. When repeat-use sending exists, it
will be the Companion linking a browser, and only the Companion.

1. **The phone is the identity.** Scanning links *this browser* to *that
   phone*. Every surface calls it linking. It is never called logging in,
   signing in, or an account, because each of those words invites the design
   this decision refuses.
2. **The browser holds a capability the phone issued, not a credential the
   site minted.** It is scoped, expiring, and revocable from the phone. Its
   possession is authority over nothing except handing that phone an intent.
   The site is not an identity provider and does not become one.
3. **Only the Companion.** An app produced by the factory must never carry a
   device identity or a workspace grant. Accepting "any TOHSENO app" would
   multiply the signed remote interfaces one owner holds by the number of apps
   they have ever made, and every one of them would be a separate thing to
   audit, expire, and revoke. One app means one identity and one revocation
   list.
4. **A phone is a remote control, not a factory.** A linked browser reaches
   only the Mac that phone is already paired with. Linking does not give the
   website the ability to build anything, and this decision authorizes no
   cloud factory.

## This is not built, and the surface says so

The Companion is not published. The production companion relay and APNs
activation remain fail-closed. The public installer still pins the authorized
release. The terminal's third door therefore names its own absence and hands
the person back to the Mac path rather than presenting a door that opens onto
nothing.

`CompanionModel.pair(scanned:)` accepts only `tohseno://pair/v1/…`. A browser
link is a different act from admitting a device to a workspace, so it must be
a distinct scheme and a distinct scanner path. Overloading the pairing code
would make one scan mean two things, which is exactly the confusion a person
cannot audit.

## What would have to be true first

Building this requires all of the following together. Any subset is not
evidence that this decision has been lifted:

1. a published Companion and an activated production companion relay;
2. a browser-link rendezvous distinct from the Mac pairing ceremony, which
   cannot be mistaken for it by a person or accepted in its place by the app;
3. a phone-issued browser capability with an explicit lifetime, a scope no
   broader than submitting an intent, and a revocation surface visible in the
   app;
4. a browser that never receives Builder identity material, workspace read
   capability, or any grant that outlives the phone revoking it;
5. relay observability unchanged: the relay must still be unable to read
   content or attribute it to an identity.

## Alternatives considered

- **An account, by email, password, or magic link.** Introduces identity,
  recovery, tracking, and a control surface, and makes the website the owner
  of something. Rejected in ADR 0011 for the single-use transfer and rejected
  again here for the repeat-use one.
- **Sign in with any TOHSENO app.** Rejected under decision 3. It also
  inverts ADR 0015's boundary by turning every output of the factory into
  another key-holding remote interface.
- **A long-lived token the website issues and stores.** Makes the site the
  identity provider under a different name.
- **Keeping only the one-time claim command.** This is the current state and
  remains correct until every requirement above is true. It is not a gap to be
  closed quickly; it is the honest surface while the phone does not exist.

## Consequences and residual risks

Until the Companion ships, the Mac is the only destination and the website
says so plainly rather than collecting interest in a door it cannot open.

A linked browser is a bearer surface. Cross-site scripting in the origin, or a
shared or compromised machine, exposes whatever capability the browser holds.
A short lifetime, a narrow scope, and phone-side revocation reduce that
exposure; they do not remove it, and they are weaker than the current
single-use command, which stops being useful the moment it is claimed.

Linking also increases what a QR scan can mean to a person. The app carries
the burden of presenting admitting a Mac and linking a browser as visibly
different acts.

Nothing here changes protocol schemas, identity derivation, canonical
encodings, acceptance rules, or the public witness. A relay record is still
never a Shot, and the website is still never the origin of one. No release,
relay activation, APNs credential, installer repin, or deployment is
authorized by this decision.
