# Identity

TOHSENO keeps four identity concepts separate.

`BuilderID` is the durable public controller of a Shot. A Builder DeviceKey is
one replaceable authority accepted by that BuilderID. Apple signing readiness
is an Xcode and Apple Developer concern. An `InstallationIdentity` belongs to
one installation of one generated app.

The generated app creates its InstallationIdentity automatically on first
launch. It uses an app-sandboxed P-256 key in Secure Enclave where available
and retains only an opaque representation in a non-synchronizing,
ThisDeviceOnly Keychain item. Simulator and unavailable-hardware fallback keys
are software P-256 keys protected by the same ThisDeviceOnly Keychain policy.
They are installation authority only, never Builder authority.

The public installation identifier is the 32-byte value:

```text
SHA-256(
  UTF-8("TOHSENO-INSTALLATION-ID-V1") || 0x00 ||
  public_key_x_32 || public_key_y_32
)
```

It is encoded as lowercase `0x`-prefixed hexadecimal.

No shared Keychain access group is allowed. The private key, opaque Secure
Enclave representation, and software fallback representation are never
returned by the public interface.

An InstallationIdentity is not automatically linked to a BuilderID, Apple ID,
username, server account, another app, or another installation. Reinstall
behavior follows Apple’s Keychain retention semantics; an explicit local reset
creates a new installation relationship and must not be presented as the same
identity.

Builder recovery material and Builder DeviceKeys must never be compiled,
copied, derived, or imported into a generated app.
