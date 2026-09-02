Tohseno Evolution — Wireless-First Distribution 
+ Network Trust You are working inside the root 
of the Tohseno repository. This is an 
evolutionary change to the product, protocol, 
UX, documentation, and implementation. Do not 
treat this as a greenfield rewrite. Before 
changing anything, deeply inspect the 
repository and understand the architecture that 
actually exists today: the current 
whitepaper/report, ADRs, STATE.md, 
LIVING_CONNECTION.md, readiness/release 
documents, Mac app, Companion app, CLI, 
registry/web surfaces, backend/relay code, 
signing model, Claim model, Ship/Release model, 
CoreDevice integration, local persistence, and 
tests. The goal is to evolve Tohseno toward two 
new first-class truths. The trust truth: > 
**Software you can trust because people you 
trust have seen it.** The distribution truth: > 
**Claim anywhere. Your Mac prepares the 
software. Your iPhone installs it when it 
becomes reachable. A cable is a compatibility 
mechanism, not the product.** These two ideas — 
wireless-first delivery and network-mediated 
software trust — are now first-class 
architectural directions for Tohseno. Do not 
merely change copy. Bring the underlying 
architecture closer to making these statements 
genuinely true. ──────── 0. FIRST: 
UNDERSTAND THE SYSTEM Before implementation, 
study the current repository exhaustively. Read 
the latest working paper / whitepaper. Read all 
ADRs relevant to: • Builder identity • 
DeviceKey • Companion authority • Ship • 
Release • Claim • verification • registry • 
delivery • CoreDevice • relay • remote actions 
• local signing • installation • source 
provenance Read at minimum: • STATE.md • 
LIVING_CONNECTION.md • the latest 
release/readiness documents • the current 
working paper / whitepaper • existing ADRs • 
golden-path documentation • current tests 
around shipping, claiming, building, 
installation, pairing, and registry state 
Inspect the implementation corresponding to 
those documents. Determine: 1. what is actually 
implemented; 2. what exists only in 
documentation; 3. what exists only as future 
design; 4. where documentation and code 
disagree; 5. which protocol encodings or 
invariants are intentionally frozen; 6. which 
concepts can safely evolve additively; 7. which 
proposed changes require migrations; 8. which 
parts of this prompt are already partially 
implemented. Do not assume file names, APIs, 
structs, services, or abstractions in this 
prompt are exact if the repository has evolved. 
The repository is the source of truth. Before 
making large implementation changes, document 
the architectural decisions in the style 
already established by Tohseno. 
──────── 1. WHAT TOHSENO IS BECOMING 
Tohseno is not merely an app factory. It is 
becoming a person-to-person software network. 
The factory remains central. A Builder can turn 
an intention into software. They can Ship that 
software. Another person can encounter the 
software, understand its provenance, inspect 
the available evidence around it, see who has 
reviewed it, Claim it, and ultimately choose 
whether to run it on their own device. The 
important shift is: > Tohseno should not decide 
what software people are allowed to run. 
Instead: > Tohseno should make the decision of 
whether to run software radically more legible. 
The final authority belongs to the person whose 
device will execute the software. That person 
should eventually be able to understand: • who 
built the software; • which Builder identity 
signed it; • which exact release they are 
considering; • where the source came from; • 
whether the source and built artifact 
correspond; • whether the build is 
reproducible; • what permissions the software 
requests; • what entitlements it uses; • what 
dependencies it contains; • what domains and 
services it communicates with; • whether 
payment functionality exists; • what machine 
verification occurred; • what findings were 
produced; • who reviewed the exact release; • 
what those people actually reviewed; • whether 
any reviewers are already part of the 
recipient’s social graph; • how those reviewers 
have behaved historically. Do not build a 
decentralized imitation of App Store Review. 
Avoid the framing: > Tohseno says this app is 
safe. Prefer: > Here is the artifact. > > Here 
is its provenance. > > Here is what machines 
observed. > > Here are the people who examined 
this exact release. > > Here is their history. 
> > You decide whether to run it. The user is 
not being abandoned with responsibility. The 
user is being given final authority supported 
by unusually good evidence. That distinction is 
fundamental. ──────── 2. PRESERVE 
BUILDER AUTHORITY This is a hard architectural 
invariant unless inspection of the repository 
demonstrates that the existing protocol defines 
something materially different: > **Builder 
authority lives on the Companion / iPhone 
DeviceKey.** Do not make any of the following 
the root of Builder authority: • Farcaster • 
GitHub • Base • Ethereum • X • OAuth • email • 
a Tohseno server account • a wallet provider • 
$TOHSENO ownership • token stake • follower 
count The root model should remain 
conceptually: ```text human
  ↓ Builder ↓ BuilderID / BuilderAccount ↓ 
DeviceKey
  ↓ human approval on Companion ``` The 
Companion-held DeviceKey is sovereign. External 
identities decorate and contextualize the 
Builder. They do not replace the Builder. 
Conceptually: ```text
                         BUILDER
                           │
                       BuilderID
                           │
                  DeviceKey authority
                           │
               Secure Enclave / Companion
                           │
          
┌────────────────┼────────────────┐
          │ │ │ ▼ ▼ ▼
     Farcaster GitHub Base
        FID account ID address
   social identity technical identity economics
          │ ▼ X
       optional ``` External identity 
compromise must not independently grant 
publishing authority. External identity loss 
must not destroy the Builder identity. External 
identity providers must not be able to 
impersonate a Builder. The DeviceKey must 
remain capable of authorizing important Tohseno 
actions independently of these external 
systems. ──────── 3. IDENTITY BINDINGS 
SHOULD BE FIRST-CLASS OBJECTS Introduce or 
formalize an additive concept equivalent to: 
```text IdentityBinding ``` if the current 
protocol does not already model this correctly. 
An IdentityBinding means approximately: > This 
Tohseno Builder cryptographically claims or 
verifies a relationship with this external 
identity. Possible binding classes: ```text 
farcaster github base x ``` The exact canonical 
representation should follow existing Tohseno 
protocol conventions. Do not blindly introduce 
a new schema if the repository already contains 
an appropriate signed-profile/attestation 
primitive. Every identity binding should 
answer: • Which Builder does this belong to? • 
Which external identity is bound? • Which 
stable external identifier is canonical? • What 
display metadata is mutable? • What proof 
established the binding? • Who signed the 
Tohseno-side statement? • When was it created? 
• Can it be revoked? • Can it expire? • Can it 
be refreshed? • What happens if the external 
username changes? • What happens if the 
external account is compromised? • What remains 
historically visible after disconnection? 
External identity bindings are context. They 
are not Builder authority. ──────── 4. 
FARCASTER AS FIRST-CLASS SOCIAL IDENTITY 
Farcaster should become Tohseno’s primary 
social identity layer. Do not turn Tohseno into 
a Farcaster client. Do not make Farcaster 
mandatory for Builder authority. Farcaster is 
useful because it gives Tohseno portable social 
context around a Builder: • stable FID; • 
username; • display name; • PFP; • bio; • 
verified addresses where relevant; • follows; • 
social relationships. Prefer stable protocol 
identifiers such as the FID as the canonical 
binding rather than mutable usernames. The UI 
can display: ```text Farcaster @jpfraneto ✓ 
``` while internally the relationship should 
look conceptually like: ```text BuilderID X
    ↕ Farcaster FID Y ``` The connection 
