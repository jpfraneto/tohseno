# Storage

TOHSENO Apple apps are local first.

- Use SwiftData for durable structured domain state when it fits the domain.
- Use `UserDefaults` only for small, non-secret preferences.
- Use Keychain for small secrets and persistent key references.
- Use Secure Enclave for eligible signing authority.
- Use app-container files for documents, exports, and larger opaque content.
- Use atomic writes and Apple data protection for durable files.

`LocalPersistence.swift` provides a traversal-safe, atomic local file store and
a deliberately small preferences wrapper. It does not add cloud behavior.

CloudKit is optional. An app using it must declare
`private_cloudkit_sync`, identify its containers, explain the user-visible
purpose, remain usable before sign-in where the domain permits, and never make
Apple ID part of TOHSENO ownership.

No prompt, reference image, unpublished Shot, continuity link, private app
content, or recovery material is uploaded by the Fascia.
