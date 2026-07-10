# Continuity app source-contract template

This directory is a valid, deliberately small starting contract. Copy all four
files into a new application repository, then replace the illustrative “One
Quiet Mark” ritual through the TOHSENO continuity-app skill.

Do not begin by adding screens. First make these changes together:

1. Rewrite `MASTER_PROMPT.md` around one observable repeated action.
2. Update `continuity.manifest.json` and validate it against
   `packages/manifest/continuity.manifest.schema.json`.
3. Confirm the completion, interruption, partial-action, reflection-consent,
   privacy, recovery, export, and forbidden-pattern decisions with the owner.
4. Update `OPERATOR.md` with real build, test, migration, backup, rollback, and
   ejection commands as implementation evidence becomes available.

The manifest separates runtime-enforced properties from coding-agent guidance
and operator metadata. Never move a privacy or lifecycle promise into prose to
avoid enforcing it. If a requested change is not representable as a valid
manifest diff, classify it as unsupported instead of silently creating custom
agency work.

This template is not a native-app compiler and contains no production
cryptographic suite, store identity, infrastructure account, or secret.