ceremony must be verifiable. Study the current 
signing and Companion architecture and design 
the cleanest proof possible. Do not create a 
purely cosmetic OAuth association that cannot 
later be independently reasoned about. We 
already operate infrastructure related to the 
Farcaster / Snapchain / Hypersnap ecosystem. 
Inspect what exists and determine whether it 
can appropriately support: • identity 
resolution; • profile metadata; • follows; • 
social graph queries; • verification. Avoid 
introducing an unnecessary mandatory 
centralized Farcaster API dependency if our 
existing infrastructure allows a more sovereign 
architecture. However, do not overcomplicate 
the initial implementation merely for 
ideological purity. Choose a practical 
architecture that preserves the ability to 
become more self-hosted over time. 
──────── 5. FARCASTER’S ROLE IS SOCIAL 
CONTEXT, NOT SECURITY AUTHORITY The important 
initial behavior is: > A recipient can see 
whether people they already follow have 
reviewed a release. For example: ```text 
NETWORK REVIEW 18 builders reviewed this 
release. 3 are people you follow. ``` That is 
immediately useful. However: > **Farcaster 
Follow != security Trust.** Someone may follow 
another person because they are: • funny; • 
interesting; • a friend; • an artist; • a 
founder; • a musician; • entertaining; • 
politically interesting; • culturally relevant. 
A social follow must not silently become a 
security delegation. Initially, Farcaster 
follows should be treated as a trust prior / 
social context. Later, Tohseno may introduce a 
private native trust concept. Conceptually: 
```text Trust Alice for: [x] privacy [x] iOS 
permissions [ ] financial contracts [ ] 
cryptography ``` Do not overbuild scoped trust 
in this pass unless necessary. But ensure the 
architecture does not permanently conflate: 
```text follow ``` with: ```text trust ``` 
These are different concepts. ──────── 
6. GITHUB AS FIRST-CLASS TECHNICAL IDENTITY 
GitHub should become a first-class technical 
identity and provenance layer. Farcaster 
approximately helps answer: > Who is this human 
in a social graph? GitHub approximately helps 
answer: > What evidence exists of this human’s 
technical history and relationship to this 
source? A Builder should be able to connect 
GitHub. Prefer a durable GitHub account 
identifier internally rather than treating 
mutable usernames as canonical. The UI may 
display: ```text GitHub jpfraneto ✓ ``` but 
the binding should survive a username change 
where possible. GitHub can eventually provide 
contextual evidence such as: • linked account; 
• repositories; • source repository 
relationship; • commit authorship; • 
contribution history; • merged changes; • 
relationship between a Builder and a source 
tree. Do not reduce this to a simplistic: 
```text GitHub score: 92 ``` Do not equate: • 
stars; • followers; • number of repositories; • 
contribution count; with security authority. 
GitHub provides technical context and 
provenance. Reputation should still emerge 
primarily from observable behavior inside the 
Tohseno network. ──────── 7. BASE / 
WALLET AS ECONOMIC IDENTITY Allow a Builder to 
bind one or more economic identities, initially 
including a Base/EVM address if compatible with 
the existing architecture. Again: > wallet != 
Builder authority. A Base address should be 
treated as a bound economic identity. It may 
eventually participate in: • $TOHSENO; • review 
markets; • bounties; • rewards; • payments; • 
verification jobs; • challenge bonds; • 
security grants; • economic commitments. Do not 
make wallet ownership itself evidence of 
technical competence. Do not make token 
ownership evidence of trustworthiness. Do not 
let a whale purchase security reputation. 
──────── 8. X IS OPTIONAL DECORATION X 
may be connected as another external social 
identity. It is useful for: • social 
continuity; • discovery; • public Builder 
context. But X should not be structurally 
necessary to the network. Do not architect the 
trust graph around a centralized X API. Do not 
allow X account compromise to affect Builder 
authority. Treat X as optional 
decoration/context. ──────── 9. 
SECURITY REQUIRES PRECISE LANGUAGE Study the 
current protocol meanings of: • Claim • 
VerificationResult • Evidence • Ship • Release 
• Shot • checkpoint • digest • Builder 
signature • candidate • build • source 
provenance Do not casually overload established 
terms. Security architecture becomes dangerous 
when several different actions all become 
called “verification.” Maintain precise 
semantics. ──────── 10. CLAIM MUST 
REMAIN CLAIM Claim should not silently become: 
• “I verified this app.” • “I trust this app.” 
• “I own this app.” • “I installed this app.” • 
“This app is safe.” • “I endorse this Builder.” 
• “I audited this release.” If the current 
protocol defines Claim as something equivalent 
to: > this Tohseno identity encountered / 
claimed this particular Shot or release; 
preserve that semantic purity. Claim is useful 
precisely because it means one thing. Do not 
contaminate it with security semantics. 
──────── 11. MACHINE VERIFICATION AND 
HUMAN ATTESTATION ARE DIFFERENT Formalize or 
introduce two different concepts. A. 
Verification Report A Verification Report 
contains machine-generated 
observations/evidence about an exact immutable 
release. Possible contents include: • Shot ID; 
• Release ID; • release digest; • checkpoint 
digest; • source tree digest; • dependency 
inventory; • dependency versions; • dependency 
changes; • build scripts; • signing 
information; • reproducibility status; • 
requested entitlements; • privacy-sensitive 
permissions; • network destinations; • payment 
SDKs; • embedded binaries; • suspicious source 
patterns; • known vulnerabilities; • provenance 
checks; • policy checks; • static-analysis 
findings; • model-assisted findings. A 
Verification Report can be generated through 
combinations of: • deterministic tooling; • 
source inspection; • static analysis; • 
dependency analysis; • binary inspection; • 
build inspection; • reproducibility checks; • 
local models; • remote models; • Bankr-routed 
intelligence; • future verification workers. 
The important principle is: > Machine output is 
evidence. Machine output is not automatically 
human authority. A model must not be able to 
silently say: > JP reviewed this app. if JP did 
not. ──────── 12. INTRODUCE RELEASE 
ATTESTATION AS A DISTINCT PRIMITIVE Introduce 
or formalize a concept equivalent to: ```text 
ReleaseAttestation ``` A ReleaseAttestation 
means: > A particular Builder/reviewer is 
willing to cryptographically sign a bounded 
statement about a particular immutable release 
after inspecting a declared set of evidence or 
scopes. Conceptually: ```text 
ReleaseAttestation {
    version shotID releaseID releaseDigest 
    checkpointDigest? reviewerBuilderID 
    reviewerDeviceKeyID reviewPolicyVersion 
    verificationReportDigest? scopes [
        source, dependencies, entitlements, 
        permissions, networking, privacy, 
        payments, reproducibility
    ] outcome findings[] createdAt signature } 
``` This is conceptual only. Do not copy the 
schema blindly. Use the existing canonical 
serialization, hashing, signing, and protocol 
conventions of Tohseno. Important invariants: 
1. An attestation refers to an exact immutable 
release. 2. An attestation is cryptographically 
attributable to a Builder authority. 3. The 
attestation states what was reviewed. 4. It may 
reference machine-generated evidence. 5. It 
does not claim metaphysical certainty that 
software is “safe.” 6. It becomes historical 
evidence. 7. It cannot silently transfer to a 
future release. 8. It must be possible to 
determine exactly what statement was signed. 9. 
A verifier cannot sign one digest and have the 
UI display the attestation against another. 10. 
Replay must be considered explicitly. 
──────── 13. DO NOT SAY “THIS APP IS 
SAFE” Do not create: ```text SAFE ✓ ``` Do not 
create: ```text Tohseno Safety Score 96 / 100 
``` Do not create a centralized omniscient 
verdict. Use bounded factual language. 
Examples: ```text Reviewed by 18 builders ``` 
```text 3 people you follow reviewed this 
release ``` ```text Source reproduced ``` 
```text No camera entitlement detected ``` 
```text Location: While Using App ``` ```text 3 
network destinations detected ``` ```text 1 
dependency changed since previous release ``` 
```text No blocking findings under Review 
Policy 1 ``` ```text Review incomplete ``` 
```text Findings present ``` Every summarized 
statement must be traceable to actual evidence. 
Do not manufacture confidence through UI 
styling. ──────── 14. ATTESTATIONS 
BELONG TO EXACT RELEASES This is critical. 
Suppose: ```text Ayunoando Release 1.4.2 18 
attestations ``` The Builder then ships: 
```text Ayunoando Release 1.4.3 ``` Release 
1.4.3 must not inherit the eighteen 
attestations. The new release should begin with 
its own review state. For example: ```text 
Ayunoando Release 1.4.3 Recently updated 0 
attestations for this release ``` What can 
carry forward: • Builder identity; • Builder 
history; • reviewer history; • previous 
releases; • previous attestations; • previous 
findings; • previous reproducibility record; • 
social relationships; • trust relationships. 
What does not carry forward: > the claim that 
someone reviewed code they have not actually 
reviewed. This distinction must exist in: • 
protocol; • storage; • indexes; • APIs; • 
registry UX; • Companion UX. ──────── 
15. MALICIOUS UPDATE MUST BE PART OF THE THREAT 
MODEL The trust network must explicitly handle 
this attack: ```text Builder releases harmless 
app
        ↓ earns trust ↓ many reviewers attest 
        ↓
later Builder ships malicious update ``` 
Builder reputation may provide historical 
context. But a trusted Builder does not make an 
unreviewed update automatically trusted. The UI 
must distinguish: ```text Builder has strong 
history ``` from: ```text This exact release 
has been reviewed ``` Never collapse those into 
the same signal. ──────── 16. “LENDING 
INTELLIGENCE THROUGHPUT” SHOULD BECOME A REAL 
NETWORK ACTIVITY A central future idea is: > 
Network members can lend intelligence 
throughput to understanding software. Design 
the architecture so this can become real. 
Conceptually: ```text new release
     ↓ verification jobs ↓ deterministic 
analysis
     ↓ model-assisted analysis ↓ Verification 
Report
     ↓ human inspection ↓ DeviceKey signs 
Release Attestation ``` The Mac is naturally 
the place for intelligence work. The Companion 
is naturally the place for sovereign human 
approval. The Mac may: • download exact inputs; 
• resolve source; • inspect dependencies; • 
build; • run scanners; • run static analysis; • 
inspect entitlements; • inspect network 
behavior; • run local models; • call remote 
intelligence; • spend inference credits; • 
produce structured evidence. The Companion can 
then present something like: ```text REVIEW 
AYUNOANDO 1.4.2 Source analyzed Dependencies 
inspected Entitlements inspected Network 
destinations inspected 3 findings [ Inspect 
Findings ] I reviewed this evidence and want to 
sign: [x] Source [x] Dependencies [x] 
Entitlements [ ] Privacy [ Sign Attestation ] 
``` Do not blindly implement this exact UI. Use 
it as the mental model. The important 
separation is: ```text Mac = intelligence 
workbench iPhone = human authority ``` 
──────── 17. AUTONOMOUS VERIFIERS MAY 
EXIST LATER Future Tohseno participants may 
configure their nodes to automatically 
contribute computation. For example: > 
Contribute up to $5/month of inference toward 
reviewing software in my network. This is a 
valid future direction. Design current 
verification primitives so future verification 
workers can exist. However, machine-generated 
statements and autonomous agents must remain 
distinguishable from human-signed attestations. 
Do not let: ```text AI scan completed ``` 
render as: ```text Alice reviewed this app ``` 
unless Alice explicitly authorized that exact 
meaning under a clearly defined delegated 
policy. No ambiguity. ──────── 18. 
REPUTATION SHOULD BE EARNED THROUGH BEHAVIOR, 
NOT PURCHASED This is a foundational principle: 
> **Reputation should be earned through 
behavior, not purchased.** Do not implement: 
```text more $TOHSENO
     = more trusted ``` Do not implement: 
