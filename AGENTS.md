# TOHSENO repository guidance

This file applies to the entire repository. A more local `AGENTS.md`, if one is added later, may narrow implementation details but must not weaken the product, privacy, ownership, or approval boundaries here.

## Mission and current status

TOHSENO is an open-source compiler and operating system for continuity apps: software organized around one meaningful action repeated over time.

```text
act → record → reflect → continue
```

The long-term product accepts `MASTER_PROMPT.md` and produces a private-by-default application and its operating package. This repository currently contains the product shell, private intake, manifest and contract harness, operator workflow, agent capsule, and deployment preparation. It is not yet a complete native continuity-app compiler. Keep that distinction explicit in code, copy, tests, and documentation.

## Product constraints

- One application has one primary core action.
- Practice and first value come before profiles or account ceremony.
- Identity is local, contextual, and unlinkable across apps by default.
- Continuity must work without AI. AI may only enrich a declared boundary.
- The control plane observes application health and order state, not end-user continuity content.
- Unsupported features are refused. Do not turn requests outside the manifest into custom agency work.
- Do not add generic dashboards, feeds, CRUD surfaces, unrelated chat, premature profiles, manipulative streaks, or broad analytics.
- Every generated application must be ejectable from birth.

## Private data rules

Never commit or log customer Markdown, contact details, capability tokens, credentials, message bodies, production data, encryption keys, operator tokens, or payment secrets.

- A content hash identifies bytes; it never authorizes access.
- A capability token authorizes access; store only its one-way hash.
- Encrypt submitted Markdown, contact details, and messages with authenticated encryption before persistence.
- Never put private content or bearer capabilities in URLs other than the intended capability path, query strings, payment metadata, email subjects, transition metadata, analytics, or error text.
- Treat capability URLs as bearer secrets: no caching, indexing, or referrer leakage.
- Record operator access without recording the inspected content.
- Future app-runtime content stays local or encrypted by default. Do not route it through this control plane.

## Architecture and implementation

- Use Bun for JavaScript and TypeScript, strict TypeScript, `Bun.serve`, `bun:sqlite`, raw SQL, raw HTML/CSS, and minimal browser JavaScript.
- Keep runtime dependencies and indirection small. Do not add a framework, ORM, component system, analytics SDK, or build system without a demonstrated requirement.
- Keep runtime-enforced manifest properties separate from coding-agent guidance and operator/deployment metadata.
- Stable event identity is separate from artifact hashes. Sealed artifacts are immutable. Reflections are separate, independently deletable records.
- Version signed request envelopes and bind the actual method, path, body hash, timestamp, nonce, signer, and signature.
- Make state transitions explicit, mode-specific, transactional, and append-only in `order_events`.
- Keep logs structured and content-free. A safe identifier is not permission to include adjacent private fields.
- Prefer deterministic behavior at runtime. AI interpretation belongs between human intent and the manifest, not in storage, signing, completion, privacy, or deployment invariants.

## External actions

Do not create paid infrastructure, spend money, alter DNS, submit to an application store, rotate production credentials, deploy production, or publish packages without explicit owner approval. Preparing commands, configuration, runbooks, and dry-run validation is in scope.

Anky is an architectural input and reference application. Treat its repository as read-only. Do not copy Anky production implementation into TOHSENO; import only the approved study documentation with provenance.

## Change discipline

Before changing code:

1. Read this file and any nearer repository guidance.
2. Inspect the working tree and preserve unrelated work.
3. State which manifest property or product contract the change serves.
4. Check whether the change expands disclosure, ownership, cost, or external authority.

Before handing off:

1. Run focused tests, then `bun run check`.
2. Verify migrations against a temporary database.
3. Exercise relevant HTTP and operator flows without real private data.
4. Run `git diff --check` and inspect tracked files for secrets and databases.
5. Report limitations honestly; never equate `READY` with a generated native application in this vertical slice.

## Documentation language

Use these words precisely:

- **Implemented:** exercised by a live code path and testable now.
- **Prepared:** configuration and instructions exist, but no external action occurred.
- **Proposed:** architecture or product behavior that is not implemented.
- **Open:** requires a product, security, or ownership decision.
