# Installer source and public pin

`oneshot/oneshot.sh` is the canonical installer source. The checked-in
`website/apps/site/public/oneshot.sh` and `install.sh` are the currently
published, immutable-release-pinned copies. They intentionally remain on the
last published release until a release operator completes the activation
sequence in `release/WEB_INTENTION_HANDOFF_ACTIVATION.md`.

The canonical source accepts `--claim TOKEN` and `--no-studio`. Generic
installation without a claim keeps its existing behavior. A claim is
shape-checked without being printed, and the verified installed CLI receives
it on stdin rather than as a nested process argument. The installer clears its
copy promptly and never enables shell tracing around it. The command a person
pastes may still be retained by their shell history; the token is high
entropy, single-use, and invalidated after import.

Do not copy the canonical source into the public directory until the exact
claim-capable CLI and helper artifacts have been published immutably and their
checksums verified.