```text higher stake
     = greater security authority ``` Do not 
implement: ```text more Farcaster followers
     = better verifier ``` Do not implement: 
```text more GitHub stars
     = better verifier ``` Instead, preserve 
objective historical facts. Examples: ```text 
releases shipped releases reviewed review 
scopes findings submitted findings confirmed 
findings disputed attestations withdrawn 
attestations contradicted reproducibility 
history source provenance history apps shipped 
time participating ``` A reputation experience 
can be derived from this history. Avoid 
prematurely creating one irreversible: ```text 
ReputationScore = 9382 ``` inside the protocol. 
Reputation should ideally remain a projection 
over evidence. Different consumers may weight 
evidence differently. ──────── 19. 
TRUST SHOULD EVENTUALLY BE PERSONALIZED The 
most interesting question is not: > Is this app 
globally trusted? It is: > What does **my 
network** know about this artifact? Example: 
```text NETWORK REVIEW 42 builders reviewed 
this release. YOUR NETWORK Sofia reviewed it. 
Antoine reviewed it. You follow both on 
Farcaster. ``` Eventually native trust 
preferences may allow more precise 
interpretation. Two users may legitimately see 
different trust context around the same 
release. That is acceptable. It may even be 
desirable. Do not force one global trust 
hierarchy where none exists. ──────── 
20. DO NOT TURN TOHSENO INTO A POPULARITY 
MACHINE Protect the character of the network. 
Do not let this evolution become: • follower 
farming; • engagement optimization; • public 
follower counts everywhere; • reviewer 
leaderboards based on fame; • social feeds; • 
influencer rankings; • attention markets 
masquerading as security. The goal is not: > 
Who is the most popular Builder? The goal is: > 
What evidence exists around this artifact, and 
which people relevant to me have contributed to 
that evidence? If existing Tohseno architecture 
intentionally keeps follow relationships 
private or deemphasizes follower counts, 
preserve that direction. ──────── 21. 
$TOHSENO IS AN ECONOMIC LAYER, NOT THE TRUTH 
LAYER $TOHSENO may eventually provide economic 
coordination underneath the verification 
network. The likely long-term loop is: ```text 
release needs review
        ↓ verification/review bounty ↓ people 
+ machines contribute intelligence
        ↓ verification reports ↓ human 
attestations
        ↓ useful work receives compensation 
``` Possible future token uses include: • 
verification rewards; • review bounties; • 
finding rewards; • challenge bonds; • reviewer 
availability markets; • security grants; • 
treasury-funded review; • protocol fees. 
However: > Do not reward reviewers for saying 
that software is safe. Do not create: ```text 
positive verdict → token reward ``` That 
produces corrupt incentives. Economic rewards 
should compensate useful work. Examples might 
include: • completing a requested review; • 
producing verified evidence; • discovering a 
confirmed issue; • performing a reproducibility 
check; • participating in a bounty; • providing 
specialized review capacity. Do not attempt to 
finalize staking/slashing/tokenomics in this 
evolutionary pass unless absolutely required. 
Build the primitives first. Let real network 
behavior inform the eventual economic design. 
──────── 22. WIRELESS-FIRST IS THE 
SECOND MAJOR EVOLUTION The product must stop 
thinking of USB cable connectivity as the 
normal runtime distribution model. The correct 
abstraction is: > **Is the intended iPhone 
reachable by this Mac?** Not: > Is a cable 
plugged in? Study the existing architecture 
carefully. Important existing pieces may 
already include: • Companion → relay → Mac 
commands; • durable commands; • Mac offline 
behavior; • build preparation; • artifact 
retention; • CoreDevice detection; • deferred 
installation; • automatic resumption when a 
device appears. Reuse this architecture. Do not 
create a cloud build system merely because 
Claim can happen remotely. The private Mac 
factory remains central. ──────── 23. 
THE NEW GOLDEN PATH The product should support 
this mental model: ```text Someone sends me: 
tohseno.com/anky
        ↓ I open it on my iPhone ↓ Tohseno 
Companion opens
        ↓ I see: Anky by jpfraneto exact 
release Builder identity Farcaster identity 
GitHub identity source provenance machine 
observations release attestations people I 
follow who reviewed it
        ↓ CLAIM ↓ the request reaches my Mac 
        ↓
my Mac privately: verifies fetches builds signs 
prepares
        ↓ READY TO INSTALL ↓ my associated 
iPhone becomes reachable
        ↓ install ↓ run ``` The person may 
Claim while away from home. The Mac may be at 
home. The Mac may temporarily be offline. The 
iPhone may temporarily be unreachable. Those 
conditions should become ordinary durable 
states, not catastrophic errors. 
──────── 24. CLAIM ANYWHERE A recipient 
should eventually be able to Claim a public 
Tohseno release regardless of whether: • their 
Mac is currently open; • the Mac is currently 
online; • their iPhone is currently near the 
Mac; • a cable is connected. The Claim should 
produce durable intent. If the Mac is offline: 
```text Claimed Waiting for your Mac ``` When 
the Mac reconnects: ```text Your Mac is 
preparing Anky ``` After 
build/signing/verification completes: ```text 
Ready for your iPhone ``` If the iPhone is 
currently unreachable: ```text Ready for your 
iPhone Tohseno will install this when your 
iPhone is reachable. ``` When the correct phone 
becomes reachable: ```text Installing… ``` 
Then: ```text Installed ``` Use real backend 
state. Do not fake progress. Do not imply 
asynchronous behavior exists unless it actually 
does. ──────── 25. REPLACE “WAITING FOR 
CABLE” WITH DEVICE REACHABILITY Find all domain 
assumptions equivalent to: ```text 
waiting_for_cable ``` Refactor them toward 
device reachability. Possible conceptual states 
include: ```text iphone_unknown 
iphone_associated iphone_unreachable 
iphone_reachable iphone_locked 
iphone_needs_pairing 
iphone_needs_developer_mode iphone_needs_trust 
ready_to_install installing installed ``` These 
exact names are not mandatory. Choose the 
smallest state machine that accurately 
describes the implementation. Transport should 
be secondary metadata. For example: ```text 
reachableVia: - wifi - usb ``` The domain 
concept is: ```text reachable ``` not: ```text 
cable_connected ``` ──────── 26. USB IS 
A TRANSPORT / BOOTSTRAP MECHANISM A cable may 
still be required in some Apple-controlled 
situations. Tohseno must remain honest about 
Apple’s constraints. But USB should no longer 
be built into the product ontology. The 
hierarchy should be: ```text 1. already-paired 
wireless / nearby delivery 2. wireless pairing 
when supported 3. one-time cable bootstrap when 
Apple requires it ``` Not: ```text STEP 1 
CONNECT YOUR USB CABLE ``` Verify current Apple 
behavior and official documentation before 
encoding assumptions around: • first-time 
pairing; • wireless pairing; • supported iOS 
versions; • supported Xcode versions; • 
Developer Mode; • trusted Mac relationships; • 
CoreDevice; • install over Wi-Fi; • device 
discovery. Do not hardcode Apple-version claims 
from this prompt. Determine capabilities 
dynamically where practical. ──────── 
27. PAIR YOUR IPHONE, NOT “PLUG IN YOUR CABLE” 
The primary UX concept should become: ```text 
Pair your iPhone ``` or: ```text Connect your 
iPhone ``` Then: ```text Searching for nearby 
devices… ``` If wireless setup is available: 
```text JP's iPhone [ Pair ] ``` Only when the 
environment actually requires a cable should 
the UI explain: ```text One-time cable setup 
This iPhone and Mac need a cable for initial 
pairing. After pairing, Tohseno will use 
supported wireless delivery whenever possible. 
``` Do not show cable instructions prematurely. 
Do not make users think a cable will be 
required every time they install an app. 
──────── 28. DURABLE COMPANION ↔ 
PHYSICAL DEVICE ASSOCIATION One of the most 
important missing primitives is likely a 
durable association between: 1. the Companion 
identity / DeviceKey requesting software; 2. 
the physical CoreDevice that should receive the 
software. Conceptually: ```text Companion 
DeviceKey A
        ↕ Physical Apple Device B ``` This 
