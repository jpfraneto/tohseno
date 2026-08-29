# ADR 0029: The first Shot precedes the factory

Status: accepted

Date: 2026-08-29

Supersedes:

- ADR 0028 only where its first-open invitation is a passive empty-library
  composition with no persisted state. ADR 0028's Finder-first installation,
  exact welcome language, and conventional DMG handoff remain accepted.

## Context

A person opens TOHSENO to turn an idea into an app. Showing the library and its
navigation before that first intention makes the product explain its storage
before delivering its purpose. The TAKE A SHOT invitation therefore needs to
be the real creation surface, not a button that leads to another screen.

Images are part of an intention. The existing creation command already accepts
bounded PNG and JPEG references, but first open did not expose that capability.

Some people still need to inspect the factory before creating anything. That
choice must be explicit, small, and remembered instead of becoming a recurring
obstacle.

## Decision

After the existing Mac and iPhone readiness checks, a workspace with no Shot
records opens a full-window first-Shot surface before the library, Registry,
sidebar, or app workspace. It says exactly:

```text
WELCOME TO TOHSENO
TAKE A SHOT
This is where your ideas transform into apps.
```

That surface is the existing creation draft and existing create command path.
It contains the intention editor, plain-Return submission, Shift-Return
newline behavior, and a primary **Create App** action. Successful submission
creates the durable Shot and naturally reveals the factory; there is no
planning round trip or second implementation path.

The same surface accepts up to eight reference images through either the
native file picker or drag and drop. References use the existing validation:
each must be a regular, non-symlink PNG or JPEG no larger than 64 MB, and the
exact accepted bytes enter the ordinary creation draft. An invalid or ninth
reference fails visibly without submitting a partial creation command.

A secondary **Skip** action sits beside **Create App**. Choosing it stores one
local preference and reveals the ordinary empty library. Only an explicit
Skip persists this bypass. Once any Shot record exists, including a retired
Shot preserved in the workspace, the first-Shot gate does not return.

## Consequences

The first useful interaction is the product itself: one intention, optional
visual references, and one send action. The factory remains available without
forcing creation, but entering it empty requires a conscious Skip.

The persisted value is interface preference only. It is not a Shot, relay,
Builder record, qualification, account, protocol fact, or publication event.

This decision changes no public protocol encoding, schema, frozen vector,
factory command, bounded implementation rule, Registry authority, billing
activation, signing, notarization, release gate, artifact pin, or external
publication state. It authorizes no upload or production activation.
