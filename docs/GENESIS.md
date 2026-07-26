# Genesis invariant

The TOHSENO mobile application does not exist in this repository.

Its absence is deliberate. The first stable TOHSENO factory release must
create that application as its first Shot. Building the mobile app by hand,
checking its source into this factory first, or adding a privileged mobile-app
template would invalidate the proof that the factory can create its own
product.

## Required order

The first stable release means the first published `1.0.0` factory artifact.
Its genesis sequence is:

1. finish and verify the factory, protocol, signer interfaces, and
   independently ejectable Shot output;
2. freeze and publish the stable factory through the existing owner-approved
   release discipline;
3. install that exact authenticated artifact into a clean environment;
4. point it at a new, empty genesis shots directory;
5. use the released factory to take Shot `001` for the TOHSENO mobile
   application;
6. retain the resulting stable Shot ID, initial Git commit, factory release
   identity, composition lock, verifier result, and Simulator evidence;
7. only then evolve that independently owned Shot according to its accepted
   product manifest.

The mobile Shot's repository is an output of the stable release, not an input
to it. It must build and run without a TOHSENO account, node, wallet, chain,
TOHSENO credentials, or hidden service. Protocol signing and node
participation remain optional around the local app.

## What may exist before genesis

This repository may contain neutral protocol interfaces, record schemas,
reference-node code, documentation, generic templates, and app capabilities
that any Shot can use. It must not contain:

- a TOHSENO mobile application target or product source;
- a prebuilt mobile application repository;
- a special template that bypasses ordinary composition for the mobile app;
- a mobile-only identity shortcut that collapses Builder, runtime, release, or
  external-action authority;
- a fabricated genesis Shot ID or evidence record.

## Status

**Implemented:** the repository gate protects the absence of reserved mobile
application paths; the factory creates independently owned Shots; and
the protocol, signer, registry, and node interfaces are executable now.

**Prepared:** the clean-environment genesis sequence and evidence checklist.
No genesis run or mobile repository has been created.

**Proposed:** the mobile application's mechanics.

**Open:** the exact private intention and accepted manifest for genesis. Those
remain owner input and never enter this repository before the stable release.