relationship should be established during an 
appropriate trusted bootstrap ceremony. It 
should be persisted. It should survive 
application restarts. It should avoid 
ambiguity. It should allow Tohseno to answer: > 
Which physical device should receive an install 
requested by this Companion? without asking 
every time. Study the stable device identifiers 
exposed by Apple’s current tooling and 
determine what can safely be persisted. Be 
careful with: • privacy; • identifier 
stability; • device replacement; • erased 
phones; • restored phones; • multiple phones; • 
lost phones; • pairing changes. Document the 
chosen strategy. ──────── 29. MULTIPLE 
IPHONES MUST BE A REAL SUPPORTED MODEL Do not 
design around: ```text one human one Mac one 
iPhone forever ``` A user may have: • primary 
iPhone; • secondary iPhone; • test iPhone; • 
old iPhone; • replacement iPhone; • future 
iPad; • future Vision device; • future 
Watch-related companion state. At minimum, the 
domain model should support multiple associated 
install targets. The current UX may still 
optimize for one primary device. But do not 
make the underlying architecture impossible to 
evolve. Possible concepts: ```text 
InstallTarget id deviceType displayName 
association lastSeen reachability primary ``` 
Again: use the existing architecture rather 
than inventing unnecessary abstractions. 
──────── 30. MULTIPLE DEVICES MUST 
NEVER CAUSE ACCIDENTAL INSTALLATION Threat 
model this explicitly. Scenario: ```text JP's 
Mac can currently see: JP iPhone Nacha iPhone 
Test iPhone ``` JP claims an app from JP’s 
Companion. The Mac must not simply install 
onto: ```text first CoreDevice returned by API 
``` or: ```text only connected device ``` if 
more than one exists. The intended recipient 
device must be known. The request must be bound 
strongly enough to avoid installing the app on 
the wrong physical phone. If no safe 
association exists, ask for explicit 
selection/confirmation. Do not guess. 
──────── 31. DEVICE ASSOCIATION SHOULD 
BE ESTABLISHED DURING BOOTSTRAP The ideal 
bootstrap relationship may look conceptually 
like: ```text Mac discovers physical iPhone
        ↓ Mac installs or recognizes Companion 
        ↓
Companion establishes DeviceKey identity
        ↓ pairing ceremony proves: "This 
Companion belongs to this physical device"
        ↓ Mac stores association ``` Study 
what the existing onboarding actually does and 
evolve it rather than rebuilding the entire 
system. The relationship should eventually 
allow: ```text remote Companion request
        ↓ Mac knows intended install target ↓ 
correct device receives software ``` 
──────── 32. MAC AS PRIVATE FACTORY The 
Mac’s role should become explicit. The Mac is a 
private software factory. Responsibilities may 
include: ```text SOURCE fetch exact source 
resolve capsule verify digest maintain source 
provenance ``` ```text BUILD compile resolve 
dependencies prepare signing produce artifact 
retain artifact ``` ```text VERIFY run 
deterministic checks inspect source inspect 
dependencies inspect entitlements inspect 
permissions run model-assisted analysis produce 
Verification Report ``` ```text DELIVER receive 
durable requests associate requests with 
intended devices wait for device reachability 
install resume interrupted delivery ``` The Mac 
should not conceptually be: > the thing the 
cable plugs into. Its role is much larger and 
more coherent than that. Reflect this in UI and 
documentation. ──────── 33. COMPANION 
AS HUMAN AUTHORITY The iPhone Companion should 
remain the place where important human 
authority is expressed. Examples may include: • 
publishing; • signing Builder actions; • 
Claiming; • approving external identity 
bindings; • signing Release Attestations; • 
approving sensitive actions; • managing 
associated devices; • approving installs where 
required. The web can inform. The Mac can 
prepare. The relay can transport. The models 
can analyze. But important human authority 
should remain legible on the human-held device. 
──────── 34. REGISTRY / PUBLIC APP UX 
MUST EXPRESS TRUST Study the current public 
Registry/App pages. Evolve them so an app page 
eventually communicates: ```text AYUNOANDO by 
jpfraneto Release 1.4.2 ``` Then: ```text 
BUILDER jpfraneto Farcaster @jpfraneto ✓ 
GitHub jpfraneto ✓ Base 0x... ✓ ``` Then: 
```text PROVENANCE Builder signature ✓ Source 
available Source digest … Build reproducibility 
… ``` Then: ```text NETWORK REVIEW 18 
attestations for this exact release ``` Then 
personalized context where available: ```text 
YOUR NETWORK 3 people you follow reviewed this 
release ``` Then machine observations: ```text 
OBSERVATIONS Camera No access detected 
Microphone No access detected Location While 
using app Network 3 destinations Payments 
Stripe Dependencies 1 changed since previous 
release ``` Then: ```text [ Inspect Evidence ] 
``` And finally: ```text [ Claim ] ``` Only 
display facts the implementation actually 
knows. No fake green checkmarks. No placeholder 
verification presented as real evidence. 
Progressive disclosure. The top-level 
experience should remain understandable to a 
normal person. ──────── 35. BUILDER 
PROFILE SHOULD EXPRESS AUTHORITY + CONTEXT + 
HISTORY Study the current Builder profile and 
evolve it. The profile should make a clear 
distinction between: 1. sovereign Builder 
authority; 2. connected external identities; 3. 
behavior/history; 4. verification activity. 
Conceptually: ```text
                [ PFP ]
              jpfraneto
        Farcaster @jpfraneto ✓ GitHub 
        jpfraneto ✓ Base 0x91...38 ✓ X 
        @jpfraneto
``` Then: ```text BUILDER AUTHORITY This 
Builder's publishing authority is held by a 
DeviceKey on Tohseno Companion. DeviceKey 
91F8…A20C Active ``` Then: ```text ACTIVITY 14 
apps shipped 39 releases 72 releases reviewed 
``` Then potentially: ```text REVIEW HISTORY 
Source reviews 62 Privacy reviews 29 Dependency 
reviews 71 Confirmed findings 11 ``` Do not 
necessarily expose all metrics publicly. 
Respect the privacy model. Do not manufacture a 
single reputation score unless extremely 
justified. ──────── 36. IDENTITY 
CONNECTION UX The profile should allow additive 
identity connections. Existing Builders must 
not be blocked if they have no Farcaster or 
GitHub. Possible UI: ```text CONNECTED 
IDENTITIES Farcaster Not connected [ Connect ] 
GitHub Not connected [ Connect ] Base 0x91...38 
✓ X Not connected [ Connect ] ``` An identity 
connection should not merely set a database 
field. It must have a verifiable binding model 
underneath it. The user should understand that: 
```text Connected identities prove context. 
They do not control your Builder. ``` Do not 
necessarily use that exact copy. But preserve 
the conceptual distinction. ──────── 
37. MACHINE VERIFICATION UX A Verification 
Report should eventually have a human-readable 
representation. For example: ```text 
VERIFICATION REPORT Release Ayunoando 1.4.2 
Source Matched expected digest Build Reproduced 
successfully Entitlements Location: While Using 
App Network api.stripe.com api.example.com 
cdn.example.com Dependencies 12 total 1 changed 
Findings 2 informational 0 blocking ``` Allow 
deeper technical detail without overwhelming 
ordinary recipients. Structured 
machine-readable evidence should exist 
underneath the UI. ──────── 38. RELEASE 
ATTESTATION UX A reviewer must understand what 
they are signing. Do not present a giant: 
```text VERIFY APP ``` button with ambiguous 
semantics. Instead, something more precise: 
```text REVIEW RELEASE Ayunoando 1.4.2 I 
reviewed: [x] Source [x] Dependencies [x] 
Entitlements [x] Network destinations [ ] 
Payment behavior [ ] Privacy behavior 
Verification report: 0x93a8… Outcome: No 
blocking findings under Review Policy 1 [ Sign 
Attestation ] ``` The signed canonical payload 
must correspond to what the UI says. Never let 
UX and signed semantics diverge. 
──────── 39. ATTESTATIONS MAY NEED 
WITHDRAWAL / SUPERSESSION Threat model honest 
mistakes. A reviewer may sign an attestation 
and later realize: • they reviewed the wrong 
thing; • new evidence emerged; • their 
assessment was mistaken; • their DeviceKey was 
compromised. Determine whether attestations 
should support: • withdrawal; • supersession; • 
revocation notices; • dispute annotations. Do 
not erase historical records silently. Prefer 
append-only historical truth where possible. 
For example: ```text Attestation A signed Sep 1 
Withdrawal B references Attestation A signed 
Sep 3 ``` The Registry can then truthfully 
display: ```text Attestation withdrawn ``` 
rather than pretending it never existed. Follow 
existing Tohseno protocol philosophy. 
──────── 40. REVIEW POLICY MUST BE 
VERSIONED If people sign statements like: 
```text I reviewed this release ``` the meaning 
of “reviewed” must not float over time. 
Introduce or identify a versioned review 
policy. For example: ```text 
tohseno-review-policy/1 ``` The policy may 
define scopes and what checks each scope means. 
An attestation can then bind: ```text 
reviewPolicyVersion ``` This allows policy 
evolution without rewriting historical claims. 
Keep the first policy small. Do not attempt to 
define a perfect universal security standard. 
──────── 41. VERIFICATION REPORTS MUST 
BE CONTENT-ADDRESSED WHERE POSSIBLE Because 
attestations refer to evidence, the evidence 
should not be mutable underneath them. Prefer a 
model where the Verification Report has a 
canonical digest. Conceptually: ```text 
VerificationReport
        ↓ canonical encoding ↓ digest ↓ 
ReleaseAttestation references digest ``` This 
gives us: ```text "I signed my assessment of 
this exact report about this exact release." 
``` The report itself may include references to 
larger artifacts. Follow existing 
content-addressing conventions in the 
repository. ──────── 42. SOURCE CHANGES 
MUST BREAK OLD EVIDENCE If a release’s source 
changes, the release identity/digest must 
change. Never allow: ```text same release 
identifier different source same attestations 
``` The exact implementation should align with 
the current Shot/Release/checkpoint 
architecture. Preserve Tohseno’s existing 
content-addressed and immutable-history 
philosophy. ──────── 43. SECURITY 
EVIDENCE SHOULD DISTINGUISH FACT FROM 
INTERPRETATION This distinction matters. 
Examples of relatively direct observations: 
```text NSCameraUsageDescription exists ``` 
```text dependency foo version 1.9.2 ``` 
```text binary contacts api.example.com ``` 
```text source digest = X ``` Examples of 
interpretation: ```text camera usage appears 
appropriate ``` ```text network behavior 
appears benign ``` ```text dependency risk 
seems low ``` Preserve this distinction in the 
data model where reasonable. Machine 
observations should be auditable. Human 
interpretation should be attributable. 
──────── 44. THREAT MODEL THE TRUST 
NETWORK This evolution is security-sensitive. 
Create an explicit threat model. At minimum 
consider: Identity attacks • fake Farcaster 
binding; • fake GitHub binding; • fake Base 
binding; • replayed identity proof; • username 
change; • compromised OAuth account; • 
compromised social account; • external service 
outage; • account deletion; • identity 
squatting. Builder attacks • DeviceKey 
compromise; • DeviceKey loss; • DeviceKey 
rotation; • malicious Builder; • Builder 
accumulates trust using benign releases then 
ships malware; • Builder changes source after 
review; • Builder publishes intentionally 
confusing release identifiers. Attestation 
attacks • reviewer signs wrong digest; • stale 
attestation shown on a new release; • 
copied/replayed attestation; • Sybil reviewers; 
• colluding reviewers; • paid positive reviews; 
• reviewer claims broader scope than performed; 
• compromised reviewer DeviceKey; • malicious 
UI tricks reviewer into signing different 
payload. Machine intelligence attacks • 
hallucination; • prompt injection embedded in 
source comments/files; • malicious README 
instructions; • adversarial source intended to 
manipulate an LLM; • dependency metadata 
poisoning; • obfuscated code; • runtime 
behavior differing from static analysis; • 
malicious build scripts; • remote model 
provider sees private source unexpectedly. 
Delivery attacks • Claim for release X installs 
release Y; • request replay; • relay mutation; 
• stale queued installation; • compromised 
relay; • compromised Mac; • wrong source; • 
wrong artifact; • source/build mismatch; • 
wrong physical phone; • accidental install on 
another person’s device; • ambiguous CoreDevice 
selection. Document mitigations and residual 
risks. ──────── 45. RELAY MUST NOT 
BECOME TRUSTED AUTHORITY The relay transports 
intent. It should not become capable of 
silently changing what software gets installed. 
Where compatible with current architecture, 
remote requests should bind enough immutable 
information that the Mac can verify: ```text 
request → intended Shot/release → exact 
digest → requesting Companion identity → 
intended action ``` The Mac should 
independently validate what it receives. The 
relay should not need access to private signing 
keys. Study the existing relay architecture and 
preserve its security model. ──────── 
46. REMOTE CLAIM MUST BE IDEMPOTENT A durable 
asynchronous system must avoid duplicate 
builds/installs. Scenario: ```text Claim Mac 
offline relay retries Mac reconnects network 
retries ``` The system must not accidentally 
perform the same intended action repeatedly. 
Review existing idempotency semantics. 
Strengthen them where necessary. Golden path 
expectations should include: ```text remote 
request executes exactly once ``` where 
“exactly once” may be implemented as idempotent 
at-least-once transport if appropriate. Use 
correct distributed-systems semantics. 
──────── 47. BUILD ONCE, INSTALL LATER 
If an app is successfully built while the 
target phone is unavailable: Do not throw away 
the artifact merely because installation cannot 
happen immediately. The system should be 
capable of: ```text build
  ↓ verified artifact retained ↓ Ready to 
Install
  ↓ device becomes reachable ↓ install ``` 
