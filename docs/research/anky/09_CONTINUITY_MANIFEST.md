# The `continuity.manifest` contract

## Purpose

The manifest is the smallest agreement among:

- the person describing the app;
- the coding agent;
- the continuity domain runtime;
- platform adapters;
- the optional monorepo-hosted reflection service.

It is not a universal product schema, UI builder, deployment manifest, database schema, prompt file, or provider configuration. Its primary job is to preserve the one action and its privacy/lifecycle invariants while code is scaffolded.

Every manifest must make this line possible:

> **AppName: OneSentenceDescribingTheCoreActionAndWhatTheAppIsFor**

## Proposed schema

The proposed version is `0.1`. JSON is the portable representation; this TypeScript form explains the contract:

```ts
type ContinuityManifest = {
  schemaVersion: "0.1";

  app: {
    id: string;
    name: string;
    oneLine: string;
  };

  runtime: {
    action: {
      kind: string;
      input: {
        adapter: string;
        mediaTypes: string[];
        restrictions?: string[];
      };
      completion: Condition;
      interruption: Condition;
      checkpoint: "on-progress" | { intervalMs: number };
      interruptedOutcome: "persist" | "discard-with-consent";
    };

    feedback: {
      onProgress?: string;
      onCompletion: string;
      afterSeal: string;
      returnInvitation: string;
    };

    continuity: {
      accumulates: string[];
      artifact: {
        codec: string;
        export: "none" | "private" | "user-selected";
      };
      proof: {
        mode: "none" | "practice-key-attestation" | "server-witnessed";
        export: "none" | "user-selected";
        revealsArtifact: boolean;
      };
    };

    reflection: {
      mode: "none" | "local" | "monorepo-server";
      trigger: "none" | "after-seal-opt-in" | "after-seal-automatic";
      policy: string;
      inputDisclosure: "none" | "derived-features" | "private-artifact";
      endpointCapability?: string;
    };

    privacy: {
      localProtection: "platform-private" | "app-encrypted";
      publicByDefault: false;
      serverDisclosure: string[];
      telemetry: "none" | "minimal-pseudonymous";
    };

    identity: {
      mode: "none" | "local-practice";
      create: "first-launch" | "first-action" | "first-committed-event";
      suite?: string;
      crossAppLinking: "never" | "explicit-consent-only";
    };

    recovery: {
      offer: "immediately" | "after-first-value" | "settings-only";
      identity: "none" | "manual" | "opt-in-cloud";
      data: "none" | "manual-export" | "opt-in-encrypted-backup";
    };

    sync?: {
      mode: "none" | "opt-in-encrypted";
    };

    payments?: {
      mode: "none" | "optional-entitlement";
      mayGate: string[];
      mustNotGate: string[];
    };
  };

  agent: {
    targetHuman: string;
    longing: string;
    accumulatedContinuity: string;
    whatWouldRuinTheRitual: string[];
    forbiddenPatterns: string[];
    tone: string[];
    visualDirection: string[];
    implementationNotes?: string[];
  };
};

type Condition =
  | { kind: "active-duration"; thresholdMs: number }
  | { kind: "inactivity"; afterMs: number }
  | { kind: "count"; threshold: number; unit: string }
  | { kind: "explicit-submission"; predicate: string }
  | { kind: "lifecycle-exit" }
  | { kind: "app-defined"; policy: string };
```

### Why the schema is this small

- `kind` and adapter/policy IDs allow different app-specific actions without adding a universal field for every sport, ritual, sensor, or medium.
- completion and interruption are discriminated and independently testable.
- feedback is declarative enough to preserve the ritual but does not describe screens.
- artifact, proof, and reflection are separate.
- privacy states exactly what crosses the local boundary.
- identity creation and recovery are separate decisions.
- payment lists explicit allowed gates and non-gates.
- product guidance is isolated under `agent`, where it cannot silently alter runtime behavior.

### Runtime-enforced versus agent guidance

