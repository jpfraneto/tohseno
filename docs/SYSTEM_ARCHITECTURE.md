# System architecture

TOHSENO is deliberately three small parts. There is no backend for generated
apps — that absence is the architecture.

## 1. The site (`apps/site`)

One stateless Bun process: hero landing, `/docs`, `/privacy`, `/healthz`, and
the pinned `oneshot.sh`. Raw HTML/CSS, one small client script for the copy
button, strict security headers, no database, no forms, no cookies.

## 2. The rails (this repository, pinned)

`oneshot.sh` clones this repository at the exact commit in `TOHSENO_PIN` and
creates a workspace as a copy of `templates/continuity-app` — a compiling,
running iOS app:

- **Identity**: BIP39 seed phrase generated on first launch (canonical
  wordlist, standard seed derivation), Curve25519 keypair, synchronizable
  keychain storage — backed up automatically through iCloud Keychain,
  end-to-end encrypted, adopted silently on a new device or reinstall —
  reveal/restore in Settings as the manual fallback. Identity syncs; session
  content does not.
- **Persistence**: plain text files plus JSON sidecars, atomic writes, draft
  recovery after process death.
- **Modules**: paywall (RevenueCat seam, off), share card (on, local
  rendering), notifications (off), SessionLink (reserved) — all behind flags
  in `AppConfig.swift`.
- **Packaged landing page**: `site/index.html`, one static file.
- **Tests**: the invariant suite that keeps all of the above honest.

The manifest (`packages/manifest`) is the machine-readable record of what an
app does; its bounded schema is the reliability boundary agents build within.
`packages/contracts` holds the draft event/artifact contract schemas and
fixtures exercised by the check gate.

## 3. The agent protocol (`skills/continuity-app`)

The build protocol a coding agent follows in a workspace: one-line input, at
most three questions, defaults recorded as `ASSUMED`, build from the base
app, silent manifest, invariant tests, prepared TestFlight path, and the
TOHSENO completion report.

## Trust boundaries

- The oneshot verifies the pin before running anything and refuses unpinned
  code.
- The site never receives user content; generated apps never phone home.
- Secrets never enter git: key slots hold public identifiers, setup writes
  key paths only, and the check gate scans for leaked credentials.
