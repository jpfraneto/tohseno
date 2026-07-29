import CryptoKit
import Foundation
import Security

public enum AppleIdentityBackend: String, Codable, Sendable {
    case secureEnclave = "secure_enclave"
    case softwareTest = "software_test"

    public var testOnly: Bool {
        self == .softwareTest
    }

    public var securityLevel: String {
        rawValue
    }
}

public struct P256PublicKeyCoordinates: Codable, Equatable, Sendable {
    public let x: String
    public let y: String
}

public struct P256SignatureValue: Codable, Equatable, Sendable {
    public let r: String
    public let s: String
}

public struct AppleIdentityDescription: Codable, Equatable, Sendable {
    public let tag: String
    public let backend: AppleIdentityBackend
    public let testOnly: Bool
    public let securityLevel: String
    public let keyID: String
    public let publicKey: P256PublicKeyCoordinates
}

public struct AppleIdentitySignature: Codable, Equatable, Sendable {
    public let identity: AppleIdentityDescription
    public let algorithm: String
    public let digest: String
    public let signature: P256SignatureValue
    public let lowS: Bool
}

public enum AppleIdentityError: Error, Equatable, Sendable {
    case invalidTag
    case invalidDigest
    case invalidSignatureEncoding
    case invalidScalar(String)
    case duplicateTag
    case identityNotFound
    case orphanedKey
    case corruptMetadata
    case secureEnclaveUnavailable
    case keychain(String, OSStatus)
    case cryptographicFailure(String)

    public var code: String {
        switch self {
        case .invalidTag: "invalid_tag"
        case .invalidDigest: "invalid_digest"
        case .invalidSignatureEncoding: "invalid_signature_encoding"
        case .invalidScalar: "invalid_signature_scalar"
        case .duplicateTag: "identity_exists"
        case .identityNotFound: "identity_not_found"
        case .orphanedKey: "orphaned_key"
        case .corruptMetadata: "corrupt_keychain_metadata"
        case .secureEnclaveUnavailable: "secure_enclave_unavailable"
        case .keychain: "keychain_failure"
        case .cryptographicFailure: "cryptographic_failure"
        }
    }

    public var safeMessage: String {
        switch self {
        case .invalidTag:
            "tag must be 1–128 ASCII letters, numbers, dots, colons, underscores, or hyphens"
        case .invalidDigest:
            "digest must be exactly 32 bytes of hexadecimal"
        case .invalidSignatureEncoding:
            "Apple returned a malformed P-256 signature"
        case let .invalidScalar(name):
            "P-256 signature scalar \(name) is outside the valid range"
        case .duplicateTag:
            "an Apple identity already exists for this tag"
        case .identityNotFound:
            "no Apple identity exists for this tag"
        case .orphanedKey:
            "a key exists without its TOHSENO metadata; refusing to replace it"
        case .corruptMetadata:
            "Apple identity metadata is invalid or no longer matches its key"
        case .secureEnclaveUnavailable:
            "Secure Enclave P-256 is unavailable; use software-test only for CI or testing"
        case let .keychain(operation, status):
            "Keychain \(operation) failed with OSStatus \(status)"
        case let .cryptographicFailure(operation):
            "P-256 \(operation) failed"
        }
    }
}

public final class AppleIdentityStore: @unchecked Sendable {
    public static let shared = AppleIdentityStore()

    private let metadataService = "org.tohseno.apple-identity.metadata.v1"
    private let metadataSchema = "tohseno.apple-identity.key-metadata/1"

    public init() {}

    public func create(
        tag: String,
        backend: AppleIdentityBackend = .secureEnclave
    ) throws -> AppleIdentityDescription {
        try Self.validate(tag: tag)
        if try metadataData(tag: tag) != nil {
            throw AppleIdentityError.duplicateTag
        }
        for candidate in [AppleIdentityBackend.secureEnclave, .softwareTest]
            where try keyExists(applicationTag: applicationTag(tag: tag, backend: candidate))
        {
            throw AppleIdentityError.orphanedKey
        }

        let keyTag = applicationTag(tag: tag, backend: backend)
        let privateKey = try generateKey(applicationTag: keyTag, backend: backend)
        do {
            let persistentReference = try persistentReference(applicationTag: keyTag)
            let metadata = KeyMetadata(
                schema: metadataSchema,
                tag: tag,
                backend: backend,
                applicationTag: keyTag.base64EncodedString(),
                persistentReference: persistentReference.base64EncodedString()
            )
            try addMetadata(metadata)
            return try description(tag: tag, backend: backend, privateKey: privateKey)
        } catch {
            SecItemDelete(keyQuery(applicationTag: keyTag) as CFDictionary)
            SecItemDelete(metadataQuery(tag: tag) as CFDictionary)
            throw error
        }
    }

    public func publicIdentity(tag: String) throws -> AppleIdentityDescription {
        let loaded = try load(tag: tag)
        return try description(tag: tag, backend: loaded.metadata.backend, privateKey: loaded.key)
    }

