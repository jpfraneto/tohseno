# Protocol privacy boundary

TOHSENO is local and private by default. Publication is an explicit,
Builder-authorized transition; a private/local Shot does not enter the public
Builder graph merely because it exists.

The optional web-to-local handoff is a transport boundary, not publication.
Before **TAKE A SHOT**, the Browser Draft remains in browser memory and
IndexedDB. After that explicit action, the browser sends only AES-256-GCM
ciphertext to the temporary relay. The relay never receives the key or
decryptable prompt/reference bytes, and it never creates a Shot. The Mac
decrypts and validates the package into durable local pending state before the
relay deletes its ciphertext. The canonical Shot remains local and is created
only by the engine after local approval.

The operator may unavoidably observe request IP, timing, ciphertext byte size,
chunk count, expiry, and relay state. Incomplete uploads expire after one hour;
ready ciphertext expires after at most seven days and is deleted after durable
import. The copied command contains a high-entropy, single-use bearer token;
anyone possessing it before claim can import the package, and the original
pasted command may remain in shell history after expiry. A downloaded
`.tohseno-intent` package is readable private material, not encrypted. There
are no site accounts, analytics, or model-inference calls during this handoff.

## Never on-chain

No contract action or registry head may be constructed from:

- InstallationIdentity or continuity statements;
- end-user identity, behavior, content, or usage;
- private feedback, references, or attachments;
- raw private intentions or agent material;
- hashes or commitments derived from any of the above.

A hash over a small or guessable private domain is disclosure, not privacy.
Application-runtime code has no contract-publication authority.

## Narrow public witness

The successor registry can expose only:

- independent random Shot ID;
- Builder controller;
- a digest of the narrow `tohseno.public-checkpoint/1` identity-continuity
  projection;
- witness-local checkpoint count;
- action nonce and registration timing.

The public checkpoint starts a separate chain at witness sequence 1. It binds
only its witness generation/chain/registry, ShotID, prior public checkpoint,
fixed scope, and newly declared publication time. It does not contain or
reference the local coherent-intention lineage, expression/version state,
genome, source/build artifacts, feedback, token relations, runtime data,
content, controllers, or free text.

Builder identity is deliberately linkable after explicit publication. End-user
installation identity stays local and unlinkable by default; it is never a
registry controller and the two identity graphs never touch.

The undeployed handle, Appcoin, App Store attestation, `publicState`, and
generic `contentCommitment` contract surface was removed. Token Association is
an optional signed protocol relationship and is not Shot identity or
ownership.

The retired mixed-ancestry public-action outbox is write-disabled. Marking one
ordinary lineage action public does not make its `previous` commitment safe to
disclose. Existing outbox files are preserved as legacy evidence; new Token
Associations remain intentionally private until a closed, ancestry-free public
relation schema exists.

## Immutable Apple Fascia

`fascia/apple/PRIVACY.md` is part of the already committed Apple Fascia tree,
so changing it would change the immutable Fascia digest carried by existing
Shot fixtures. Its v0.7 public-contract list is retained as historical signed
material. This document and ADR 0006 define the successor protocol boundary;
the next accepted Fascia revision must incorporate it through an explicit
versioned migration rather than silently rewriting sealed artifacts.
