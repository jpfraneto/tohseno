---
title: The app workspace
description: Build, App, Source, activity, Simulator evidence, and the permanent iPhone handoff.
---

Selecting an app opens one native workspace with three destinations.

## Build

Build is the default. It presents the understandable path:

```text
Intent → Source → Simulator → Your iPhone
```

It may show up to 200 owner-local source files observed as changed against the request's own Git baseline, plus bounded semantic activity from the durable journal. It does not show raw prompts, full harness output, internal phase names, cryptographic identities, or protocol controls on the normal path.

## App

App contains **What should change?** and the product-facing history of accepted work. Evolution begins from the exact current base already selected by the product. There is no Version picker and no mandatory feedback ceremony.

## Source

Source opens the real local Git working tree. For an adopted app this is the owner's unchanged repository. For a generated Shot it is the ordinary app repository created by the factory, including the integral app-local `.tohseno/` compatibility record and explicit private exclusions.

Opening Source is visibility, not a promise that every change was produced by the latest request. Pre-existing dirty paths are recorded separately. The product does not claim a safe general rollback for a working tree it does not own.

## Simulator and physical iPhone

The iPhone-shaped stage uses the latest real verified Simulator capture. It is labeled non-interactive; a screenshot is evidence of a built surface, not a remote-control simulator.

The physical-phone handoff remains visible beside every tab. When a verified candidate is waiting, the card states the smallest Apple-controlled action—connect, unlock, trust, enable Developer Mode, or make device selection unambiguous. There is no “install anyway” button that bypasses the missing fact. Once the condition is satisfied, the service resumes automatically.

## Activity language

Normal states are bounded: Waiting, Building, Ready, Installing, Installed, Failed, Retry, and Details. The UI may become more specific about a recovery action, but it must not expose implementation phases as if the person were operating a pipeline.
