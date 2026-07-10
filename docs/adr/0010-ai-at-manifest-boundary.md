# ADR 0010: Put AI interpretation at the manifest boundary

- Status: Accepted
- Date: 2026-07-10

## Context

Human product descriptions are ambiguous, and AI can help distill a core action. Runtime privacy, storage, signing, completion, payments, and deployment need reproducible behavior that survives provider failure and cannot depend on a model's interpretation.

## Decision

AI interpretation belongs at the boundary between human meaning and a human-confirmed continuity manifest. Once confirmed, runtime behavior for privacy, local storage, identity, request signing, completion/interruption, state transitions, payment gates, and deployment is deterministic and testable.

AI reflection may be an optional manifest capability with explicit consent/provenance. The core action, local record, export, recovery, and continuity loop continue without AI.

## Consequences

- A model output cannot silently alter a locked manifest or legal order state.
- Generated changes trace to valid manifest diffs and invariant tests.
- Provider/model outages do not erase or block local continuity.
- Prompts and provider policies are app-specific boundary configuration, not hidden runtime law.
- Unsupported human requests are surfaced for clarification/refusal instead of being improvised as custom code.

## Non-goals

This ADR does not claim the current repository semantically compiles Markdown, nor does it mandate AI for interviews, scaffolding, or reflection.
