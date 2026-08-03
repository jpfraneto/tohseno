# ADR 0011: Accept an intention in the browser and give it a body locally

Status: accepted

Date: 2026-08-03

## Context

The public entry point previously asked a person to install TOHSENO before it
accepted what they wanted. Installation and onboarding can take time, and a
browser-only draft cannot cross into a newly installed local process through a
single shell command. Re-entering the prompt or references would make loss and
divergence part of the first ceremony.

This transport has four different objects. A **Browser Draft** is editable
private browser state. A **Pending Relay Intention** is a temporary encrypted
package that the relay cannot decrypt. A **Local Pending Intention** is a
durably imported, validated package in the machine data root. A **Shot** is
created only by the existing engine after local review and approval. An
**Evolution** remains an immutable continuation of that Shot. The first three
are not Shots and have no Shot identity.

## Decision

The landing page accepts prompt text and up to eight locally supported image
references before installation. IndexedDB preserves the editable Browser
Draft. Pressing **TAKE A SHOT** freezes a distinct transfer snapshot, builds
the small, versioned, noncanonical `tohseno.intent-package/1` transport
package, encrypts the complete package in the browser with a fresh AES-256-GCM
key and nonce, and uploads bounded ciphertext chunks.

Three independent high-entropy bearer capabilities authorize upload, status,
and claim. The relay stores only their SHA-256 verifiers. The copied `ti1`
claim token contains an opaque relay ID, claim capability, and AES key. It has
no origin, account, app name, Shot ID, or protocol authority. The authenticated
relay lease supplies the nonce and ciphertext metadata. The CLI accepts only
the fixed official HTTPS relay; a loopback HTTP override exists only in debug
and test builds.

The claim-capable installer installs an immutable verified release through the
existing release chain, gives the token to the CLI over stdin, and never puts
it in another child argument. The CLI leases, downloads, verifies, decrypts,
parses with shared engine rules, and atomically imports a Local Pending
Intention under the durable machine data root. It acknowledges completion only
after that import, causing synchronous ciphertext deletion and leaving a
short-lived metadata tombstone. The Studio URL contains only the local opaque
pending ID. Studio resolves the private content server-side and reuses the
existing planning, genome, harness, preparation, and terminal boundary. Only a
successful Shot preparation consumes the pending record.

There is no account because possession of the one-time claim capability is the
authorization. The package is deliberately transport data, not a protocol
object, signed action, canonical Shot package, owner record, or public
witness. It does not change protocol schemas or identity derivation.

Installer release ordering is part of this security boundary. Production
handoff remains disabled unless durable relay storage, an HTTPS canonical
origin, explicit relay enablement, and an explicit claim-installer-ready flag
are all present. That last flag may be set only after the matching immutable
claim-capable release exists and the public installer pin has been verified.

The design is additive to the local-first product. Generic installation,
inline Studio composition, `tohseno create`, and portable Shot bundles retain
their current meanings. A browser-created `.tohseno-intent` download is the
offline fallback; it is private but not encrypted.

## Alternatives considered

- Browser storage alone preserves a draft but cannot deliver it to the newly
  installed CLI in one command.
- Manual download plus `tohseno intent open` is robust and remains available,
  but makes file handling the default ceremony.
- Direct browser-to-localhost transfer avoids relay storage but is unreliable
  before Studio exists and creates difficult origin, port, discovery, and
  local-network permission boundaries.
- Accounts or email links would introduce identity, recovery, tracking, and
  control surfaces that are unnecessary for a single-use private transfer.
- Creating a Shot on the website would violate the engine's local approval,
  identity, genome, and execution boundaries.

## Consequences and residual risks

The relay can observe transport IP, timing, ciphertext size, chunk count,
expiry, and state, but cannot see plaintext, filenames, supplied image
metadata, the AES key, or a canonical Shot identity. Ciphertext expires after
seven days; incomplete uploads expire after one hour; claim leases last
fifteen minutes.

XSS in the origin could steal draft bytes, keys, or capabilities. Anyone who
copies the bearer token before claim can import the intention. The pasted
command may remain in shell history after its single-use token expires.
Compromise of the local Mac exposes locally decrypted material. The relay can
be denied service, fill its bounded capacity, or fail to delete storage at the
filesystem or infrastructure layer despite the application's synchronous
deletion rule. TLS, browser-origin hardening, short-lived single-use
capabilities, bounded storage, durable tombstones, strict parsing, and local
idempotence reduce these risks; they do not make the relay abuse-proof.