    public func sign(tag: String, digest: Data) throws -> AppleIdentitySignature {
        guard digest.count == 32 else {
            throw AppleIdentityError.invalidDigest
        }
        let loaded = try load(tag: tag)
        var error: Unmanaged<CFError>?
        guard let der = SecKeyCreateSignature(
            loaded.key,
            .ecdsaSignatureDigestX962SHA256,
            digest as CFData,
            &error
        ) as Data? else {
            throw AppleIdentityError.cryptographicFailure("signing")
        }
        let components = try ECDSASignatureCodec.fixedWidthComponents(fromDER: der)
        guard let publicKey = SecKeyCopyPublicKey(loaded.key) else {
            throw AppleIdentityError.cryptographicFailure("public-key derivation")
        }
        let normalizedDER = try ECDSASignatureCodec.derSignature(
            from: components
        )
        var verificationError: Unmanaged<CFError>?
        guard SecKeyVerifySignature(
            publicKey,
            .ecdsaSignatureDigestX962SHA256,
            digest as CFData,
            normalizedDER as CFData,
            &verificationError
        ) else {
            throw AppleIdentityError.cryptographicFailure(
                "normalized-signature verification"
            )
        }
        let identity = try description(
            tag: tag,
            backend: loaded.metadata.backend,
            privateKey: loaded.key
        )
        return AppleIdentitySignature(
            identity: identity,
            algorithm: "p256",
            digest: digest.hexadecimal(prefix: true),
            signature: P256SignatureValue(
                r: components.r.hexadecimal(prefix: true),
                s: components.s.hexadecimal(prefix: true)
            ),
            lowS: ECDSASignatureCodec.isLowS(components.s)
        )
    }

    @discardableResult
    public func delete(tag: String) throws -> AppleIdentityDescription {
        let loaded = try load(tag: tag)
        let existing = try description(
            tag: tag,
            backend: loaded.metadata.backend,
            privateKey: loaded.key
        )
        let keyStatus = SecItemDelete([
            kSecValuePersistentRef: loaded.persistentReference,
        ] as CFDictionary)
        guard keyStatus == errSecSuccess else {
            throw AppleIdentityError.keychain("key deletion", keyStatus)
        }
        let metadataStatus = SecItemDelete(metadataQuery(tag: tag) as CFDictionary)
        guard metadataStatus == errSecSuccess else {
            throw AppleIdentityError.keychain("metadata deletion", metadataStatus)
        }
        return existing
    }

    private func generateKey(
        applicationTag: Data,
        backend: AppleIdentityBackend
    ) throws -> SecKey {
        var privateAttributes: [CFString: Any] = [
            kSecAttrIsPermanent: true,
            kSecAttrApplicationTag: applicationTag,
            kSecAttrLabel: backend.testOnly
                ? "TOHSENO SOFTWARE TEST KEY — NOT PRODUCTION"
                : "TOHSENO Builder DeviceKey",
        ]
        var attributes: [CFString: Any] = [
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits: 256,
            kSecPrivateKeyAttrs: privateAttributes,
        ]

        if backend == .secureEnclave {
            var accessError: Unmanaged<CFError>?
            guard let access = SecAccessControlCreateWithFlags(
                nil,
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                .privateKeyUsage,
                &accessError
            ) else {
                throw AppleIdentityError.secureEnclaveUnavailable
            }
            privateAttributes[kSecAttrAccessControl] = access
            attributes[kSecPrivateKeyAttrs] = privateAttributes
            attributes[kSecAttrTokenID] = kSecAttrTokenIDSecureEnclave
        } else {
            privateAttributes[kSecAttrAccessible] =
                kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            attributes[kSecPrivateKeyAttrs] = privateAttributes
        }

        var error: Unmanaged<CFError>?
        guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else {
            if backend == .secureEnclave {
                throw AppleIdentityError.secureEnclaveUnavailable
            }
            throw AppleIdentityError.cryptographicFailure("key generation")
        }
        return key
    }

    private func description(
        tag: String,
        backend: AppleIdentityBackend,
        privateKey: SecKey
    ) throws -> AppleIdentityDescription {
        guard let publicKey = SecKeyCopyPublicKey(privateKey) else {
            throw AppleIdentityError.cryptographicFailure("public-key derivation")
        }
        var error: Unmanaged<CFError>?
        guard let representation = SecKeyCopyExternalRepresentation(publicKey, &error) as Data?,
              representation.count == 65,
              representation.first == 0x04
        else {
            throw AppleIdentityError.cryptographicFailure("public-key encoding")
        }
        let x = representation.subdata(in: 1 ..< 33)
        let y = representation.subdata(in: 33 ..< 65)
        let fingerprint = Data(SHA256.hash(data: representation)).hexadecimal(prefix: false)
        return AppleIdentityDescription(
            tag: tag,
            backend: backend,
            testOnly: backend.testOnly,
            securityLevel: backend.securityLevel,
            keyID: "sha256:\(fingerprint)",
            publicKey: P256PublicKeyCoordinates(
                x: x.hexadecimal(prefix: true),
                y: y.hexadecimal(prefix: true)
            )
        )
    }

