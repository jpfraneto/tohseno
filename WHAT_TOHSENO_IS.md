# What TOHSENO is

## The short answer

TOHSENO is the shortest reliable path from an idea to an iOS app you can use.

Install one local tool. Take a shot. Tell your coding agent what to make. The
result is not a demo trapped inside a service: it is an independent repository,
a working SwiftUI app, a local runtime, a manifest, tests, and a path onto your
phone. Keep it, change it, share the source, or leave it behind.

Then take another one.

## The thing it makes possible

Software has usually asked people to decide too much before they can learn
anything:

- Is the idea worth months of work?
- Is it a business?
- Which stack, database, account system, and deployment platform should it use?
- Will enough people want it?
- Is this the one idea to bet on?

Those questions arrive before the most useful evidence: opening the thing and
using it.

TOHSENO changes that order. It makes a real prototype the beginning of thought,
not the prize at the end of a long planning process. An idea gets a body quickly
enough that taste, need, and behavior can answer what speculation cannot.

Most shots will miss. That is not a failure of the system. A cheap, owned,
working experiment is the payoff. It tells you what the idea feels like, what
you return to, and where to aim next.

## What a shot is

A **shot** is an independently owned iOS application repository.

Every shot begins from the same tested continuity base rather than an empty
directory. The base provides:

- a compiling SwiftUI app;
- BIP39 seed-phrase identity instead of an account screen;
- crash-safe local writing and recovery;
- exact owner-selected export of canonical content;
- a local Bun API with health and readiness checks;
- deterministic SQLite migrations for operational metadata;
- separate Debug and Release endpoint configuration;
- a manifest that states what the app does and where data can move;
- tests, setup, verification, production inspection, and runbooks;
- a static landing page that leaves with the app.

The coding agent mutates that base toward the owner's intention. The shot keeps
its own Git history and its own pinned factory/runtime tools. No symlink points
back to a TOHSENO checkout. A later CLI upgrade cannot silently rewrite its
critical behavior.

This is why “shot” does not mean disposable. It means cheap to try and complete
enough to keep.

## How TOHSENO works

There are two human doors into one factory:

1. `tohseno` is the fast terminal path. It creates or continues a shot and
   launches an installed Codex or Claude Code in that repository.
2. `tohseno studio` is a local contact sheet. It shows the same shots and can
   create, verify, run, and preview them from a browser bound to the Mac.

Both doors use the same allocator, immutable release, atomic publication,
private provenance, Git baseline, verifier, and Simulator service. Studio is a
view over the local filesystem, not a cloud control plane and not a dependency
of an existing shot.

The process is deliberately narrow:

1. Normalize the owner-selected local input.
2. Allocate a unique shot number.
3. assemble a private staging repository from an authenticated factory release;
4. validate the manifest and required structure;
5. create a neutral, independent Git history;
6. atomically publish the complete shot;
7. let the selected coding agent build;
8. verify privacy and integrity after the agent exits, even when it fails;
9. build and launch in Apple Simulator when the machine supports it.

If the post-agent gate finds copied private input, changed pinned machinery, an
unsafe link, or a missing ignore boundary, TOHSENO isolates the result under an
explicitly unsafe hidden path. It does not call that shot ready.

## The value it brings

### For a builder

TOHSENO compresses the distance between curiosity and evidence.

It removes repeated setup without taking ownership of the result. Identity,
persistence, local development, Simulator launch, verification, and production
inspection already have a shape, so the coding agent can spend its time on the
part that makes this app itself.

The builder does not have to learn a private platform, maintain a cloud
workspace, or ask permission to leave. The output is ordinary source and
ordinary Git.

### For the person using the app

The continuity base starts account-free and private by default. The identity is
a seed phrase stored in Keychain. Writing lives in protected files on the
device. The baseline backend has no route for that writing and TOHSENO operates
no central backend for generated-app content.

Those are defaults, not a refusal to build other mechanics. If a requested
feature fits the manifest, the builder decides how the app works. The important
part is that disclosure is declared and implemented deliberately instead of
arriving accidentally with an analytics, auth, or database starter kit.

### For an ecosystem

Shared rails make many small apps legible without making them centrally owned.
A shot has a known identity spine, configuration seam, manifest vocabulary,
verification command, and production boundary. Builders and agents can move
faster because those invariants are familiar.

The community can look at a contact sheet of attempts rather than a leaderboard
of promises. It can help people make, use, learn, and continue.

## Why now

Coding agents have made source generation abundant. That does not automatically
make software dependable.

The scarce parts are now:

- choosing a bounded starting point;
- preserving ownership while automating;
- knowing what is private and what can leave the machine;
- surviving crashes, malformed local state, and partial work;
- distinguishing a runnable prototype from a convincing transcript;
- keeping external authority with the human;
- telling the truth about what is implemented.

