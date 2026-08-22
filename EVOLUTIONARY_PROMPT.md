# TOHSENO 0.9.9 — Cable Genesis, Earned Pro, and the Real npm Door

You are Codex operating directly inside the existing `jpfraneto/tohseno` repository on JP’s Mac.

Read this prompt completely. Then inspect the repository, `AGENTS.md`, accepted ADRs, protocol authority, current working tree, recent Git history, release machinery, CLI, Local Workspace Service, Studio, Companion, Apple device gates, installer, website, tests, and documentation before changing anything.

Do not respond with a plan and stop.

Do the work.

The mission is to evolve the current repository into **TOHSENO 0.9.9**, with one complete, coherent path:

```text
npm i -g tohseno
        ↓
tohseno
        ↓
pick up your iPhone
        ↓
connect it by cable
        ↓
trust this Mac
        ↓
enable Developer Mode
        ↓
install Xcode if needed
        ↓
add your Apple Account to Xcode
        ↓
TOHSENO installs Companion on the iPhone
        ↓
Mac and iPhone establish their private relationship through the cable
        ↓
take your first Shot
        ↓
use TOHSENO successfully on five distinct days
        ↓
unlock the opportunity to join TOHSENO Pro
        ↓
$9.99/month or $99/year

The product law remains:

App → Intent → App on your iPhone.

The new product law beneath it is:

The cable is genesis. Five successful days earn Pro.

This is not a roadmap exercise. Implement the strongest coherent end-to-end version possible tonight, test it, document it, and leave every external or secret-bearing activation fail-closed with exact owner instructions.

1. Repository authority and operating mode

Read AGENTS.md first and obey it.

The repository already has an explicit hierarchy:

protocol/ is normative for protocol bytes and validation.
Accepted ADRs govern architecture.
Historical prompts and frozen lineage material are not current authority.
docs/STATE.md describes what actually ships.

Do not accidentally treat this file as authorization to alter public protocol bytes, historical records, contract generations, or frozen test vectors.

Before editing:

git status --short
git log -10 --oneline
cargo metadata --no-deps

Preserve unrelated user changes.

Do not:

create or switch branches unless already working on one;
force-push;
publish npm automatically;
create a GitHub release or tag;
repin the public installer;
activate production relay, APNs, DNS, contracts, or billing credentials;
deploy a contract;
alter historical protocol bytes;
weaken code signing, Gatekeeper, device trust, capability admission, exact-Version semantics, command durability, or acceptance gates;
reintroduce the deleted Studio dashboard;
introduce sudo into the normal installation;
claim that an external action succeeded when it was not actually verified.

When physical participation is genuinely necessary—unlocking an iPhone, tapping Trust, enabling Developer Mode, signing into Xcode, or completing payment—present one immediate human action and continue all other independent work.

Never request, display, log, or commit private keys, recovery words, Apple credentials, Stripe secrets, npm credentials, or identifying device information unnecessarily.

2. Preserve the architecture that already works

The repository already contains:

the Rust engine and acceptance machinery;
ShotApplicationService;
durable command and execution journals;
detached factory execution;
the factory lease;
the persistent Local Workspace Service;
loopback-only Studio;
the thin CLI;
physical-device discovery through CoreDevice/devicectl;
cable, trust, and Developer Mode detection;
Xcode toolchain and signing readiness detection;
free versus paid Apple-team detection;
the three-app free Personal Team limit;
signed build, install, launch, and accepted-Version gates;
the private Companion protocol;
durable encrypted phone outbox;
content-blind relay;
capability-based command admission;
the Swift Companion SDK;
the branded Companion app;
the one human presentation projection;
release-integrity and lifecycle verification scripts.

Preserve these rails.

In particular:

Studio remains a thin browser projection over the loopback Local Workspace Service.
The Companion remains a remote control for intent, not a mobile factory.
CLI, Studio, and Companion continue converging on the same application service.
The Mac remains authoritative for factory and workspace state.
Installed Shots remain ordinary independent apps.
Existing paired devices and persisted private state must remain compatible.
Exact accepted-Version binding and stale-request refusal remain unchanged.
The relay remains content-blind.
No second backend, factory, entitlement authority, or synchronization mechanism may appear accidentally.
The normal path must not expose Shots, Expressions, Versions, executions, capabilities, harnesses, or protocol language.
Details may continue exposing bounded technical information deliberately.

If a change adds more normal-path concepts than it removes, reconsider it.

3. Record the product decision

Add an accepted ADR covering the complete 0.9.9 product decision and update docs/STATE.md, README files, runbooks, and relevant comments consistently.

The ADR must establish:

The first Mac ↔ iPhone relationship begins physically through a cable.
Mac ↔ iPhone QR scanning is removed from the normal genesis path.
The existing secure invitation, device identity, capability, signing, encryption, revocation, and relay model remains underneath.
The cable transports or initiates the one-use bootstrap; it does not replace cryptographic identity with USB trust.
TOHSENO offers the complete product during the trial, not a reduced demo.
The trial lasts at most seven calendar days.
Five successful days qualify the person for TOHSENO Pro.
TOHSENO Pro costs $9.99/month or $99/year.
TOHSENO Pro and the Apple Developer Program are independent.
A free Apple Personal Team remains supported for Pro users, subject to Apple’s weekly provisioning and three-installed-app limits.
The Studio factory, CLI mutations, and Companion mutations lock when entitlement is unavailable.
Existing work and installed applications are never deleted or remotely disabled by the paywall.
Billing and entitlement records are private product state, not public TOHSENO protocol objects.
Existing historical records and already-paired devices are not rewritten.

Do not make the ADR authorize npm publication, release publication, production billing activation, or external infrastructure activation.

4. Cable Genesis

Replace first-run QR pairing with a guided cable-first genesis.

The exact human sequence is:

Pick up your iPhone.
Connect it to this Mac with a cable.
Trust this Mac.
Enable Developer Mode.
Install Xcode.
Add your Apple Account to Xcode.
Install TOHSENO on your iPhone.
Take your first Shot.

This sequence must feel straightforward, direct, and precise.

Interaction law

At every moment, show:

one instruction;
one immediate action;
at most one primary button;
one way to go back when going back is safe;
no dashboard;
no checklist wall;
no simultaneous collection of decisions;
no protocol vocabulary;
no fake completion controls.

Prefer automatic detection. If TOHSENO can observe that the action succeeded, advance automatically.

Do not ask “Did you do it?” when the machine can know.

When the user must act outside TOHSENO, guide them to the exact place and wait without losing state.

Examples:

Pick up your iPhone.
Connect your iPhone to this Mac with a cable.
Unlock your iPhone and tap Trust.
On your iPhone, open Settings → Privacy & Security → Developer Mode.
Turn it on and let your iPhone restart.
Install Xcode from the App Store, then open it once.
Open Xcode → Settings → Accounts and add your Apple Account.
Installing TOHSENO on your iPhone…
TOHSENO is on your iPhone.
Take your first Shot.

The desired user-visible order is canonical. Internally, account for real tool dependencies honestly. For example, if Developer Mode cannot be authoritatively inspected until Xcode/CoreDevice is available, do not falsely claim it is enabled. Advance to the required Xcode action, then return to the deferred verification without exposing implementation complexity.

Reuse the existing device gates

The repository already distinguishes:

cable missing;
trust required;
Developer Mode required;
ready physical device.

Extend and project this machinery rather than creating a second device detector.

Use privacy-minimal device information.

The genesis state must be durable across:

Studio refresh;
Studio closure;
Local Workspace Service restart;
Mac restart;
iPhone restart;
failed Companion installation;
interrupted Apple sign-in;
an already-completed step;
an already-paired compatible device.
Install the Companion through the cable

The Local Workspace Service must:

Detect the attached trusted development iPhone.
Detect Xcode and Apple-signing readiness.
Select the appropriate Apple development team using the existing paid/free preference.
Build the existing branded Companion target for that physical device.
Sign it with the user’s Apple development identity.
Install it through the existing safe devicectl boundary.
Launch it.
Complete the existing cryptographic pairing/capability ceremony through a one-use cable-originated bootstrap.
Verify that the Companion and Mac reached the paired state.
Continue to the first-Shot experience.

Do not weaken the pairing protocol. Change how the one-use invitation reaches the phone.

Investigate the installed Xcode/devicectl tooling and official local help before selecting the transport. Prefer a supported local mechanism such as launching the installed Companion with a one-use TOHSENO URL or another bounded CoreDevice-supported payload. Never invent a command-line flag without verifying it.

The one-use bootstrap must remain:

signed;
bounded;
expiring;
single-use;
tied to the intended workspace and device;
free of recovery words, private keys, arbitrary URLs, and filesystem paths;
invalid after cancellation or completion.

Preserve the Companion’s twelve-word recoverable device identity behavior. Recovery words remain shown exactly once and must not be copied into Mac logs, URLs, build settings, environment variables, artifacts, relay records, or Studio.

Remove the camera/QR ceremony from normal Mac ↔ Companion first run. Preserve only QR behavior that is still separately required by an accepted decision, such as browser-linking under ADR 0018. Do not conflate browser ↔ phone linking with Mac ↔ phone genesis.

Existing paired installations must continue working without being forced through genesis again.

Add focused tests for every state and transition.

5. The complete trial experience

The trial begins only after all of the following are true:

Companion was successfully installed on the physical iPhone;
Mac and Companion completed secure pairing;
the Local Workspace Service durably recorded genesis.

Before that point, onboarding is not consuming trial time.

The trial gives the person the exact product they would use after paying.

There is:

no reduced feature set;
no separate “trial app”;
no separate “Pro app”;
no fake sample factory;
no watermark;
no card required upfront.

“What you tried is what you buy.”

Trial state

Implement one private, versioned, durable entitlement state machine.

At minimum, it must represent:

genesis_incomplete
trial_active
trial_qualified
trial_expired
pro_monthly
pro_yearly
pro_lapsed

Use names appropriate to the codebase, but preserve the semantics.

The state must survive service and Mac restarts and must be recoverable without rewriting app lineage.

The canonical authority should live on the Mac within private machine state. Companion receives only the minimal signed private product projection it needs. Do not place subscription state into public protocol records or the public node.

Do not mutate a frozen Companion schema casually. If a new private versioned entitlement projection or event is required, add the smallest explicit structure with compatibility tests.

Successful day

A successful day is a distinct local calendar day on which a factory command produces a newly accepted Version that was actually delivered, installed, and launched on the physical iPhone according to the existing acceptance rules.

Consequences:

Opening Studio does not count.
Opening Companion does not count.
Writing an intention does not count.
A harness exit does not count.
Generated source does not count.
A failed build does not count.
Waiting for a phone does not count.
Multiple accepted Versions on the same day count as one successful day.
Create and Evolve are treated equally when they reach accepted delivery.
Retrying the same durable command cannot count twice.

Record bounded evidence connecting each successful day to an accepted execution/Version without exposing private intention bytes.

This product trusts the person. Do not build invasive anti-tamper, surveillance, fingerprinting, or adversarial clock policing. Validate the obvious invariants and keep the implementation local and comprehensible.

Trial completion

The trial ends at the first of:

five successful distinct days; or
seven calendar days after genesis.

When the fifth successful day is reached, allow the current accepted operation to finish normally.

On the next normal entry into Studio or Companion, show the Pro decision instead of the product.

Qualified copy:

You made software on five different days.

Continue with TOHSENO Pro.

Options:

$9.99 monthly
$99 yearly
Not now

The annual option should explain concisely that it saves approximately two months, without sales clutter.

If seven days pass without five successful days, lock the factory without offering purchase:

Your TOHSENO trial has ended.

Everything you made is still here.

Only people who completed five successful days are qualified to purchase Pro.

Do not invent an automatic cancellation. No subscription exists before qualification and conscious purchase.

Add deterministic tests using an injected clock. Never make tests depend on the actual wall clock or sleeping for days.

Test:

genesis does not consume a successful day;
one accepted Version counts;
failure does not count;
waiting does not count;
two acceptances on one day count once;
acceptance on five distinct days qualifies;
the fifth day completes the current operation before locking;
seven-day expiry without qualification does not offer Pro;
retries are idempotent;
restart preserves state;
timezone/day-boundary handling is deterministic;
existing paired users receive a documented, safe migration policy;
no Shot or accepted Version is deleted by expiry or lapse.
6. The hard product boundary

The entitlement boundary must be enforced below the UI.

Do not implement only a JavaScript paywall.

When locked:

Studio displays only the appropriate entitlement screen.
tohseno create refuses before admitting a command.
tohseno evolve refuses before admitting a command.
Companion Create/Evolve requests are rejected with one human sentence.
The application service rejects new factory mutations.
Existing durable work already admitted before the boundary is resolved according to an explicit, tested rule.
Read-only integrity, diagnostics, renewal, export, billing recovery, and safe uninstallation remain possible.
The Local Workspace Service continues running so entitlement can be restored.
Existing app folders, source, Shots, accepted Versions, journals, identities, pairing state, and installed apps remain intact.
Generated apps receive no remote kill switch.
Already-installed apps do not depend on a live TOHSENO subscription to launch.

Choose and document the rule for an operation admitted immediately before qualification/expiry. Prefer the rule implied above: an already-admitted operation may reach its deterministic terminal result, then the factory locks.

Companion and Studio must describe the same entitlement state in human language.

7. TOHSENO Pro and Apple membership are separate

TOHSENO Pro unlocks the factory:

creating apps;
evolving apps;
Companion commands;
continuing to use Studio as a living factory.

Apple’s development membership controls Apple provisioning and distribution.

The existing repository already detects:

free Personal Team;
paid development team;
unknown team;
the three-app free-team wall;
provisioning expiry from embedded profiles.

Preserve and surface that truth simply.

A TOHSENO Pro user with a free Apple Personal Team remains supported.

They may:

keep using TOHSENO Pro;
install up to Apple’s free-device limit;
replace an already-installed app;
rebuild and reinstall when the seven-day provisioning expires.

They may encounter:

approximately weekly reinstallation;
at most three active development apps on the device;
no TestFlight or App Store distribution.

A paid Apple Developer Program membership unlocks the Apple side:

more concurrent development apps;
longer-lived development provisioning;
TestFlight;
App Store distribution;
additional Apple capabilities.

Do not call it “Apple Pro.”

Do not require the Apple Developer Program to purchase TOHSENO Pro.

When the free-team limit or expiry is actually encountered, present one decision:

Remove one installed app

or:

Use a paid Apple development team

Guide the person through Apple enrollment only when they choose that road. Open Apple’s official enrollment location, preserve state, and wait. On return, re-detect the team rather than trusting a button press.

Never claim TOHSENO can purchase, activate, or guarantee Apple membership.

8. Billing and entitlement receipts

Implement the smallest production-shaped billing boundary that can support:

$9.99/month;
$99/year;
qualified users only;
subscription activation;
renewal;
cancellation at period end;
lapse;
restoration;
customer billing management;
local entitlement refresh.

Prefer a hosted web checkout initiated from Studio, not payment collection inside the sideloaded Companion.

Inspect the existing website/server architecture before choosing placement. Do not create a second unrelated web stack if the existing Bun website can host the bounded billing endpoints safely.

A viable shape is:

qualified local installation
    ↓
one-use signed checkout claim
    ↓
official TOHSENO HTTPS billing endpoint
    ↓
hosted checkout
    ↓
verified webhook
    ↓
server-signed entitlement receipt
    ↓
Local Workspace Service verifies receipt
    ↓
Studio and Companion unlock

The exact implementation may differ if the repository contains a stronger existing identity primitive. Prefer reuse over invention.

Requirements:

Bind checkout to a privacy-minimal stable TOHSENO installation/Builder identity.
Do not send app source, intentions, app names, device names, Apple identifiers, or recovery material to billing.
Use opaque one-use nonces.
Verify webhook signatures.
Make webhook processing idempotent.
Never trust a browser success redirect as proof of payment.
Represent monthly/yearly plan and validity explicitly.
Verify entitlement receipts locally using a pinned public verification key or an equivalently strong asymmetric mechanism.
Do not embed a billing signing secret in the CLI, Studio, npm package, Companion, or repository.
Store server secrets only through documented environment configuration.
Fail closed when billing configuration or receipt verification is unavailable.
Permit a bounded offline grace policy only if explicitly documented and tested.
Never log complete checkout tokens or payment data.
Provide a customer-portal path for managing the subscription.
Preserve access until the paid-through date when cancellation is scheduled for period end.

If live billing credentials are unavailable, complete the code, schemas, fixtures, local fake provider, webhook tests, configuration validation, and exact activation runbook. Do not fake a production transaction or mark production billing active.

Add test-mode flows that cannot be enabled accidentally in release builds.

9. Make npm i -g tohseno the real front door

The public npm package tohseno@0.0.2 currently exists but is functionally empty.

Create or isolate the npm package at:

packages/cli

unless inspection reveals an already-authoritative package location that should be preserved.

The npm package is a thin, dependency-free, secure Node.js bootstrap and launcher.

It does not reimplement the Rust CLI, Local Workspace Service, Studio, Companion protocol, application service, or factory.

Its job is:

identify the Mac;
securely obtain the authorized native TOHSENO release;
install it into the existing installer-owned layout;
delegate to the stable native launcher;
open the existing loopback Studio/onboarding experience.

The desired fresh-Mac experience is:

npm i -g tohseno
tohseno
Reconcile with the current architecture

Do not invent a second Tohseno Studio.app architecture merely because a generic installer prompt assumes one.

The current repository defines:

a Rust native CLI;
embedded/static Studio assets;
a loopback Local Workspace Service;
a stable launcher under ~/.tohseno/bin/tohseno;
a user LaunchAgent;
no-sudo installation.

Preserve that model unless repository inspection proves a deliberate accepted replacement already exists.

The npm package installs and launches the official native TOHSENO release bundle containing the Rust executable and Studio assets.

Do not install into /Applications unless the repository already contains an accepted, signed application-bundle architecture for Studio. Do not add one as scope creep.

npm package requirements

Implement using ESM and built-in Node APIs only.

Configure:

{
  "name": "tohseno",
  "version": "0.1.0"
}

Also include:

a bin mapping from tohseno to the executable entry point;
files containing only required runtime files;
Node >=20;
repository metadata;
homepage;
bugs URL;
license;
precise description;
publishConfig.access: "public".

Support:

tohseno
tohseno install
tohseno open
tohseno doctor
tohseno --version
tohseno --help

After native installation, preserve existing native commands by delegating unknown commands to the installed Rust launcher.

Therefore commands such as these must continue working:

tohseno create my-app
tohseno evolve my-app
tohseno studio
tohseno service status
tohseno companion devices

Avoid recursive self-invocation. Resolve the native launcher by its explicit trusted installation path.

No-argument behavior

tohseno with no arguments must:

reject non-macOS systems clearly;
detect arm64 versus x86_64;
determine whether the authorized native release is installed;
verify the installed release metadata;
install or update when required;
ensure the Local Workspace Service is installed and healthy;
open Studio;
enter cable genesis when genesis is incomplete;
otherwise enter the trial, Pro, or locked product surface.

Keep output concise and human:

Installing TOHSENO…
Starting TOHSENO…
Opening TOHSENO…

Do not expose internal installation vocabulary unless something fails.

Official release manifest

Do not embed arbitrary artifact URLs in npm source.

Fetch a small HTTPS release manifest from an official TOHSENO origin.

Design, version, validate, test, and document its JSON schema.

It must include:

schema version;
native release version;
minimum compatible npm CLI version;
artifact URL for each supported architecture;
exact artifact byte size;
SHA-256 digest;
expected file/layout version;
expected signing identity information where applicable.

Use an allowlist of exact HTTPS origins and hosts.

Reject:

HTTP;
credentials in URLs;
unexpected ports;
unapproved hosts;
redirects to unapproved hosts;
malformed manifests;
duplicate architecture entries;
unsupported architecture;
oversized manifest or artifact;
missing digest;
noncanonical digest;
incompatible versions.

Download into a securely created temporary directory.

Verify:

exact byte size;
exact SHA-256;
archive shape;
no path traversal;
no symlink escape;
expected native launcher;
expected release metadata;
existing repository release-integrity/signature mechanisms.

Use the repository’s current release-package integrity model rather than inventing a weaker parallel verifier.

If the native release is delivered as a notarized macOS application or package, additionally use the appropriate existing macOS verification:

codesign --verify --deep --strict
spctl --assess --type execute --verbose

Verify the expected Team ID/designated requirement when applicable.

If the release is currently a signed archive rather than an application bundle, verify it according to the repository’s authoritative release package and activation policy. Do not pretend spctl meaningfully verifies an artifact type it does not assess.

Never:

bypass Gatekeeper;
remove quarantine attributes;
invoke sudo;
execute an unverified download;
trust npm-package integrity as a substitute for native-release verification;
install from a source checkout;
follow an arbitrary manifest URL from an environment variable in production mode.

Install into the repository’s existing safe user-owned layout, preserving rollback and installer ownership.

Doctor

tohseno doctor should report, without unnecessary identifiers:

macOS version;
architecture;
Node version;
native TOHSENO installation/version;
release-manifest compatibility;
Local Workspace Service installation and health;
Xcode installation;
Xcode command-line tools;
Apple-signing readiness;
free/paid/unknown provisioning category when detectable;
whether a physical iPhone is visible;
cable/trust/Developer Mode readiness in human terms;
Companion installation/pairing state;
trial/qualification/Pro state without payment details or secrets.

Doctor is diagnostic. It must not mutate device trust, install software, start checkout, expose recovery words, print full device identifiers, or dump private paths unnecessarily.

npm tests

Add unit tests for:

command parsing;
no-argument routing;
platform rejection;
architecture selection;
semantic version comparison;
manifest validation;
minimum CLI enforcement;
hostname allowlist;
redirect rejection;
byte-size enforcement;
SHA-256 verification;
archive path safety;
installed-native detection;
delegation without recursion;
readable failures;
redaction of sensitive values.

Add an isolated integration test that:

runs npm pack;
installs the tarball into a temporary npm prefix;
proves the tohseno executable exists;
proves tohseno --version succeeds;
proves tohseno --help succeeds;
does not modify the developer’s global npm installation;
performs no network download or native installation.

Ensure npm pack --dry-run contains no:

repository secrets;
fixtures unrelated to the npm runtime;
signing material;
release authority material;
private test data;
.env files;
source checkout;
native binaries not intentionally shipped;
user paths;
unrelated repository files.
npm release documentation

Add exact manual instructions for publishing tohseno@0.1.0:

authenticate with npm;
verify package ownership;
verify the existing 0.0.2 package and dist-tags;
run unit and isolated integration tests;
inspect npm pack --dry-run;
inspect the actual tarball;
publish with provenance where supported;
verify the registry metadata;
install from npm into a genuinely clean temporary prefix;
test tohseno --help and tohseno --version;
test the native download only after an authorized native release manifest exists.

Do not publish automatically.

Do not change the live npm dist-tag.

10. Studio experience

Studio remains four conceptual surfaces at most, but genesis and entitlement may replace the normal surface when applicable.

Routing priority:

genesis incomplete
    → cable genesis

genesis complete + trial active
    → Your Apps / Create / Evolve

trial qualified or Pro lapsed
    → Pro decision

trial expired without qualification
    → preserved-work trial-ended screen

active Pro
    → Your Apps / Create / Evolve

Do not render the normal factory behind a dismissible modal. The entitlement screen is the product surface while locked.

Do not expose a permanent “Connect iPhone” QR action as the normal first-device path.

Settings may retain:

connected iPhone summary;
revoke;
diagnostics;
reconnect/replace through an explicit cable flow;
Apple provisioning status;
subscription management when appropriate.

Maintain:

loopback binding;
Host validation;
same-origin mutations;
CSRF protection;
strict CSP;
text-only safe rendering;
bounded assets;
event-stream refresh;
no permissive CORS.

Enforce genesis and entitlement server-side.

11. Companion experience

The Companion remains the same app before and after payment.

Add only the smallest product states required:

cable bootstrap / recovery words
Your Apps
one app
entitlement decision
trial ended

Do not add:

account dashboards;
billing history;
Apple enrollment inside the phone;
source browsing;
build logs;
model selection;
protocol terminology;
chat;
a second factory.

On entitlement lock, the Companion may open the trusted TOHSENO billing continuation on the Mac or explain in one sentence that Pro is completed on the Mac. Do not collect payment directly inside the sideloaded Companion for 0.9.9.

The existing durable outbox must not accept new evolution commands after the entitlement boundary. Commands already durably accepted before the boundary follow the documented in-flight rule.

Existing offline/reconciliation behavior must remain correct.

12. CLI behavior

Preserve all existing scriptable forms.

The native CLI should gain a useful no-argument entry point if it does not already have one:

tohseno

Its behavior after npm delegation is:

ensure service health;
open Studio;
route Studio to genesis, trial, paywall, or normal use.

Add explicit structured entitlement inspection only if useful for tests or administration, for example:

tohseno entitlement status
tohseno entitlement refresh

Do not expose developer-only trial mutation commands in release builds.

Test-only time/state controls must require the repository’s existing isolated verification mode or equivalent safe compile-time/debug gating.

JSON output must remain stable and must not mix human progress into stdout.

13. Migration

Define and test migration for existing 0.9.0 users.

Requirements:

never erase pairing state;
never erase identity;
never erase workspace data;
never reset an already-running factory silently;
never fabricate successful trial days from app count alone;
never lock JP out of a development checkout without an explicit safe development policy;
existing paired Companion installations continue functioning;
an existing user receives a deterministic trial anchor or explicit grandfathered development entitlement according to a documented rule;
release builds cannot accidentally inherit test/grandfather flags.

Choose the smallest honest migration and record it in the ADR and docs/STATE.md.

14. Verification

Start by running the relevant focused tests before editing so existing failures are distinguished from regressions.

After implementation, run the repository’s declared suite from AGENTS.md:

cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
swift build --package-path apple-identity
swift test --package-path apple-identity
swift test --package-path fascia/apple
swift test --package-path sdk/apple/TohsenoCompanionKit
swift test --package-path companion/apple/TohsenoCompanion
forge build --root contracts
forge test --root contracts -vvv
node --test studio/tests/static_assets.test.mjs
(cd website && bun run typecheck && bun test)
./scripts/test-ontology-lifecycle.sh
./scripts/test-local-companion-e2e.sh
./scripts/test-macos-service-lifecycle.sh

Also add and run:

cable-genesis state-machine tests;
Companion bootstrap tests;
device-gate projection tests;
genesis restart/recovery tests;
trial clock tests;
successful-day/idempotency tests;
entitlement-admission tests;
Studio paywall tests;
Companion paywall tests;
billing receipt verification tests;
webhook idempotency tests;
npm CLI unit tests;
npm pack/install integration test;
package-content audit;
release-manifest validation tests;
a focused 0.9.9 local golden-path script.

The golden path should prove as much as possible without real secrets or a physical phone:

fresh isolated installation
→ service starts
→ genesis begins
→ simulated cable states advance one action at a time
→ Companion installation/pairing fixture completes
→ trial begins
→ accepted versions on five distinct injected days qualify
→ next entry locks
→ verified test entitlement unlocks
→ lapse locks again
→ all workspace and accepted records remain

When a physical iPhone is available, provide one exact smoke command or runbook that exercises:

cable
→ Trust
→ Developer Mode
→ free Personal Team
→ Companion build/sign/install/launch
→ cryptographic pairing
→ first Shot

Do not make the automated suite require JP’s real phone, Keychain, LaunchAgent, global npm prefix, production relay, or billing credentials.

15. Documentation and release truth

Update consistently:

root README;
docs/STATE.md;
ADR index;
Studio README;
CLI README;
Companion README;
SDK documentation if bootstrap behavior changes;
installer/release documentation;
0.9.9 readiness/runbook;
npm package README;
billing activation runbook;
trial and entitlement privacy documentation;
threat model where the new billing/entitlement boundary matters.

Clearly distinguish:

repository source targets;
npm bootstrap package version 0.1.0;
native TOHSENO product version 0.9.9;
currently published native release;
currently authorized installer pin;
currently active billing state;
currently active relay/APNs state.

Do not claim 0.9.9 is publicly available merely because source code targets it.

Do not overwrite the historical root EVOLUTIONARY_PROMPT.md semantics without first reconciling its current historical status. This file is execution input, not protocol authority. Once the implementation is complete, move or preserve it according to repository documentation conventions without allowing it to supersede protocol/ or accepted ADRs.

16. Definition of done

This mission is complete when:

A fresh Mac can install the npm package in isolation.
tohseno is a real executable.
The npm bootstrap securely installs or locates the authorized native release.
The native service opens Studio.
First run is a durable cable-first journey.
Every onboarding screen asks for one immediate action.
Xcode, Apple Account, cable, Trust, Developer Mode, signing, Companion installation, and pairing are detected honestly.
Companion installation uses the user’s free or paid Apple development team.
Mac and iPhone establish the existing secure relationship without normal-path QR scanning.
Trial begins only after verified Companion genesis.
The complete factory works during the trial.
Accepted physical-device results count at most once per calendar day.
Five successful days qualify the person for Pro.
Seven elapsed days without qualification end the trial without a purchase offer.
Studio, CLI mutations, application-service mutations, and Companion mutations enforce the entitlement below the UI.
Qualified users see $9.99/month and $99/year.
Verified entitlement activation unlocks the same product.
Subscription lapse locks it again without deleting anything.
Free Apple Personal Team remains supported for Pro users.
The existing three-app limit and weekly provisioning reality remain honest.
Installed generated apps contain no subscription kill switch.
Existing pairings and workspaces migrate safely.
Public protocol, contracts, frozen history, and release authority remain unchanged.
Relevant focused tests and the full declared verification suite pass.
npm package publication remains a documented manual owner action.
External activation blockers are listed exactly and no unavailable external success is fabricated.
17. Final report

At the end, report:

the product behavior now implemented;
the exact architecture chosen for cable bootstrap;
how recovery words remain safe;
the trial and successful-day authority;
where entitlement is enforced;
how monthly/yearly billing is represented;
what is real versus still configuration-gated;
how free and paid Apple teams behave;
the npm package structure;
the release-manifest and verification design;
migration behavior;
every test run and its result;
any physical or secret-bearing action JP must perform;
exact manual commands to publish tohseno@0.1.0;
exact command to run the physical-iPhone 0.9.9 smoke path.

Do not end with merely a proposal.

Build the coherent system.