| Section | Runtime behavior? | Coding-agent role |
|---|---|---|
| `app.id`, `schemaVersion` | Yes: namespacing/versioning | Preserve and migrate |
| `app.oneLine` | Validated as nonempty; not executed | North-star acceptance test |
| `runtime.action` | Yes | Select/build policy and native input adapter |
| `runtime.feedback` | Identifiers/config interpreted by app policy | Design minimal response surfaces |
| `runtime.continuity` | Yes: event/artifact/proof policy | Build export and projections |
| `runtime.reflection` | Yes: consent trigger/disclosure | Implement policy/provider endpoint |
| `runtime.privacy` | Yes: storage/disclosure/telemetry gates | Produce threat/deletion inventory |
| `runtime.identity/recovery` | Yes | Select adapters and delay recovery UX appropriately |
| `runtime.sync/payments` | Yes if present | Scaffold only the declared option |
| `agent.*` | No | Guides copy, scope, visuals, and feature rejection |

Model names, provider API keys, chain RPC URLs, bundle IDs, store SKUs, database paths, prompt bodies, screen layouts, and deployment credentials belong in app/service configuration—not this conceptual contract.

## Anky example

The machine-readable version is `docs/tohseno/continuity.manifest.example.json`.

Key choices:

- action is forward-only grapheme writing;
- eight minutes marks completion;
- eight seconds of inactivity seals;
- interruption persists a fragment;
- reflection is an explicit post-seal disclosure;
- `.anky v0` is a private, user-exportable artifact;
- no public proof exists today;
- identity is created on first launch with the frozen Anky Base EOA suite;
- local data protection is truthfully `platform-private`, not `app-encrypted`;
- payment may gate reflection/dynamic painting/extra unlock behavior but never writing, local recording, export, or recovery.

The manifest chooses `after-seal-opt-in`, matching the current iOS flow and the privacy-safe framework default. Android currently auto-requests for entitled users. This is an acknowledged compatibility conflict, not a claim that Android already conforms.

Evidence:

- `apps/ios/Anky/AppRoot.swift:1966-2025`.
- `apps/android/app/src/main/java/inc/anky/android/feature/write/PostSessionSealingScreen.kt:90-94`.
- `apps/ios/Anky/Core/Protocol/AnkyDuration.swift:3-36`.
- `backend/server.ts:253-257`.

## Second example: Gratitude Lock

This is a prospective continuity implementation grounded in the existing in-memory prototype:

```json
{
  "schemaVersion": "0.1",
  "app": {
    "id": "org.tohseno.gratitude-lock",
    "name": "Gratitude Lock",
    "oneLine": "Gratitude Lock: capture one concrete gratitude in words, voice, or an image, witness it once, and release the day."
  },
  "runtime": {
    "action": {
      "kind": "multimodal-gratitude",
      "input": {
        "adapter": "gratitude.text-voice-photo.v1",
        "mediaTypes": ["application/json", "audio/m4a", "image/jpeg"],
        "restrictions": ["one-entry-per-ritual", "no-public-post"]
      },
      "completion": {
        "kind": "explicit-submission",
        "predicate": "at least one non-empty text, finished recording, or captured photo"
      },
      "interruption": {
        "kind": "lifecycle-exit"
      },
      "checkpoint": "on-progress",
      "interruptedOutcome": "persist"
    },
    "feedback": {
      "onProgress": "quiet-presence",
      "onCompletion": "show-the-entry-back-once",
      "afterSeal": "local-gratitude-card",
      "returnInvitation": "tomorrow-at-user-chosen-time"
    },
    "continuity": {
      "accumulates": ["days-practiced", "private-gratitude-cards"],
      "artifact": {
        "codec": "gratitude.compound.v1",
        "export": "user-selected"
      },
      "proof": {
        "mode": "none",
        "export": "none",
        "revealsArtifact": false
      }
    },
    "reflection": {
      "mode": "local",
      "trigger": "after-seal-automatic",
      "policy": "gratitude.witness.v1",
      "inputDisclosure": "none"
    },
    "privacy": {
      "localProtection": "app-encrypted",
      "publicByDefault": false,
      "serverDisclosure": [],
      "telemetry": "none"
    },
    "identity": {
      "mode": "local-practice",
      "create": "first-committed-event",
      "suite": "tohseno.local-practice.v1",
      "crossAppLinking": "explicit-consent-only"
    },
    "recovery": {
      "offer": "after-first-value",
      "identity": "manual",
      "data": "manual-export"
    },
    "sync": {
      "mode": "none"
    },
    "payments": {
      "mode": "none",
      "mayGate": [],
      "mustNotGate": ["capture", "record", "local-reflection", "export", "recovery"]
    }
  },
  "agent": {
    "targetHuman": "A person who wants to close a day by noticing one real thing without maintaining a journal.",
    "longing": "To feel that a small good thing was seen and did not vanish unnoticed.",
    "accumulatedContinuity": "A private constellation of practiced days and optional gratitude cards, not a performance streak.",
    "whatWouldRuinTheRitual": [
      "A feed of other people's gratitude",
      "Pressure to make the entry profound",
      "Automatic cloud upload of voice or photos",
      "A long form or dashboard before capture"
    ],
    "forbiddenPatterns": [
      "profile-before-first-gratitude",
      "social-feed",
      "generic-ai-chat",
      "public-by-default-media",
      "streak-shame"
    ],
    "tone": ["quiet", "warm", "brief", "non-therapeutic"],
    "visualDirection": ["one focal capture surface", "soft end-of-day light", "no productivity dashboard"],
    "implementationNotes": [
      "The current prototype has ritual/reflection/rest screens and text/voice/photo input, but no persistence or identity; those are new work."
    ]
  }
}
```