Study current artifact retention behavior. Do 
not rebuild unnecessarily if the already-built 
artifact remains valid. Make validity rules 
explicit: • release digest; • signing state; • 
provisioning expiration; • device 
compatibility; • developer certificate; • build 
configuration. If a retained artifact is no 
longer valid, transition honestly and 
rebuild/re-sign when appropriate. 
──────── 48. REACHABILITY SHOULD DRIVE 
DELIVERY The delivery system should react to 
device state changes. Conceptually: ```text 
pending install
      + associated target device + target 
becomes reachable
      ↓ attempt install ``` If installation 
fails because: ```text device locked ``` 
display that. If: ```text Developer Mode 
disabled ``` display that. If: ```text pairing 
missing ``` display that. If: ```text signing 
expired ``` display that. Do not reduce every 
failure to: ```text Connect cable ``` The UI 
should explain the actual condition. 
──────── 49. DO NOT PROMISE BACKGROUND 
MAGIC THAT IOS/MACOS CANNOT PROVIDE Be 
ambitious but truthful. Inspect what: • the Mac 
app can do while open; • the Mac app can do in 
background; • launch agents/services can do; • 
CoreDevice APIs/tools permit; • iOS permits; • 
Companion can receive while backgrounded; • 
relay can persist. If continuous behavior 
requires a Mac helper/service, architect it 
explicitly. Do not show: ```text Your Mac will 
automatically install this anytime forever ``` 
if the Mac must actually be awake with Tohseno 
running. Make the constraints visible without 
making the experience feel brittle. 
──────── 50. PRIVACY MUST BE EXPLICIT 
Trust networks can easily become surveillance 
systems. Classify information. At minimum 
distinguish: ```text PUBLIC PROTOCOL DATA ``` 
Examples may include: • public releases; • 
public Builder profile bindings; • public 
attestations. ```text PUBLIC REGISTRY METADATA 
``` Examples may include: • display names; • 
app metadata; • public review counts. ```text 
PRIVATE LOCAL DATA ``` Examples may include: • 
private apps; • local source; • local build 
information; • device associations. ```text 
PRIVATE TRUST DATA ``` Examples may include: • 
explicit “I trust Alice for privacy”; • 
personalized trust weights. ```text RELAY 
METADATA ``` Understand what relay servers 
learn. ```text OPTIONAL SOCIAL DATA ``` 
Understand what Farcaster/GitHub data is 
fetched and whether it is stored. Do not 
casually centralize: • complete user social 
graphs; • all software installed by a user; • 
private trust relationships; • stable 
physical-device identifiers; • private 
repositories; • raw source; • OAuth 
credentials. Prefer data minimization. 
──────── 51. FARCASTER GRAPH STORAGE 
SHOULD BE MINIMAL Ask: > Does Tohseno need to 
permanently copy someone’s entire Farcaster 
follow graph? Maybe not. If the graph can be 
queried when needed or maintained through 
existing node infrastructure, prefer that to 
duplicating unnecessary social data. If caching 
is needed, define: • purpose; • TTL; • 
invalidation; • privacy impact. Do not create a 
second social network database merely because 
the data is accessible. ──────── 52. 
GITHUB ACCESS SHOULD REQUEST MINIMAL 
PERMISSIONS If GitHub integration is 
implemented: • request only necessary scopes; • 
avoid private repository access unless 
explicitly needed and explained; • do not 
ingest arbitrary private source; • store 
durable identifiers; • handle username changes; 
• protect tokens; • support disconnect/revoke. 
If a GitHub App architecture is more 
appropriate than generic OAuth, evaluate it. 
Use official current GitHub documentation. 
──────── 53. APP RELEASE PAGES SHOULD 
BE EXACT Whenever possible, public deep links 
should resolve clearly to: ```text app identity 
+ current release ``` and the UI must make 
clear what is being Claimed. If: ```text 
tohseno.com/anky ``` resolves to the latest 
release, the Claim ceremony should still bind 
the exact immutable release selected at the 
moment of Claim. Avoid a race where: ```text 
user views release X Builder publishes Y user 
taps Claim user unknowingly claims Y ``` The UI 
and signed intent should resolve this 
precisely. ──────── 54. TRUST 
INFORMATION MUST FOLLOW THE EXACT RELEASE 
Likewise: If the user sees: ```text 18 
attestations ``` those attestations must 
correspond to the release they are about to 
Claim. If the latest release changes while the 
page is open, handle it explicitly. For 
example: ```text A newer release is available. 
You are currently viewing 1.4.2. [ View 1.4.3 ] 
``` Do not silently swap trust evidence 
underneath a user’s decision. ──────── 
55. UX LANGUAGE MUST BE CONSISTENT Prefer 
precise terms such as: • Builder • Shot • App • 
Release • Claim • Review • Attestation • 
Evidence • Verification • Your network • People 
you follow • Your iPhone • Pair • Reachable • 
Prepare • Build • Install Do not casually use 
the following as synonyms: ```text follow trust 
review verify claim install own approve safe 
``` Each means something different. Define the 
terms in product documentation. 
──────── 56. DO NOT OVERLOAD USERS WITH 
PROTOCOL LANGUAGE The architecture can be 
rigorous underneath while the product stays 
simple. A normal user should not need to 
understand: • content-addressed storage; • 
DeviceKey derivation; • FIDs; • EIP-712; • 
digest algorithms; • CoreDevice identifiers; • 
relay idempotency; • canonical serialization. 
Prefer: ```text Built by JP ``` with deeper 
detail available under: ```text Inspect 
provenance ``` Prefer: ```text 3 people you 
follow reviewed this release ``` with deeper 
detail under: ```text View attestations ``` 
Progressive disclosure. ──────── 57. DO 
NOT LOSE THE MAGIC The simple experience must 
remain: ```text friend sends app
      ↓ open ↓ see what it is ↓ see who made 
it
      ↓ see relevant trust context ↓ Claim ↓ 
Mac handles preparation
      ↓ iPhone gets the software ``` The trust 
