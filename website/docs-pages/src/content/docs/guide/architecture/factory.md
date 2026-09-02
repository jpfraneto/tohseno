---
title: The local factory
description: The persistent service, application boundary, engine, harness budget, lease, and recovery behavior.
---

The Mac is the only coding and Xcode execution boundary.

## Persistent service

The user LaunchAgent `com.tohseno.workspace-service` runs the stable program `~/.tohseno/bin/tohseno service run`. It opens durable state and recovers commands before accepting loopback traffic. The native app, CLI, retained Studio, and Companion all converge on one `ShotApplicationService`.

Studio is a thin compatibility projection. Its deleted dashboard, pipeline renderer, forms, and manual Version controls are not another product waiting to return.

## Admission before work

Every command is journaled before its semantic action. The journal stores immutable request metadata, canonical payload bytes, exact references, and status. Stable command and execution identities make retry an equality check: the same ID with the same bytes returns the existing receipt; conflicting reuse fails.

## One bounded implementation transition

The harness reads a private `.tohseno/TASK.md` containing the exact intention, continuity and data-preservation rules, and app identity. For adopted projects it also receives the bounded repository instructions and current observations.

The limits are:

- one implementation harness invocation;
- at most one targeted repair for a concrete code/build defect;
- one shared 60-minute harness wall-clock budget;
- stop the active harness after 15 minutes without source progress.

A repair cannot become a second planning round. Device, signing, provisioning, network, lineage, and protocol failures never spend intelligence.

## Serialization without a scheduler

One advisory machine-wide lease serializes expensive source work. A job that cannot take it remains durably waiting and begins when the lease is free. There is no hidden queue protocol or fabricated state.

The lease is released while a verified artifact waits for a phone, so one missing cable cannot block unrelated source work. Process exit releases the advisory lock; the command journal remains the durable authority.

## Recovery

On startup:

- prepared work starts;
- a live detached runner is reattached;
- a verified candidate waiting for a phone resumes deterministic delivery;
- admitted pre-running commands replay from durable exact inputs.

If a whole-Mac restart leaves an in-flight harness with no live runner, the execution fails closed. Tohseno does not rerun intelligence over unknown partial source mutation. The owner can inspect and submit a new explicit request.
