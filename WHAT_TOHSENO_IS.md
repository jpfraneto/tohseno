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

## Shot and Evolution

A Shot is one coherent software intention with a stable identity and an
independent SwiftUI application repository. An Evolution changes that same
Shot. It keeps the Shot ID and repository; it is not counted as another Shot.
New Shots do not all start as the same product. They start from a deterministic
composition:

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

Distribution is a separate three-state lifecycle:

- `EVOLVING` — not represented as publicly downloadable;
- `PUBLISHED` — downloadable through TOHSENO from published source;
- `APP_STORE` — shipped through Apple.

Evolutions may be recorded in any of these states. Creation progress, Simulator
readiness, and app runtime state do not change those definitions.

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
rewrite them. Each app declares its data behavior and starts from local,
account-free defaults. There is no TOHSENO account or TOHSENO-operated
generated-app content backend.

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
allocator, private provenance, atomic repository creation, pinned verifier, and
Simulator service. Studio is a local view, not infrastructure a Shot depends
on.

## The optional public layer

Builder-signed records can append deliberately public Evolutions, lifecycle
transitions, and deployment-agnostic Appcoin links. Registries deterministically
project one accepted history; nodes are replaceable indexes rather than
ownership or consensus authorities. Each signature is a portable Builder
attestation over declared claims, not independent proof that those claims are
true or globally preferred. The reference node has no designated endpoint or
record field for generated-app runtime content. Protocol policy limits it to
deliberately submitted public records, whose summaries still require Builder
review.

Builder identity, generated-app runtime identity, release signing, Apple
credentials, and external-action authority are distinct roles. The included
local signer proves the interface in tests; it is not a production recovery or
key-custody policy.

No public protocol component is needed to make, evolve, build, run, or eject a
local Shot.

The factory is private-by-default and
account-free-by-default, but manifests may truthfully declare other data,
identity, integration, entitlement, or irreversible mechanics. Those are
product decisions, not moral refusals.

## The honest boundary

**Implemented:** the canonical app manifest; a neutral SwiftUI kernel; Blank and Daily
Game templates; four real app skills; deterministic composition and locks;
CLI and Studio planning; independent repositories; post-agent privacy and
integrity verification; native build, Simulator launch, and screenshot
capture; deterministic signed public Shot records; registry projection; and a
local reference node.

**Prepared:** the owner-ladder catalog and operator instructions that do not
perform an external action; the CLI does not project or execute that ladder.

**Proposed:** automatic production deployment, monitoring, recovery,
TestFlight submission, persistent Builder-key custody, and mobile signing.

**Open:** the production Builder trust root and cross-node resolution of
competing valid histories, plus any decision that expands disclosure, cost,
ownership, or external authority beyond the accepted manifest and owner
approval.
