# ADR 0024: `.tohseno` is app-local durable state with explicit private Git exclusions

Status: accepted

Date: 2026-08-26

## Context

The app is an ordinary Git repository, but its identity and accepted history
also live beside the source under `.tohseno/`. Describing that entire directory
as disposable or gitignored makes the repository look complete while omitting
the state that makes it one continuing TOHSENO app.

The opposite blanket rule is unsafe. Current lineage may contain inline exact
intention text and intentionally-private actions. References, feedback,
execution receipts, harness logs, local paths, and planning evidence are also
private working material. Automatically making all of `.tohseno/` trackable
would turn an ordinary repository push into an accidental publication path.

Git tracking and TOHSENO public-registry publication are separate decisions.
Neither one may silently upgrade material declared intentionally private.

## Decision

The `.tohseno/` directory is an integral, durable part of each app folder. It
MUST NOT be ignored with a blanket `.tohseno/` rule.

TOHSENO manages an exact allow-by-default Git boundary inside that directory:

- `app.toml`, the rebuildable `shot.json` state summary, `expression.json`,
  `capabilities.lock`, `protocol-version`, and safe immutable Evolution
  structure remain Git-visible;
- exact intention and evolutionary-intent documents, inline-private
  `lineage.jsonl`, private Genome/ownership/import/verification views,
  references, feedback, execution state, incomplete attempts, retained
  artifacts, previous source, and logs remain explicitly ignored; and
- `.tohseno/private/` remains wholly ignored.

The engine continues to exclude `.tohseno/` from an app Version's source-tree
commitment. That exclusion prevents self-reference and distinguishes app source
from TOHSENO metadata; it does not mean the metadata directory is disposable
or absent from Git.

Public source submission MUST use a reviewed, allowlisted export or future
registry publication flow. Recursive repository upload is not publication
authorization, and a Git-visible file is not automatically public protocol
material.

## Consequences

Moving or cloning a repository can preserve safe app identity and integrity
context instead of carrying only source. Private working state remains local
unless the owner uses an explicit verified export designed for it.

Existing app repositories converge when TOHSENO next initializes their
managed `.gitignore` block: previously ignored safe views become visible;
private exclusions remain. Owner-written ignore rules outside the managed
block are never removed. An owner may independently choose a stricter ignore,
but that repository will not carry the safe durable metadata described here.