architecture exists to make that simple flow 
defensible. It must not turn Claiming an app 
into completing a security certification exam. 
The normal path should be obvious. The deep 
path should be available. ──────── 58. 
EVOLVE THE EXISTING PROTOCOL ADDITIVELY WHERE 
POSSIBLE The repository already has historical 
protocol commitments. Respect them. If Shot, 
Ship, Claim, Release encodings, or canonical 
digests are frozen, do not mutate them 
casually. Prefer additive primitives equivalent 
to: ```text IdentityBinding VerificationReport 
ReleaseAttestation TrustPreference 
DeviceAssociation ``` where appropriate. If an 
existing primitive already correctly represents 
one of these concepts, extend/reuse it instead 
of creating duplication. For every protocol 
object, explicitly answer: • What is immutable? 
• What is signed? • Who signs it? • What is 
canonical? • How is it hashed? • Can it be 
replayed? • Can it be revoked? • Can it be 
superseded? • Can it expire? • What Builder 
does it belong to? • What release does it 
belong to? • Is it protocol-critical? • Is it 
derived/indexed? • Is it public? • Is it 
private? ──────── 59. MIGRATIONS MUST 
PRESERVE EXISTING USERS Existing: • Builders; • 
DeviceKeys; • apps; • Shots; • releases; • 
Claims; • pairings; • private apps; must 
continue working. Farcaster and GitHub are 
additive. Do not require existing Builders to 
connect either. Possible UI: ```text Complete 
your Builder identity [ Connect Farcaster ] [ 
Connect GitHub ] Optional ``` Do not gate basic 
Tohseno usage behind social accounts. Device 
association migrations must be conservative. If 
an existing Companion cannot be safely 
associated with a physical CoreDevice 
automatically, require a one-time explicit 
confirmation. Never guess an install 
destination. ──────── 60. 
IMPLEMENTATION PHASES After studying the 
repository, create a detailed execution plan. 
Adapt ordering if repository dependencies 
require it. The rough desired phases are: Phase 
A — Architecture and invariants Write ADRs or 
equivalent architecture documentation for: 1. 
Network Trust and Release Attestations 2. 
Wireless-First Apple Delivery Potentially 
separate identity binding into another ADR if 
the existing architecture warrants it. Update 
the working paper where appropriate. Do not 
claim functionality exists before it exists. 
Clearly distinguish: ```text implemented 
partially implemented designed future ``` Phase 
B — Wireless-first domain model Implement the 
underlying state model necessary for: • device 
reachability; • persistent install intent; • 
durable target association; • deferred 
installation; • resumption; • correct failure 
states; • multiple visible devices. Remove 
cable-specific domain assumptions where they 
are not actually fundamental. Phase C — Durable 
Companion ↔ device association Implement or 
formalize: ```text Companion identity
        ↕ physical install target ``` Include: 
• persistence; • migration; • 
reset/reassociation; • multiple-device safety; 
• lost/replaced device behavior; • tests. Phase 
D — Wireless-first delivery Make: ```text Claim 
anywhere → Mac prepares → artifact retained 
→ associated iPhone becomes reachable → 
install ``` a genuine supported product path. 
Reuse existing relay infrastructure. Do not 
replace the private Mac factory with cloud 
execution. Phase E — Wireless-first UX Update 
Mac and Companion UX so: • 
wireless/reachability is default; • USB appears 
only when required; • actual states are 
communicated; • remote Claims have 
understandable progress; • installation 
destination is clear. Phase F — External 
identity binding foundation Build the 
domain/protocol model for verifiable external 
identity bindings. Support architecture for: • 
Farcaster; • GitHub; • Base; • optional X. 
Implement the integrations that can be 
completed correctly in this pass. Do not fake 
unavailable connections. Phase G — Farcaster 
connection Implement the first-class social 
binding. At minimum: • stable FID binding; • 
display metadata; • PFP; • username; • 
connection/disconnection; • verification of 
binding; • social graph retrieval where 
appropriate. Use existing Hypersnap/Snapchain 
infrastructure if appropriate. Phase H — GitHub 
connection Implement the technical identity 
binding. At minimum: • durable account 
identifier; • username; • profile; • verified 
connection; • disconnect; • secure token 
handling. Expose source/repository provenance 
where already reasonably available. Do not turn 
this into broad GitHub analytics. Phase I — 
Verification Report Formalize machine evidence 
around an exact release. Reuse existing 
verification pipeline. Generate deterministic 
structured output where possible. Create a 
canonical digest for the report if appropriate. 
Phase J — Release Attestation Allow a reviewer 
to: ```text select exact release
        ↓ inspect Verification Report ↓ 
choose review scopes/outcome
        ↓ approve on Companion ↓ sign exact 
canonical payload
        ↓ publish/store attestation ``` The 
attestation must never accidentally attach to 
another release. Phase K — Trust-aware Registry 
UX Expose: • Builder identity; • external 
bindings; • exact release; • provenance; • 
machine observations; • Release Attestations; • 
personalized Farcaster-follow context. Do not 
invent nonexistent data. Phase L — Tests and 
owner-attended evidence Extend automated tests 
and physical-device test scripts. A feature is 
not “done” merely because the architecture 
compiles. Produce real evidence. 
──────── 61. REQUIRED GOLDEN PATH A — 
BUILDER SHIPS Verify: ```text Builder
    ↓ creates/updates app ↓ Ship exact 
release
    ↓ Companion DeviceKey authorizes ↓ 
release becomes canonical
    ↓ Registry displays exact release ``` 
Preserve existing behavior. ──────── 
62. REQUIRED GOLDEN PATH B — EXTERNAL IDENTITY 
Verify: ```text Builder
    ↓ Connect Farcaster ↓ binding ceremony ↓ 
stable FID bound
    ↓ Builder profile shows verified Farcaster 
identity ``` Repeat conceptually for GitHub. 
Then verify: ```text disconnect Farcaster ``` 
does not remove or transfer Builder authority. 
──────── 63. REQUIRED GOLDEN PATH C — 
SOCIAL TRUST CONTEXT Verify: ```text Recipient 
opens app release
        ↓ release has attestations ↓ 
recipient has connected Farcaster
        ↓ some reviewers are followed by 
recipient
        ↓ UI distinguishes them ``` Example: 
