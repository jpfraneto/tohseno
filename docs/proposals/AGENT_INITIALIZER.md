# Proposal: local agent initializer

- Status: **Proposed**, with the bootstrap stage **Implemented** in-repo:
  `apps/site/public/oneshot.sh` is served at `/oneshot.sh` and tested. Its
  availability at `https://tohseno.com/oneshot.sh` becomes true only at the
  next owner-approved deployment, with the pin bumped to the released commit.
- Open: free/community operating mode, distribution name, supported agents,
  release signing, and publication ownership
- Does not implement or publish: a CLI package, install code, or agent adapter

## Desired experience

A person should be able to start a continuity app from one pinned terminal
command, then answer:

```text
Which coding agent will create this continuity app?
```

The initializer selects an adapter for a supported local coding agent, installs
the same TOHSENO manifest and privacy rails, and creates the smallest private
source contract. Agent choice changes instructions and invocation, not the
runtime product laws.

The owner chose the entry command:

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
```

An earlier draft of this proposal preferred a pinned package over `curl | sh`.
The owner's direction supersedes that, and the two positions are reconciled by
a two-stage design: the curl'd script is a small, inspectable bootstrap whose
entire body runs inside a `main` function invoked on its final line (a
truncated download executes nothing), and whose only trust decision is an
exact pinned commit embedded in the script. It clones the rails repository,
refuses to continue unless the checkout equals that pin, creates the app
workspace from the pinned template, and hands off. Convenience lives in the
URL; trust lives in the pin. A future `bunx @tohseno/create@<version>` can
coexist as the package-managed form of the same pinned flow.

Release discipline: the served script and its embedded pin change together in
one release, and the route is served with `must-revalidate` so a stale cached
bootstrap cannot point at a superseded release.

## Safe flow

1. Print the exact initializer version, source, and planned filesystem changes.
2. Detect a dirty or nonempty target and stop unless the owner chooses a safe
   path; never overwrite silently.
3. Ask for the target directory, agent, and source path interactively.
4. Keep `MASTER_PROMPT.md` local by default.
5. If using a TOHSENO capsule, accept its capability through hidden standard
   input or a bounded browser/device flow—not a command argument, query string,
   process list, or shell history.
6. Install the manifest, skill/capsule, operator runbook, safe evolution index,
   and validation commands at pinned versions.
7. Show a dry-run diff before invoking the selected agent.
8. Produce exact uninstall/rollback and ejection instructions.

The initializer does not create paid resources, change DNS, rotate production
credentials, submit stores, deploy contracts, or publish packages as a side
effect.

## Free/community boundary

The current self-hosted order is payment-gated. “Create your first continuity
app for FREE” therefore requires a distinct community contract rather than copy
alone. That decision must define:

- what source, capsule, validation, and support are included;
- whether private intake is bypassed or offered separately;
- who owns the generated repository and all accounts;
- how abuse, updates, compatibility, and ejection work;
- how the free path coexists with self-hosted, client-owned, and operated modes.

A plausible least-coupled default is a local, open-source starter that requires
no TOHSENO account or source upload, while paid modes cover operated intake and
delivery. This remains **Proposed**.

## Agent adapter contract

Every adapter should provide:

- agent availability/version detection;
- exact local instruction/capsule installation;
- noninteractive validation where supported;
- a documented manual fallback;
- no telemetry or prompt upload added by TOHSENO;
- identical manifest, privacy, approval, and ejection requirements.

Agent/provider data handling must be disclosed by the chosen provider; TOHSENO
must not imply that selecting an agent makes private input local or zero-
retention without evidence.

## Acceptance evidence

- pinned clean install and deterministic file tree;
- checksum/signature verification and tamper negative;
- dry run, offline behavior after package acquisition, and manual fallback;
- spaces/Unicode paths, existing files, symlinks, interruption, and retry tests;
- install code/capability absent from arguments, process list, shell history,
  logs, errors, analytics, and generated Git history;
- two agent adapters produce the same validated source contract;
- clean uninstall/rollback and owner-controlled ejection;
- package publication only after explicit owner approval.