Without rails, faster generation can produce more fragile repositories, more
credential exposure, and more software that only works inside the conversation
that made it. TOHSENO supplies the local, testable contract around the agent:
the working base, the manifest, the private-input boundary, deterministic
operations, and the final gate.

The timing matters for another reason. Personal software can become specific
again. When making an app costs less, an app no longer needs a giant market to
deserve existence. It can serve one person, one season, one practice, one
friend group, or one strange recurring thought—and remain real software.

## How the future of software looks

The future is likely to contain far more software, made for far smaller groups,
with much shorter distances between author and user.

Some software will still be large, centralized, regulated, and operated by
specialist teams. TOHSENO does not pretend those systems are one prompt away.
But a growing share of software can be personal, local-first, and continuously
reshaped by the people who use it.

In that world:

- a repository is a durable artifact, not merely an implementation detail;
- a coding agent is a collaborator inside explicit authority boundaries;
- manifests and executable checks carry more trust than prose claims;
- local data is the default until a real product need justifies disclosure;
- prototypes are used, not just presented;
- abandoning one attempt does not abandon the ability to try again;
- portability and ejection are designed at creation time.

The unit of software creation becomes smaller. The standard for honesty should
become higher.

## TOHSENO's role in that world

TOHSENO is not trying to be the cloud where every app must live. Its role is to
be the reliable local aperture through which an idea becomes owned software.

It gives the coding agent enough structure to move quickly and enough
constraints to remain inspectable. It gives the builder a repeatable ritual
without making the ritual a dependency. It gives each app continuity:
account-free identity, local persistence, explicit disclosure, independent
history, and an exit.

The durable product is not any single generated app. It is the ability to keep
turning thought into experience without surrendering ownership each time.

## What the security work changed

The production-readiness review treated the seed phrase, writing, creation
input, credentials, repositories, release bytes, and external authority as
protected assets. It exercised process death, partial writes, malformed state,
links and hardlinks, hostile output, oversized input, ambient Git/provider
configuration, loopback deputy attacks, forged PIDs, endpoint disagreement,
installer drift, and failure paths.

The resulting 0.3.1 work:

- makes Keychain failures fail closed and prevents destructive pre-delete;
- keeps the draft authoritative until a session text/sidecar commit completes;
- requires device-owner authentication to reveal or replace an identity;
- obscures private UI when the app is inactive;
- provides exact text-and-sidecar export promised by the manifest;
- protects setup files, App Store Connect keys, SQLite, logs, runtime state,
  screenshots, release trees, and private provenance against ambiguous local
  filesystem entries;
- bounds security-relevant reads, request bodies, logs, and subprocess output;
- authenticates whole third-party, release, installed CLI, Bun, and managed
  tunnel bytes;
- removes implicit package execution from the irreversible token path;
- constrains Git and coding-agent ambient authority;
- unifies production endpoint rejection across Swift, TypeScript, and shell;
- protects every Studio private read and mutation with an owner-launched local
  session;
- checks for copied private creation input after every agent exit and isolates
  unsafe results;
- makes the public site logs semantic and content-free;
- removes the idea-derived shot slug from public development health responses.

The complete finding-by-finding evidence and residual limits live in
[`SECURITY_LOGS.md`](SECURITY_LOGS.md).

## The honest current boundary

Use the repository's status words literally.

**Implemented**

- local CLI installation from a checksum- and inventory-pinned release;
- terminal and private local Studio doors into one factory;
- independent iOS shot creation from a compiling base;
- private creation provenance and post-agent verification;
- seed-phrase identity and protected local session persistence;
- local API, SQLite migration foundation, supervised runtime, and structured
  content-free logs;
- Debug endpoint injection, Apple Simulator build/run/capture, and supported-Mac
  interactive preview;
- manifest validation and read-only production inspection;
- an optional owner-approved Bankr token operation under the owner's account.

**Prepared**

- signing-team and App Store Connect setup;
- a `fastlane beta` lane that is printed for the owner and never run
  unprompted;
- stable production endpoint and single-instance SQLite configuration once an
  owner chooses infrastructure and backup policy.

**Proposed**

- broad production provisioning, deployment, monitoring, and recovery for a
  generated shot;
- DNS automation and store submission;
- SessionLink and TokenMint modules;
- Android or web shot platforms.

**Open**

- the owner decision for the unused legacy Railway volume mounted at `/data`;
- the provider/privacy terms chosen for each coding agent;
- production ownership, cost, backup, and recovery decisions for each shot.

TOHSENO cannot promise that generated code has no bugs, that a chosen provider
retains nothing, that same-user malware cannot read local state, or that an
idea will succeed. It can make the boundaries explicit, make failures safer,
make claims testable, and leave the work in the builder's hands.

That is enough to make the next shot worth taking.