```text 18 builders reviewed this release. 3 
are people you follow. ``` This must not imply 
that all follows are explicit Tohseno trust 
relationships. ──────── 64. REQUIRED 
GOLDEN PATH D — CLAIM WHILE MAC ONLINE Verify: 
```text Recipient opens release
        ↓ Claim ↓ durable request reaches 
correct Mac
        ↓ exact release resolved ↓ 
verification/build runs
        ↓ artifact retained ↓ correct 
associated iPhone receives install ``` 
──────── 65. REQUIRED GOLDEN PATH E — 
CLAIM WHILE MAC OFFLINE Verify: ```text 
Recipient Claims
        ↓ Mac offline ↓ request persists ↓ 
Mac later returns
        ↓ request executes ↓ exact release 
builds ``` No duplicate execution. No lost 
Claim intent. No wrong release. 
──────── 66. REQUIRED GOLDEN PATH F — 
IPHONE UNREACHABLE Verify: ```text build 
finishes
        ↓ intended iPhone unavailable ↓ 
artifact remains Ready to Install
        ↓ iPhone becomes reachable later ↓ 
installation resumes ``` The user should not 
have to repeat the entire Claim/build process. 
──────── 67. REQUIRED GOLDEN PATH G — 
MULTIPLE IPHONES Verify: ```text Mac sees: 
Phone A Phone B Phone C ``` A request 
originating from Companion A must install on 
physical Phone A. No accidental installation 
onto B or C. If association is unavailable or 
ambiguous, the system must stop and request 
explicit resolution. ──────── 68. 
REQUIRED GOLDEN PATH H — RELEASE REVIEW Verify: 
```text Release X exists
        ↓ Mac creates Verification Report ↓ 
reviewer inspects evidence
        ↓ reviewer chooses scopes ↓ Companion 
signs ReleaseAttestation
        ↓ attestation becomes visible on 
Release X ``` Verify signature and digest 
integrity. ──────── 69. REQUIRED GOLDEN 
PATH I — UPDATE DOES NOT INHERIT REVIEWS 
Verify: ```text Release X 10 attestations ``` 
Then: ```text Builder ships Release Y ``` 
Expected: ```text Release Y 0 attestations ``` 
while: • Release X still shows its 10 
historical attestations; • Builder history 
remains; • reviewer history remains; • social 
context remains. No stale review propagation. 
──────── 70. REQUIRED GOLDEN PATH J — 
SOCIAL ACCOUNT DOES NOT CONTROL BUILDER Test 
that possession of: • Farcaster; • GitHub; • 
Base wallet; • X; without the Builder DeviceKey 
does not independently permit: • Ship; • 
Update; • release signing; • Builder authority 
changes; • attestation signing as that Builder. 
──────── 71. REQUIRED GOLDEN PATH K — 
CABLE FALLBACK Test both: ```text 
wireless-capable / already-paired device ``` 
and: ```text environment requiring initial 
cable bootstrap ``` The first should not prompt 
for USB. The second should explain why USB is 
necessary. After successful pairing, supported 
wireless behavior should become the default. 
──────── 72. REQUIRED FAILURE STATES 
Test and document: ```text Mac offline relay 
unavailable wrong device reachable device 
locked Developer Mode disabled pairing missing 
signing expired build failed source digest 
mismatch verification failed attestation 
signature invalid identity binding invalid 
Farcaster unavailable GitHub unavailable OAuth 
token revoked multiple ambiguous devices stale 
release new release published during Claim ``` 
No generic: ```text Something went wrong ``` 
when the system has actionable information. 
──────── 73. OBSERVABILITY Add enough 
structured diagnostics to understand real 
behavior. Especially around: ```text remote 
request created remote request received request 
identity requested release Mac processing 
started source resolved source verified 
verification started verification completed 
build started build completed artifact retained 
intended target target reachable/unreachable 
installation deferred installation resumed 
installation succeeded identity binding created 
identity binding verified Verification Report 
created Release Attestation signed Release 
Attestation published ``` Never log: • private 
keys; • signing secrets; • OAuth tokens; • 
access tokens; • passwords; • recovery phrases; 
• unnecessary private source; • raw private 
user content. ──────── 74. CURRENT 
APPLE REALITY MUST BE VERIFIED Do not rely 
solely on assumptions in this prompt about 
Apple behavior. Before implementing 
version-sensitive logic, consult current 
official Apple documentation. Verify current 
behavior around: • Xcode; • CoreDevice; • 
wireless device discovery; • first pairing; • 
Wi-Fi installation; • Developer Mode; • 
developer certificates; • provisioning; • 
signing; • free provisioning versus paid 
Developer Program; • device trust; • iOS 
version requirements. Encode capability checks 
where reasonable. Document which limitations 
come from Apple rather than Tohseno. 
──────── 75. DO NOT MAKE CLOUD BUILDS 
THE DEFAULT The current direction remains: > 
software is built and signed by the recipient’s 
own Mac. Do not respond to remote Claim by 
moving builds into a centralized Tohseno 
server. The private Mac factory is a feature. 
The relay coordinates. The Mac executes. The 
Companion authorizes. The recipient’s Apple 
environment signs/installs. Preserve that. 
──────── 76. DO NOT MAKE TOKENOMICS A 
BLOCKER This architectural pass should make the 
verification economy possible. It does not need 
to finalize it. Do not spend the majority of 
implementation effort building: • staking; • 
slashing; • emission schedules; • LP 
management; • token dashboards; • governance. 
The real primitive we need first is: ```text 
humans performing useful review work + 
cryptographic attribution + exact-release 
attestations + behavioral history ``` Token 
economics can wrap around behavior once 
behavior exists. ──────── 77. DO NOT 
MAKE BLOCKCHAIN VISIBLE WHERE IT DOES NOT HELP 
A normal user should not need to know: ```text 
chain ID transaction hash ABI EIP-712 type 
contract generation ``` to understand: ```text 
JP built this. Sofia reviewed this exact 
release. Your Mac will prepare it. ``` Keep 
cryptographic details accessible for 
inspection. Do not require users to understand 
them. ──────── 78. DOCUMENT THE NEW 
CONCEPTUAL STACK The repository should 
eventually be able to explain the architecture 
approximately as: ```text DEVICEKEY Sovereign 
Builder authority FARCASTER Social identity + 
existing relationship graph GITHUB Technical 
identity + source provenance BASE Economic 
identity + future verification economy TOHSENO 
HISTORY Behavior + releases + reviews + 
attestations MAC Private factory + intelligence 
workbench + delivery node COMPANION Human 
authority + consent REGISTRY Public 
artifact/provenance/trust view ``` This 
distinction should be clear enough that future 
contributors do not accidentally collapse the 
layers. ──────── 79. DOCUMENT THE NEW 
TRUST MODEL The repository should explicitly 
capture: > Tohseno does not certify arbitrary 
software as safe. Instead, Tohseno provides: 1. 
artifact identity; 2. Builder provenance; 3. 
signed release history; 4. machine-generated 
verification evidence; 5. human Release 
Attestations; 6. social context around 
reviewers; 7. behavioral history; 8. final user 
authority. This is the trust model. 
──────── 80. DOCUMENT THE NEW 
DISTRIBUTION MODEL The repository should also 
explicitly capture: > Installation is 
destination-driven, not cable-driven. 
Conceptually: ```text Claim
  ↓ durable intent ↓ Mac preparation ↓ 
verified built artifact
  ↓ associated install target ↓ wait for 
reachability
  ↓ install ``` USB and Wi-Fi are transports. 
The user’s physical iPhone is the destination. 
──────── 81. WEBSITE / LANDING-PAGE 
IMPLICATIONS Do not necessarily redesign the 
entire marketing site unless it is within the 
current task scope. However, update product 
language where necessary to avoid contradicting 
the architecture. The core message is no longer 
merely: ```text Skip the App Store ``` That can 
remain an important provocative expression. But 
Tohseno now has a more complete explanation: > 
Build software person-to-person. > > See who 
made it. > > See what it does. > > See who 
reviewed it. > > Claim it anywhere. > > Your 
own Mac prepares it. > > Your own devices 
remain under your authority. And especially: > 
**Software you can trust because people you 
trust have seen it.** Do not over-market future 
capabilities that are not yet implemented. 
──────── 82. KEEP CURRENT RELEASE 
READINESS HONEST Do not retroactively mark 
unfinished physical-device paths as complete 
merely because code exists. Preserve the 
repository’s discipline around evidence. 
Distinguish: ```text unit tested integration 
tested simulator tested local Mac tested 
physical iPhone tested remote tested 
owner-attended tested clean-Mac tested ``` 
Update readiness/state files honestly. 
──────── 83. TEST THE ACTUAL DEEP LINK 
FLOW A major desired user experience is: 
```text tohseno.com/anky ``` opened from: • 
Messages; • Farcaster; • X; • Safari; • another 
application. Study the current 
universal-link/deep-link architecture. The 
desired behavior on an iPhone with Companion 
installed is: ```text public app URL
        ↓ Companion ↓ exact app/release 
context ``` The desired behavior when Companion 
is unavailable must also be coherent. Do not 
invent behavior unsupported by Apple. Test the 
real path. ──────── 84. CLAIM SHOULD 
CAPTURE THE EXACT RELEASE SEEN When the user 
sees an app through a deep link, ensure the 
Claim ceremony resolves a stable exact release. 
Conceptually: ```text URL
   ↓ Registry resolves app ↓ release X 
displayed
   ↓ Claim binds release X ``` Not: ```text 
URL
   ↓ user reads X ↓ new Y published ↓ Claim 
silently installs Y ``` Concurrency matters. 
Design this correctly. ──────── 85. 
TRUST DATA SHOULD BE AVAILABLE BEFORE CLAIM The 
user should ideally see the most relevant trust 
evidence before deciding to Claim. The flow 
should not be: ```text Claim first then learn 
what it does ``` It should be: ```text 
encounter
        ↓ inspect enough context ↓ Claim ``` 
