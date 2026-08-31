# Owner approval — Claims activation 1

On 2026-08-31, after the existing same-Mac authority-key custody deviation was
explained, the repository owner wrote, verbatim:

> Go ahead with the Claims activation.

That instruction authorized threshold signing and client activation of the
exact `tohseno.claims-activation/1` observation whose signing digest is
`0xec418380f588b9a6f72fc251b7a0ae7bee8a19a1d843017e4733ebd2d094966d`.
The resulting 2-of-3 envelope is `signed-claims-activation-1.json` in this
directory.

This approval does not enable either production relayer. Claims writes remain
closed until the owner-attended physical acceptance required by ADR 0035 and
the activation runbook is complete.
