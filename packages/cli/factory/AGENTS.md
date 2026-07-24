# TOHSENO shot instructions

This is an independent native iOS **shot**. Its source, Git history, installed
capabilities, tests, manifest, machine rails, and operational playbook remain
usable without the global TOHSENO CLI or factory cache.

Creation input may exist under private, gitignored `.tohseno/provenance/`.
Read only what this build requires. That input is deliberately disclosed to
the owner-selected coding agent under that provider’s privacy and retention
terms; never quote it in output, copy it into tracked files, log it, or forward
it to another service.

Before changing the app, read these local sources completely:

1. `SHOT.md` — the sanitized functional interpretation and boundaries.
2. `DONE.md` — observable acceptance criteria for the first working shot.
3. `tohseno.skills.lock` — the exact kernel, template, skills, and digests.
4. `.tohseno/OPERATIONS.md` — deterministic development and verification rails.
5. `skills/<skill-id>/SKILL.md` for every installed app skill.

Treat the kernel, selected template, and installed skills as deliberate working
capabilities. Implement the unresolved product work in `SHOT.md` and
`DONE.md`; do not replace installed capabilities with unrelated parallel
systems. Keep the generic manifest truthful when data movement, storage,
identity, entitlements, integrations, or irreversible operations change.

Use `bun .tohseno/machine.ts operations --json` to inspect the independently
ejectable low-level machine surface. Run the pinned verifier and applicable
skill acceptance checks after changes. Attempt the supported Simulator path
when the environment permits it.

The normal owner handoff belongs to TOHSENO. Never tell the owner to install
Bun or to run an ambiguous repository-relative command. If an advanced,
ejected developer command is genuinely necessary, include the exact quoted
absolute repository path. Do not claim the app is verified, built, launched,
or working without tool evidence.

Keep credentials, private prompts, reference filenames, app content,
production data, logs, and secret values out of Git and output. Do not deploy,
create accounts, purchase services, alter DNS, submit to an app store, launch a
token, or perform another externally consequential action without explicit
owner approval.

Metadata-v1 continuity shots keep their historical local instructions and
pinned machinery. Do not inject this generic composition into an older shot or
silently migrate it.
