# Intent distillation: oneshot terminal entry

- Public record ID: `intent-public-2026-07-17-oneshot-entry`
- Recorded: 2026-07-17
- Parent record: `intent-public-2026-07-17-deployment-cell-and-phone-authority`
- Status: bootstrap implemented in-repo; production availability awaits an
  owner-approved deployment
- Canonical private source: not committed to this public repository

## Owner direction

People should be able to start from a brand-new computer with one terminal
command:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

The desired flow is `MASTER_PROMPT.md` to a production-ready pack — landing
page and mobile app — with the repository's `README`/`AGENTS.md` serving as
the entry point that tells a coding agent how to relate to the setup process.
The owner's thesis: coding agents can increasingly one-shot monorepos with
precision, and AI will become an abundant commodity, so TOHSENO's value is
the rails, not the code generation itself.

## Distillation

This supersedes the earlier proposal preference against `curl | sh`, resolved
by a two-stage design recorded in
[Agent initializer](../proposals/AGENT_INITIALIZER.md): the curl'd script is a
minimal bootstrap that verifies an exact pinned rails commit before any
template is used, refuses nonempty targets, accepts no secrets, sends no
telemetry, and generates a per-app `AGENTS.md` entry point that routes the
agent to the continuity-app skill.

Honest boundary preserved: the bootstrap creates a rails checkout and an app
workspace. The agent building a tested application from it, and any
production deployment or native mobile binary, remain separate steps with
their own approval boundaries. One command to a workspace is **Implemented**;
one command to a production app on a phone remains **Proposed**.

## Organized work produced from this intent

1. Serve `/oneshot.sh` from the product shell with revalidation. (Implemented
   in-repo; deployment pending owner approval.)
2. Bump the embedded pin to the released commit as part of the next release.
3. Restructure the root `README`/`AGENTS.md` as a dual-persona entry point:
   agent-building-an-app versus contributor-changing-the-shell.
4. Rehearse the full flow on a fresh machine with an example prompt and
   record the gaps as evidence before claiming the flow works.
