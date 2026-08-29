# ADR 0028: Finder completes installation and first open starts with one invitation

Status: accepted

Date: 2026-08-29

Supersedes:

- ADR 0026 only where its one-line script copies the app into Applications,
  replaces an existing app, and opens it automatically. Its canonical command,
  interactive consent, immutable digest, Developer ID, Gatekeeper, bounded
  download, fail-closed publication, and no-admin constraints remain accepted.
- ADR 0016 only where an empty native library begins with the generic Your Apps
  placeholder. Its App → Intent → App abstraction and simple primary vocabulary
  remain accepted.

## Context

The safest installer was already small, but it crossed the last familiar Mac
boundary on the person's behalf: after Return it copied and opened the app.
The signed DMG already contains the native Applications alias. Letting Finder
finish that gesture makes the handoff visible, recognizable, and reversible
without weakening any verification performed before the DMG reaches Downloads.

The first empty native window also described storage before it invited action.
On first open there is only one useful idea to communicate: this is the place
where an idea becomes an app.

## Decision

The canonical command remains:

```sh
curl -fsSL https://tohseno.com/install | sh
```

After environment checks, the script shows only:

```text
You will install the TOHSENO installer.
Enter to continue. Esc to exit.
```

Enter starts the download and displays a terminal progress bar. Escape exits
successfully without downloading or installing anything. Other keys do
nothing. The controlling terminal is restored on normal exit, error, and
signal.

The download remains HTTPS-only and size-bounded. Before exposing it, the
script verifies the exact configured SHA-256, mounts it read-only, verifies the
app's nested signature, bundle identifier, Developer ID Team, and Gatekeeper
assessment, and closes the mount. It then moves the verified DMG into the
owner's Downloads folder, prints the exact location, says to double-click it
and drag TOHSENO into Applications, and reveals it in Finder.

The script never copies an app into Applications, replaces an installed app,
opens TOHSENO, requests administrator elevation, or edits a shell profile. An
existing verified identical DMG may be reused and revealed. A symlink,
non-regular destination, or different file at the destination is left
untouched and reported clearly.

When the native app has no visible apps, its detail area shows the TOHSENO mark
and exactly this invitation:

```text
WELCOME TO TOHSENO
TAKE A SHOT
This is where your ideas transform into apps.
```

The action remains **Create an App**, preserving ADR 0016's plain primary-path
vocabulary. It opens the existing keyboard-first creation composer; it creates
no new onboarding state, account ceremony, tutorial carousel, factory, or
command path. A library that already contains apps keeps the ordinary app
selection placeholder instead of pretending the owner is new.

## Consequences

Installation ends at a native Mac artifact in Finder and the person completes
the conventional DMG gesture. Updates use the same one-liner but do not
silently replace the existing application; the owner performs the visible
Finder replacement.

The welcome is a composition, not a workflow. It adds no persisted flag and
disappears naturally as soon as an app exists.

This decision changes no public protocol encoding, schema, frozen vector,
Shot, Evolution, Builder, Registry authority, billing activation, signing,
notarization, release gate, artifact pin, or external publication state. It
authorizes no upload or production activation.
