# The complete Tohseno app experience

The owner default is identity, seed-phrase recovery, and consentful continuity
in every new Tohseno app unless explicitly directed otherwise. The person
describes the app once, uses it on their phone, and asks the same agent for the
next change. The shared experience should feel native and quiet.

## Identity at first use

Use the existing Fascia InstallationIdentity implementation, which prepares a
separate P-256 identity for this installation. In the Tohseno repository the
implementation is `fascia/apple/swift/InstallationIdentity.swift`; its public
entrypoint is `InstallationIdentity.shared.prepare()`. Integrate the exact
compatible Fascia sources/package into the app target and call preparation at
first launch. Inspect the actual released implementation before coding against
it; never substitute guessed methods or copied cryptography.

The key uses Secure Enclave when available, with an explicitly represented
software fallback. Its storage is app-scoped, non-synchronizing, and
ThisDeviceOnly. Preserve it across ordinary launches and updates. Keychain
failure must not silently generate a replacement identity. An app's installation
key is neither a Builder key nor an Apple signing identity.

## Seed phrase and recovery: required experience, missing shared implementation

Create each new installation's own recovery material on-device using an
established recovery implementation and cryptographic randomness. Keep the
underlying secret in app-scoped Keychain storage, accessible when unlocked and
device-only. Do not generate real user phrases in agent tools, bake them into
the binary, sync them silently, log them, or send them to a server or model.
Never import the Companion or Builder phrase to satisfy this requirement.

The current Fascia InstallationIdentity is a randomly generated device-bound
key. It has **no seed-phrase recovery API**. Companion's BIP-39 implementation
recovers Companion identity only; it is not an app-account recovery adapter.
A recoverable secret and a non-exportable installation key are different
objects. Do not derive/export the existing Secure Enclave key to pretend they
are the same. Do not create decorative recovery words that restore nothing.

Before implementing recovery, specify exactly what the words recover (for
example, an encrypted app-data backup or a separately defined account), what
backup bytes or service are required, and how the restored state relates to a
fresh installation identity. Reuse a compatible, reviewed recovery component
when available. If one is absent, report and scope the shared recovery work;
do not silently drop the default or design a new authentication protocol inside
each generated app. Changes to normative identity semantics require the actual
protocol/architectural change, not just these instructions.

Recovery remains an explicit unfinished requirement until its implementation
and restore behavior are verified. Continue independent app work while it is
unresolved. A phrase alone cannot restore data that has no recoverable backup.

## Recovery UX

Let the person use the app before backup setup. In Settings → Recovery, explain
in one short sentence what is recoverable and where the backup lives. Provide
Back up and Restore only when their actual implementation exists. Reveal words
through an explicit local action protected by supported device authentication;
conceal them when leaving the screen or backgrounding. Do not place them in
notifications, analytics, screenshots for QA, or the clipboard automatically.
Explain that anyone holding the words can access whatever they recover.

Keep restoration separate from resetting identity or erasing existing data.
Validate the phrase and backup before mutation. Preserve the current working
state on failure and explain missing backup, wrong words, or unavailable
authentication in ordinary language. Describe precisely what was restored;
never imply restoration of Companion pairing or Builder authority.

## Continuity without universal tracking

Include the compatible Fascia continuity implementation and an intentional
entrypoint for connection. In this repository inspect
`fascia/apple/swift/ContinuityEnvelope.swift`, `fascia/apple/CONTINUITY.md`, and
the Pairing and continuity section of `protocol/SPECIFICATION.md`. Use the
existing canonical encoding, signing and verification behavior.

Before a handoff, explain which app receives which claims and for how long.
Use the exact destination Shot and, when known, recipient InstallationID.
Issue only the scoped claims the person chose; verify issuer, signature,
audience, claims and validity interval at receipt. Use the runtime's replay
handling where applicable; do not invent a new envelope format. For an unknown
or unavailable target, preserve the originating app and offer a useful retry.

Apps remain unlinkable by default, including apps from the same Builder.
Shared provenance is not user consent. Do not share Keychain groups, seed
phrases, or universal user identifiers between apps. A received continuity
claim is not automatically a backend login, access to all app data, or proof of
a human identity. Enforce the receiving app's actual authorization rules.

## A polished primary path

- Start with the app's useful action, meaningful empty state, and one clear
  primary control. Avoid mandatory accounts, recovery ceremonies, or protocol
  explanations before first use.
- Use native navigation, readable type, accessibility labels, Dynamic Type,
  appropriate contrast and touch targets. Check keyboard behavior, safe areas,
  loading, empty, offline, error, and success states on the actual screen.
- Put identity and recovery detail in Settings. Show friendly app/person names
  in continuity consent; keep raw keys and identifiers out of the normal path.
- Preserve drafts, content, and identity through backgrounding and relaunch.
  Errors retain work and give the smallest corrective action.
- In the Tohseno shell, prioritize Discover, an always-available Take a Shot
  action, Your Apps, and one truthful Mac/iPhone connection line. Setup should
  gate only actions that need it. A nearby Session, relay pairing and device
  installation readiness must not collapse into a misleading Connected badge.

These are defaults for new work, not authorization to redesign existing apps
or migrate their identities as an unrelated side effect.

## Acceptance on the real path

Exercise the first useful action, save/relaunch persistence, and ordinary update
without identity replacement. For recovery use isolated test identities and a
real backup/restore round trip, including wrong input and failure preservation.
For continuity verify the intended recipient accepts a valid handoff while
wrong-audience, expired, and modified envelopes fail. Never use or expose the
owner's actual recovery material in an automated test.

Render and inspect the actual screens at representative phone sizes, including
larger text. Use a physical phone for device signing, Secure Enclave and final
installation claims; Simulator evidence stays labeled. Preserve completed build
artifacts when delivery waits for the owner. The command reference describes
the remaining direct-agent delivery gap; do not claim that this experience
reference implements that entrypoint.