    private func load(tag: String) throws -> LoadedKey {
        try Self.validate(tag: tag)
        guard let data = try metadataData(tag: tag) else {
            throw AppleIdentityError.identityNotFound
        }
        guard let metadata = try? JSONDecoder().decode(KeyMetadata.self, from: data),
              metadata.schema == metadataSchema,
              metadata.tag == tag,
              let recordedTag = Data(base64Encoded: metadata.applicationTag),
              recordedTag == applicationTag(tag: tag, backend: metadata.backend),
              let persistentReference = Data(base64Encoded: metadata.persistentReference)
        else {
            throw AppleIdentityError.corruptMetadata
        }
        var item: CFTypeRef?
        let status = SecItemCopyMatching([
            kSecValuePersistentRef: persistentReference,
            kSecReturnRef: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ] as CFDictionary, &item)
        guard status == errSecSuccess, let item else {
            if status == errSecItemNotFound {
                throw AppleIdentityError.corruptMetadata
            }
            throw AppleIdentityError.keychain("key lookup", status)
        }
        guard CFGetTypeID(item) == SecKeyGetTypeID() else {
            throw AppleIdentityError.corruptMetadata
        }
        let key = item as! SecKey
        return LoadedKey(
            metadata: metadata,
            persistentReference: persistentReference,
            key: key
        )
    }

    private func persistentReference(applicationTag: Data) throws -> Data {
        var item: CFTypeRef?
        var query = keyQuery(applicationTag: applicationTag)
        query[kSecReturnPersistentRef] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess, let reference = item as? Data else {
            throw AppleIdentityError.keychain("persistent-reference lookup", status)
        }
        return reference
    }

    private func keyExists(applicationTag: Data) throws -> Bool {
        var query = keyQuery(applicationTag: applicationTag)
        query[kSecReturnAttributes] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        if status == errSecItemNotFound {
            return false
        }
        guard status == errSecSuccess else {
            throw AppleIdentityError.keychain("duplicate check", status)
        }
        return true
    }

    private func keyQuery(applicationTag: Data) -> [CFString: Any] {
        [
            kSecClass: kSecClassKey,
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeyClass: kSecAttrKeyClassPrivate,
            kSecAttrApplicationTag: applicationTag,
        ]
    }

    private func metadataQuery(tag: String) -> [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: metadataService,
            kSecAttrAccount: tag,
            kSecAttrSynchronizable: kCFBooleanFalse as Any,
        ]
    }

    private func metadataData(tag: String) throws -> Data? {
        var query = metadataQuery(tag: tag)
        query[kSecReturnData] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess, let data = item as? Data else {
            throw AppleIdentityError.keychain("metadata lookup", status)
        }
        return data
    }

    private func addMetadata(_ metadata: KeyMetadata) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(metadata)
        var query = metadataQuery(tag: metadata.tag)
        query[kSecValueData] = data
        query[kSecAttrAccessible] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw AppleIdentityError.keychain("metadata creation", status)
        }
    }

    private func applicationTag(tag: String, backend: AppleIdentityBackend) -> Data {
        let digest = Data(SHA256.hash(data: Data(tag.utf8))).hexadecimal(prefix: false)
        return Data(
            "org.tohseno.apple-identity.key.v1.\(backend.rawValue).\(digest)".utf8
        )
    }

    public static func validate(tag: String) throws {
        guard (1 ... 128).contains(tag.utf8.count),
              tag.unicodeScalars.allSatisfy({
                  switch $0.value {
                  case 45, 46, 48 ... 58, 65 ... 90, 95, 97 ... 122:
                      true
                  default:
                      false
                  }
              })
        else {
            throw AppleIdentityError.invalidTag
        }
    }
}

private struct KeyMetadata: Codable {
    let schema: String
    let tag: String
    let backend: AppleIdentityBackend
    let applicationTag: String
    let persistentReference: String
}

private struct LoadedKey {
    let metadata: KeyMetadata
    let persistentReference: Data
    let key: SecKey
}

public extension Data {
    init?(strictHexadecimal value: String, expectedBytes: Int? = nil) {
        let text = value.hasPrefix("0x") ? String(value.dropFirst(2)) : value
        guard text.count % 2 == 0,
              text.unicodeScalars.allSatisfy({
                  ("0" ... "9").contains(Character($0))
                      || ("a" ... "f").contains(Character($0))
                      || ("A" ... "F").contains(Character($0))
              })
        else {
            return nil
        }
        var bytes: [UInt8] = []
        bytes.reserveCapacity(text.count / 2)
        var index = text.startIndex
        while index < text.endIndex {
            let next = text.index(index, offsetBy: 2)
            guard let byte = UInt8(text[index ..< next], radix: 16) else {
                return nil
            }
            bytes.append(byte)
            index = next
        }
        if let expectedBytes, bytes.count != expectedBytes {
            return nil
        }
        self = Data(bytes)
    }

    func hexadecimal(prefix: Bool) -> String {
        let encoded = map { String(format: "%02x", $0) }.joined()
        return prefix ? "0x\(encoded)" : encoded
    }
}
