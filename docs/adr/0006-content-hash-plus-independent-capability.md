# ADR 0006: Address customer Markdown by hash and protect it with an independent capability

- Status: Accepted
- Date: 2026-07-10

## Context

Customer Markdown needs a stable integrity identifier for order/contracts, but publishing it at a content-hash URL would turn a guessable/reusable digest into authorization and leak private source through references.

## Decision

The intake computes a SHA-256 content hash over the accepted Markdown bytes. Access is protected by an independently generated bearer capability containing at least 256 bits of cryptographically secure randomness. Only a one-way hash of the capability is persisted.

The Markdown is encrypted at rest. It is never served from a public content-hash route, and the content hash is never sufficient to retrieve it.

## Consequences

- The same content can retain a known integrity digest while access is revoked or expires.
- A leaked database does not directly reveal raw capability URLs.
- The raw capability is returned/delivered only at creation and must not enter logs, payment metadata, analytics, subjects, or referrers.
- Invalid, revoked, and expired capabilities share a `404` response.
- Hash/digest migrations and capability rotation have independent lifecycles.

## Non-goals

This ADR does not claim capability URLs resist disclosure by a compromised recipient/browser/server. Bearer possession is authorization until revocation or expiry.
