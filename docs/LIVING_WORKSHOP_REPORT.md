# One Shot / Living Workshop report

Status: implemented and locally verified on 2026-09-03; not released or
physically accepted.

## Result

ADR 0039 makes **One Shot** the visible identity and the **Living Workshop** the
primary spatial projection of the existing product. It changes no protocol or
factory architecture. The Mac remains the factory, the intended iPhone remains
the private destination, Companion remains the keeper of human authority, app
objects remain projections of the six real presentation states, and the public
network remains a threshold reached only by exact evidence.

The Mac administrative sidebar is replaced by a cohesive workshop: Mac factory
→ current app bench → intended iPhone, with Keeper, Network threshold, the app
shelf, and a persistent One Shot dock in the same world. The first-open and
readiness chapters now introduce that relationship before exposing detailed
evidence. The Companion home is the same workshop seen from the pocket, with
the Mac visibly remote, this iPhone central, and Network, Updates, and Keeper
present as real destinations. Tohseno is separately embodied as the workshop
keeper: its posture, gesture, and one short line are derived only from the same
real scene chapter, so it cannot celebrate ahead of build/install evidence or
be confused with the Companion's human-authority destination.

All visuals are native SwiftUI vectors/SF Symbols and can be replaced without
changing state or authority. Motion is state-driven and disabled by Reduce
Motion. A list fallback, labels, identifiers, keyboard selection, `/` command
palette, Return-to-open, and the existing Return/Shift–Return composer semantics
remain available. Companion gives one restrained system haptic when the person
deliberately takes a Shot, alongside its existing Claim haptic; there is no
continuous sound, points, currency, fake celebration, or shortcut for Ship,
Claim, signing, or installation.

## Capability migration

| Prior capability | Living Workshop location | State/action authority | Deep route or preserved path | Verification |
| --- | --- | --- | --- | --- |
| Mac app library | App shelf, shelf menu, accessible list | `WorkspaceSnapshot.visibleApps` | `.library`, `.app(id)` | projection + source tests |
| Create App / First App | Persistent **One Shot** dock | existing `CreationDraft` and exactly-once `submitCreation` | `.library`, `.create` options | submit/reference/keyboard tests |
| Adopt existing Xcode app | **Adopt app…** beside One Shot and empty shelf action | existing file picker and `adoptProject` | workshop → adopt sheet | identifier/source tests |
| Select an app | Spatial shelf object, arrows + Return, list fallback | existing selected app ID | `.app(id)` | route and accessibility tests |
| Evolve app | Existing app workbench **What should change?** composer | exact current app/source state and `submitEvolution` | `.app(id)` | existing capability tests |
| Build/App/Source | Unchanged selected-app tabs | service activity, presentation, source path | `.app(id)` | existing workspace render/tests |
| Simulator and iPhone handoff | Unchanged app workbench device stage | actual capture, readiness, receipt/install evidence | `.app(id)` | existing render/state tests |
| Registry / Discover | **Network** destination and threshold object | `RegistrySnapshot`, signed catalog and chain checks | `.registry` | Registry source/tests |
| Following and private Updates | Network modes and Keeper/Updates destinations | encrypted local preference/update stores | `.registry` / Companion Updates | existing reconciliation tests |
| Profile / Builder identity | **Keeper** destination | Builder DeviceKey/public profile evidence | `.profile` / Companion Keeper | existing profile tests |
| Settings, device management, diagnostics | Gear and destination bar | existing settings/model actions | `SettingsLink` | identifiers and existing tests |
| Companion Apps tab | **Workshop** tab and app shelf | authenticated workspace projection | `.apps`, `.app(shotID)` | flow + projection tests |
| Companion Create | **One Shot** button | existing signed `shot.create` request; name optional | `.create` | unnamed-create and flow tests |
| Companion Evolve | Existing app destination | existing signed `shot.evolve` request | `.app(shotID)` | flow tests |
| Companion Registry | **Network** tab, Discover/Following | public Registry client plus private follows | Network tab | source + Registry tests |
| Companion Updates | **Updates** tab / Keeper inbox | real private update records and read receipts | Updates tab | update reconciliation tests |
| Companion Profile | **Keeper** tab | non-exportable DeviceKey and profile actions | Keeper tab | capability/source tests |
| Claim / Install / Fork | Existing exact public-release destination and Claim circle | activation, edition, canonical receipt, recipient verification | Network → release | Claim and action tests |
| Ship / Update approval | Keeper inbox; existing explicit approval sheet | complete Companion-verified structured action | Updates → approval | existing publication tests |
| Pairing and recovery | First workshop chapter | one-use invitation and authenticated pairing proof | first run / readiness | readiness and pairing tests |

Nothing in this migration reintroduces the deleted Studio dashboard or removes
the CLI recovery path.

## Deterministic evidence

`fixtures/workshop-scenes-v1.json` names 22 required scenarios and the existing
evidence projection that permits each visible claim: readiness steps,
presentation state, workspace cardinality, private update kind, public timeline
event, Claim activation/edition state, or Companion connection. Tests assert
the readiness labels against `ReadinessView`, app states against the shared six
state contract, and private network claims against `PrivateUpdateKind`. The Mac
render harness exercises every readiness chapter and every workbench/app state
without hardware; the Companion harness renders empty, waiting, building,
ready-for-phone, installing, accepted, and failed states. Separate policy tests
prove both surfaces return no workshop animation under Reduce Motion.

Captured fixture images:

- `docs/assets/living-workshop/mac-first-open.png`
- `docs/assets/living-workshop/mac-building.png`
- `docs/assets/living-workshop/mac-companion-build-stopped.png`
- `docs/assets/living-workshop/companion-pocket-workshop.png`

The complete Mac and Companion Swift test suites pass locally. These fixtures
are layout evidence only. They do not claim a released DMG, a physical pairing,
a public Ship, a Claim, or an installed app on another person's iPhone.
