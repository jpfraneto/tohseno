# Ejection

Ejection means the owner can operate, modify, migrate, or replace their app
without TOHSENO's permission and without any hidden dependency. It is an
architectural property from birth, not a cancellation favor.

## What it means today

Every workspace the oneshot creates is already ejected:

- the whole app — source, project, tests, landing page, manifest — lives in
  the owner's directory under their own git history;
- it builds and runs with zero TOHSENO credentials and no TOHSENO endpoint;
- if distribution needs short-lived third-party credentials, the TokenMint
  service is owner-deployed and receives no user content; TOHSENO is not in
  that path;
- identity is a seed phrase on the person's device; content is plain files
  the owner can copy, export, or migrate with `cp`;
- bundle IDs, signing, and store accounts are the owner's from the moment
  `bun run setup` writes them.

## Anti-lock-in acceptance tests

An app is not honestly ejectable if any of these are true:

- the core action requires a TOHSENO endpoint or secret;
- only TOHSENO can read or export the artifacts;
- bundle IDs, domains, or infrastructure are held in an undisclosed
  third-party account;
- the build cannot be reproduced from the workspace source;
- leaving requires publication of private content or loss of the recovery
  phrase.

Open source is necessary but not sufficient. Data portability, identifier
ownership, and a reproducible build make ejection real.