Evidence for the starting prototype:

- `apps/gratitude-lock/v0/ios/GratitudeLockByAnky/ContentView.swift:4-54`.
- `apps/gratitude-lock/v0/ios/GratitudeLockByAnky/RitualView.swift:4-144`.

This example tests essential generality:

- completion is explicit content submission, not duration;
- artifacts can be compound media;
- reflection can be local and deterministic;
- no server, provider, payment, gate, or blockchain is needed;
- identity can be delayed until the first committed event;
- accumulation need not be a gamified score.

## Manifest validation rules

A validator should reject a manifest when:

1. `oneLine` does not name one observable action.
2. completion and interruption cannot be evaluated by the named policy.
3. reflection discloses an artifact while privacy declares no server disclosure.
4. automatic server reflection lacks an explicit product decision and disclosure copy.
5. `publicByDefault` is anything but `false` in v0.1.
6. proof reveals an artifact without explicit export/consent.
7. payment may gate the core action, local commit, recovery, or user-owned export.
8. sync is enabled without app-encrypted local storage and a recovery/key plan.
9. identity suite is omitted when mode is `local-practice`.
10. `agent.forbiddenPatterns` is empty without a confirmed reason.

The validator should warn, not necessarily reject, when:

- identity is created at first launch rather than first action/value;
- telemetry is pseudonymous rather than none;
- interruption discards rather than persists;
- reflection is automatic;
- recovery is settings-only;
- local storage is only platform-private;
- a stable address is reused with an external service.

## Current implementation versus proposed contract

| Manifest concern | Anky today | Contract implication |
|---|---|---|
| One action | Strong writing ritual | Preserve as Anky policy |
| Completion | 480 s threshold | Runtime config |
| Interruption | Silence seals fragment | Needs explicit outcome |
| Checkpoint | Per glyph | Runtime invariant |
| Reflection consent | iOS opt-in, Android automatic | Product decision required |
| Artifact | `.anky v0` | Compatibility codec |
| Proof | None | Must remain `none` until claim defined |
| Identity | First-launch Base EOA | Compatibility suite, not universal default |
| Local protection | Platform sandbox | Truthful manifest value |
| Recovery | Uneven | Platform capability validation |
| Sync | No event sync | `none`, despite backup/server aggregates |
| Payment | Reflection/progression features | Optional boundary only |

## Interpretation

The manifest cannot erase platform differences. It is a desired behavior contract whose adapters must prove compliance. A source tree may contain an implementation and still fail manifest validation at its live composition root, as Android currently demonstrates.

## TOHSENO implication

The manifest should be the first public TOHSENO artifact because it has immediate value before code extraction:

- it forces product decisions;
- it lets an agent resist feature bloat;
- it generates lifecycle/privacy test cases;
- it identifies required adapters;
- it provides a compatibility target for Anky.

## Recommendation

Version `0.1` should remain hand-authored and human-confirmed. Do not auto-expand it into screens or CRUD entities. Generate a short acceptance checklist and fixture skeleton from it, and require explicit confirmation of `app.oneLine`, completion, interruption, reflection disclosure, and “what would ruin the ritual” before scaffolding.
