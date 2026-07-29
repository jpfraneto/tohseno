# TOHSENO Apple identity bridge

`tohseno-apple-identity` is the narrow Apple-native bridge used by the TOHSENO
factory to create and use a Builder DeviceKey without exporting its private
material.

The default backend creates a permanent P-256 key in Secure Enclave and stores
only its Keychain persistent reference as metadata:

```sh
tohseno-apple-identity create --tag org.tohseno.builder.device.mac
tohseno-apple-identity public --tag org.tohseno.builder.device.mac
tohseno-apple-identity sign --tag org.tohseno.builder.device.mac --digest 0x<64-hex>
tohseno-apple-identity delete --tag org.tohseno.builder.device.mac
```

Every successful command emits one JSON object using
`tohseno.apple-identity/1`. Failures emit a JSON error to standard error and
return non-zero. Private keys, recovery material, and raw Keychain persistent
references are never returned.

For CI, Simulator work, or a Mac without Secure Enclave, creation may be made
explicitly with:

```sh
tohseno-apple-identity create --tag test.example --backend software-test
```

Software-test keys are separate Keychain items labelled
`TOHSENO SOFTWARE TEST KEY — NOT PRODUCTION`. Every response carries
`"test_only": true` and `"security_level": "software_test"`. There is no
automatic downgrade from Secure Enclave to this backend.

The helper signs an already-computed 32-byte SHA-256 digest with
`ecdsaSignatureDigestX962SHA256`, converts Apple DER signatures to fixed-width
`r` and `s`, and normalizes `s` to the lower half of the P-256 group order.
The fixed-width values are big-endian 32-byte hexadecimal strings.

Build and test:

```sh
swift build --package-path apple-identity
swift test --package-path apple-identity
```
