# ADR 0002 — Harness credential broker

**Status:** superseded by ADR 0003 — the driven/sandboxed harness mode was removed entirely; TOHSENO conducts the builder's own agent in the builder's own session, so no broker is needed
**Date:** 2026-07-29

## Context

The GENESIS harness sandbox (`engine/src/harness.rs`) gives a coding agent a
fresh HOME, a cleared environment, denied Keychain services, and outbound
network limited to port 443. This is the right boundary: the agent must never
read the builder's ledger, keys, or host credentials.

It also means **no supported agent can authenticate**. Claude Code keeps its
OAuth token in the Keychain or `$CLAUDE_CONFIG_DIR/.credentials.json`; Codex
keeps `auth.json` under `$CODEX_HOME`; Grok and OpenCode behave likewise. All
of those sources are deliberately outside the boundary, so every
`tohseno create` with a stock agent fails after intake. Three exploratory
persona runs (see `CLAUDE_FEEDBACK_1..3.md`) each hit this wall verbatim. The
interim mitigations shipped alongside this ADR are honesty (the exit error
now explains the situation) and the bring-your-own-agent escape hatch
(`--harness /absolute/path`).

## Decision

Introduce a **narrow localhost credential broker** owned by the engine,
outside the sandbox:

1. At harness launch, the engine starts a loopback HTTP CONNECT/TLS-forward
   proxy on an ephemeral port, holding provider credentials read from the
   *host* session (Keychain / config) — never written into the sandbox.
2. The sandbox profile allows outbound connections **only to that loopback
   port** for the agent process tree (replacing the blanket `*:443` allow),
   plus DNS.
3. The agent is configured through provider-standard environment variables
   (`ANTHROPIC_BASE_URL` + a placeholder `ANTHROPIC_API_KEY`, `OPENAI_BASE_URL`,
   …) to send API traffic through the broker. The broker injects the real
   `Authorization` header on the way out, and only toward an allowlist of
   provider API hosts.
4. The broker logs request counts and byte totals (never bodies) into
   `harness.log`, and dies with the harness process.

Properties preserved:

- raw credentials never exist inside the sandbox filesystem or environment;
- the agent can reach exactly one place, and that place only reaches the
  model provider;
- the builder can audit every brokered request count in the shot's log;
- a compromised or malicious generated instruction cannot exfiltrate host
  data it never sees, nor reach arbitrary hosts.

Out of scope: OAuth flows that require the provider CLI to mint tokens
interactively (first login happens on the host, outside tohseno, as today).

## Consequences

- `HarnessSandbox` grows a `CredentialBroker` collaborator and a per-provider
  environment map; the Seatbelt profile's network section becomes
  loopback-only for agent traffic.
- Until implemented, the candidate must keep saying plainly that stock
  agents cannot run inside the boundary, and `--harness <path>` remains the
  documented path for self-supplied agents.