Claim remains lightweight. Deep inspection 
remains optional. ──────── 86. 
REVIEWING SHOULD BE A FIRST-CLASS NETWORK 
ACTION A Builder profile should eventually be 
shaped not only by: ```text software created 
``` but also: ```text intelligence contributed 
``` This is important. The network is not only: 
```text builders shipping apps ``` It is: 
```text builders shipping apps + people helping 
everyone understand those apps ``` Someone may 
eventually become highly respected inside 
Tohseno because they are an exceptional 
reviewer, even if they rarely publish apps 
themselves. Design reputation history so this 
can happen. ──────── 87. SECURITY 
SPECIALIZATION SHOULD REMAIN POSSIBLE Do not 
hardcode the assumption that every reviewer is 
equally authoritative about every domain. 
Future reviewers may specialize in: • 
Swift/iOS; • privacy; • cryptography; • payment 
security; • smart contracts; • networking; • 
accessibility; • dependency analysis; • AI 
behavior; • data handling. The first 
implementation does not need a full expertise 
ontology. But the scoped attestation design 
should leave room for it. ──────── 88. 
BEHAVIORAL REPUTATION SHOULD BE AUDITABLE If 
the UI eventually says something like: ```text 
Experienced privacy reviewer ``` there should 
be inspectable underlying history. Avoid opaque 
algorithmic reputation when possible. Prefer: 
```text 43 privacy-scoped attestations 6 
confirmed privacy findings ``` to: ```text 
Privacy score: 873 ``` Transparency fits the 
network. ──────── 89. HANDLE DEVICEKEY 
ROTATION Study how DeviceKey rotation currently 
works or should work. The new architecture must 
define what happens to: • Builder identity; • 
old releases; • old attestations; • external 
identity bindings; • device associations; • 
reputation history. Historical signatures by 
old valid DeviceKeys must remain interpretable. 
Rotation should not erase history. A revoked 
key must not continue signing new authoritative 
actions. ──────── 90. HANDLE EXTERNAL 
IDENTITY CHANGES For Farcaster: • username may 
change; • PFP may change; • account state may 
evolve according to protocol semantics. For 
GitHub: • username may change; • account may be 
deleted; • installation may be revoked. For 
Base: • user may want multiple addresses; • 
address control remains cryptographic but 
preferred economic identity can change. Design 
bindings around stable identifiers. Treat 
display names and avatars as mutable cached 
metadata. ──────── 91. DO NOT CREATE 
SILENT SECURITY DELEGATION Connecting Farcaster 
must not mean: ```text everyone I follow can 
approve apps for me ``` Connecting GitHub must 
not mean: ```text everyone who contributed to 
my repo can approve releases ``` Holding tokens 
must not mean: ```text I have voting authority 
over someone's phone ``` Security authority 
must remain explicit. Personal trust may become 
a recommendation signal. Installation authority 
remains with the user. ──────── 92. THE 
RECIPIENT RETAINS FINAL AUTHORITY The 
philosophical symmetry should be maintained. 
The Builder says: > I, holder of this Builder 
authority, publish this exact release. The 
reviewer says: > I, holder of this 
Builder/reviewer authority, attest to this 
bounded review of this exact release. The 
recipient says: > I have seen the available 
evidence and choose to run this software on my 
device. No centralized Tohseno actor needs to 
substitute for these humans. Tohseno 
coordinates the evidence and actions. 
──────── 93. NON-GOALS FOR THIS PASS Do 
not allow this evolution to explode 
uncontrollably. Unless foundationally 
necessary, do not fully implement: • final 
$TOHSENO tokenomics; • staking/slashing; • DAO 
governance; • global decentralized identity 
standards; • global reputation scores; • social 
feeds; • public follower leaderboards; • 
recommendation algorithms; • a Farcaster clone; 
• a GitHub analytics product; • perfect malware 
detection; • formal verification of arbitrary 
Swift; • centralized cloud builds; • arbitrary 
remote code execution; • every Apple platform; 
• generalized anonymous security markets; • 
complex expertise taxonomies; • fully 
autonomous AI reviewers presented as humans. 
Leave deliberate seams for future work. 
──────── 94. QUALITY BAR This is an 
architectural evolution, not a hackathon patch. 
For each meaningful feature: 1. understand 
current behavior; 2. define invariant; 3. 
document architecture; 4. update domain model; 
5. update canonical data model if needed; 6. 
implement persistence; 7. implement networking; 
8. implement cryptography/signing where 
required; 9. implement UX; 10. implement 
migrations; 11. test happy path; 12. test 
failure states; 13. test security assumptions; 
14. update documentation; 15. update 
readiness/state honestly. Do not keep two 
contradictory models alive. Examples: Do not: 
```text rename "Connect cable" to "Find iPhone" 
``` while the underlying domain still 
fundamentally assumes a cable. Do not: ```text 
add Farcaster icon ``` without a meaningful 
identity binding. Do not: ```text show 
"Reviewed" ``` without an exact-release signed 
attestation. Do not: ```text show "Safe" ``` 
because a model returned no findings. Do not: 
```text show reputation ``` without evidence 
underneath it. ──────── 95. CLEAN UP 
OBSOLETE ARCHITECTURAL LANGUAGE After 
implementation, search the repository for stale 
assumptions. Especially: ```text cable required 
USB required connect cable waiting for cable 
one connected phone safe app verified app 
trusted app ``` Determine whether each instance 
remains technically true. Update: • UI copy; • 
docs; • comments; • state enums; • tests; • 
diagrams; • README; • whitepaper; • onboarding. 
Do not delete historically important ADR 
content merely because the architecture 
evolved. Historical documents can remain 
historical. Current docs should be clear. 
──────── 96. CREATE A DECOMPRESSION 
REPORT WHEN FINISHED At the end of the work, 
produce a detailed architectural 
decompression/evolution report in the style 
appropriate for this repository. The report 
must describe: What you found Explain the 
starting architecture after repository study. 
Identify important discrepancies between docs 
and implementation. What changed Cover: • 
protocol; • domain model; • Mac; • Companion; • 
relay; • delivery; • CoreDevice; • 
registry/web; • identity; • Farcaster; • 
GitHub; • Base binding; • verification; • 
attestations; • UX; • tests. What invariants 
were preserved Especially: • DeviceKey 
authority; • Claim semantics; • immutable 
exact-release identity; • local/private 
factory; • recipient authority; • historical 
protocol commitments. What is genuinely working 
Separate: • unit evidence; • integration 
evidence; • simulator evidence; • local Mac 
evidence; • physical-device evidence; • remote 
relay evidence; • owner-attended evidence; • 
clean-Mac evidence. What is designed but not 
implemented Be extremely precise. Do not blur: 
```text architecture exists ``` with: ```text 
user can do this today ``` Remaining risks At 
minimum: • Apple pairing restrictions; • 
wireless reliability; • device-association 
correctness; • Farcaster availability; • GitHub 
availability; • identity proof weaknesses; • 
Sybil reviewers; • malicious Builders; • 
malicious updates; • review collusion; • 
machine-analysis limitations; • prompt 
injection; • privacy; • eventual token 
incentives. Next smallest valuable step Do not 
conclude by proposing another giant rewrite. 
Identify the smallest next milestone that most 
increases the amount of this vision that is 
actually true for a real human on a real Mac 
and real iPhone. ──────── 97. THE 
PRODUCT STORY AFTER THIS EVOLUTION The 
repository should be able to explain Tohseno 
approximately like this: > Tohseno is a 
person-to-person software network. > > Builders 
ship software under cryptographic authority 
held on their iPhone. > > Anyone can encounter 
and Claim a published app. > > Their own Mac 
verifies, builds, and prepares the software. > 
> Their iPhone receives it when it is 
reachable. > > A cable is used only when Apple 
actually requires it. > > The network helps 
people decide what to run by exposing 
provenance, machine evidence, and attestations 
from other people. > > Farcaster supplies 
portable social context. > > GitHub supplies 
technical identity and provenance. > > Base 
supplies an economic identity without 
controlling Builder authority. > > Reputation 
comes from behavior. > > The final authority 
belongs to the person running the software. And 
at the center: > **Software you can trust 
because people you trust have seen it.** 
──────── 98. THE ARCHITECTURAL FLYWHEEL 
Keep this larger system in mind: ```text CREATE
   ↓ SHIP ↓ RELEASE ↓ VERIFY ↓ REVIEW ↓ 
ATTEST
   ↓ CLAIM ↓ BUILD ↓ INSTALL ↓ USE ↓ MORE 
HISTORY
   ↓ MORE REPUTATION ↓ BETTER TRUST SIGNALS 
   ↓
MORE SOFTWARE CAN MOVE PERSON-TO-PERSON ``` 
Every shipped artifact expands the software 
registry. Every verification creates evidence. 
Every review contributes intelligence. Every 
attestation creates accountable history. Every 
Claim expands the distribution graph. Every 
successful build strengthens the 
private-factory model. Every installation gives 
another human sovereign access to software they 
deliberately chose to run. The network effect 
we care about is not attention for its own 
sake. It is trust accumulating around software. 
──────── 99. IMPLEMENT THE SMALLEST 
COHERENT VERSION FIRST This prompt describes 
the direction of Tohseno, not a demand to 
irresponsibly ship every possible future 
feature in one commit. After studying the 
repository, identify the smallest coherent 
slice that makes the two new truths materially 
more real: 1. wireless-first delivery; 2. 
network-mediated trust. A strong first 
milestone may look like: ```text WIRELESS 
remote Claim → durable Mac work → durable 
install intent → Companion ↔ physical-device 
association → install when target becomes 
reachable ``` plus: ```text TRUST Builder 
external identity bindings → first 
deterministic Verification Report → first 
exact-release human Attestation → Registry 
displays attestation → Farcaster relationship 
context ``` Do not ship a fake broad system 
when one honest narrow vertical slice can prove 
the architecture. Prefer end-to-end truth over 
feature count. ──────── 100. FINAL 
NORTH STAR The system we are building should 
eventually make this flow feel obvious: ```text 
Someone sends me an app.
        ↓ I see who built it. ↓ I see where 
the software came from.
        ↓ I see what machines observed. ↓ I 
see who reviewed this exact release.
        ↓ I see which of those people I 
already know or trust.
        ↓ I Claim it. ↓ My Mac privately 
verifies and builds it.
        ↓ The artifact waits durably if 
necessary.
        ↓ My intended iPhone becomes 
reachable.
        ↓ The correct device receives the 
software.
        ↓ I retain final authority over 
whether I run it. ``` The product should feel 
simple. The machinery underneath it should be 
rigorous. The network should not replace human 
judgment. It should make human judgment better 
informed, more accountable, and more connected. 
The Mac should not disappear. It should become 
the private factory and intelligence workbench. 
The Companion should not disappear. It should 
become the human authority surface. Farcaster 
should not become the root identity. It should 
become the first-class social context. GitHub 
should not become a reputation score. It should 
become technical identity and provenance. Base 
and $TOHSENO should not become the truth layer. 
They should become the economic layer around 
useful work. And a cable should not define the 
product. It should be used only when the 
underlying Apple environment genuinely requires 
it. The deepest product promise remains: > 
**Software you can trust because people you 
trust have seen it.** And the deepest 
distribution promise becomes: > **Claim 
anywhere. Your Mac prepares it. Your iPhone 
receives it when it is reachable.**
Build toward making those statements literally 
