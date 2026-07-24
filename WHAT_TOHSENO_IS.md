# What TOHSENO is

TOHSENO is a local intention compiler and open app factory.

Give it one private intention. It produces an independently owned native iOS
repository you can run, inspect, change, or leave behind. Then take another
one.

## Why it exists

The useful question is often not “is this idea worth months?” but “what does it
feel like when it exists?” TOHSENO makes a working prototype cheap enough to
answer that question without turning the prototype into a rented cloud
workspace.

Most shots miss. A bounded, owned app is still useful evidence.

## What a shot is

A shot is an independent SwiftUI application repository. New shots do not all
start as the same product. They start from a deterministic composition:

```text
private intention
      ↓
sanitized plan
      ↓
neutral iOS kernel + template + ordered app skills
      ↓
coding-agent work
      ↓
pinned verification + native run evidence
```

The neutral kernel supplies only a compiling app shell, design tokens,
configuration, tests, a static site, and build operations. Templates supply a
bounded experience shape. Skills supply versioned capabilities such as local
progress or sharing. Each layer declares its files and digest; the resulting
lock is verifiable without trusting the conversation that created it.

## What AI does—and does not do

The selected coding-agent provider interprets meaning once, at the plan
boundary. Its output is strict JSON limited to the installed catalog. The raw
intention is never accepted as tracked plan text.

After approval, composition and runtime invariants are deterministic. AI does
not decide file ownership, lock hashes, storage identity, verifier outcomes, or
whether an external action is authorized. If planning fails, TOHSENO uses the
Blank template and the same provider later; it does not silently route private
input elsewhere.

## What stays yours

The repository, source, history, manifest, composition lock, tests, landing
page, and pinned operating tools leave together. A later TOHSENO release cannot
rewrite them. There is no TOHSENO account or generated-app content backend.

Raw intentions and references stay in gitignored local provenance. The selected
coding agent may read them under its provider’s privacy and retention terms.
Tracked files contain only the sanitized plan and content-free digests.

External authority remains with the owner. TOHSENO can prepare and inspect
deployment work, but it does not spend money, create accounts, alter DNS,
publish packages, submit to an app store, or perform an irreversible operation
without explicit approval.

## Two doors, one factory

`tohseno` is the direct terminal flow. `tohseno studio` is the loopback-only
contact sheet. Both use the same release catalog, planner, composition engine,
allocator, private provenance, atomic publication, pinned verifier, and
Simulator service. Studio is a local view, not infrastructure a shot depends
on.

## The compatibility line

Continuity-v1 is an implemented legacy architecture: BIP39 identity,
crash-safe writing, local API/SQLite, module flags, and its operational rails.
Existing shots retain that behavior through their pinned release. Those
capabilities are not universal properties of new generic apps.

The generic factory is the current default. It is private-by-default and
account-free-by-default, but manifests may truthfully declare other data,
identity, integration, entitlement, or irreversible mechanics. Those are
product decisions, not moral refusals.

## The honest boundary

**Implemented:** generic manifests; a neutral SwiftUI kernel; Blank and Daily
Game templates; four real app skills; deterministic composition and locks;
CLI and Studio planning; independent repositories; post-agent privacy and
integrity verification; native build, Simulator launch, and screenshot
capture; legacy continuity verification.

**Prepared:** production inspection and operator instructions that do not
perform an external action.

**Proposed:** automatic production deployment, monitoring, recovery,
TestFlight submission, TokenMint, and SessionLink.

**Open:** any decision that expands disclosure, cost, ownership, or external
authority beyond the accepted manifest and owner approval.
