import CryptoKit
import Foundation
import Security

public struct CompanionIdentityDescription: Codable, Equatable, Sendable {
    public let schema: String
    public let deviceID: String
    public let signingPublicKey: String
    public let agreementPublicKey: String

    public init(
        schema: String = "tohseno.companion-identity/1",
        deviceID: String,
        signingPublicKey: String,
        agreementPublicKey: String
    ) {
        self.schema = schema
        self.deviceID = deviceID
        self.signingPublicKey = signingPublicKey
        self.agreementPublicKey = agreementPublicKey
    }
}

public protocol CompanionSecretStore: Sendable {
    func loadIdentityEntropy() async throws -> Data?
    func saveIdentityEntropy(_ entropy: Data) async throws
    func deleteIdentityEntropy() async throws
}

public actor KeychainCompanionSecretStore: CompanionSecretStore {
    private let service: String
    private let account: String

    public init(
        service: String = "org.tohseno.companion.identity.v1",
        account: String = "primary"
    ) {
        self.service = service
        self.account = account
    }

    public func loadIdentityEntropy() throws -> Data? {
        var item: CFTypeRef?
        let status = SecItemCopyMatching([
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrSynchronizable: kCFBooleanFalse as Any,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ] as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = item as? Data, data.count == 16 else {
            throw TohsenoCompanionError.unsafeStorage
        }
        return data
    }

    public func saveIdentityEntropy(_ entropy: Data) throws {
        guard entropy.count == 16 else { throw TohsenoCompanionError.unsafeStorage }
        let base: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrSynchronizable: kCFBooleanFalse as Any,
        ]
        let add = base.merging([
            kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAttrLabel: "TOHSENO Companion recovery entropy",
            kSecValueData: entropy,
        ]) { _, replacement in replacement }
        let status = SecItemAdd(add as CFDictionary, nil)
        if status == errSecDuplicateItem {
            let update = SecItemUpdate(
                base as CFDictionary,
                [kSecValueData: entropy] as CFDictionary
            )
            guard update == errSecSuccess else { throw TohsenoCompanionError.unsafeStorage }
        } else if status != errSecSuccess {
            throw TohsenoCompanionError.unsafeStorage
        }
    }

    public func deleteIdentityEntropy() throws {
        let status = SecItemDelete([
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrSynchronizable: kCFBooleanFalse as Any,
        ] as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw TohsenoCompanionError.unsafeStorage
        }
    }
}

public actor InMemoryCompanionSecretStore: CompanionSecretStore {
    private var entropy: Data?

    public init(entropy: Data? = nil) {
        self.entropy = entropy
    }

    public func loadIdentityEntropy() -> Data? { entropy }
    public func saveIdentityEntropy(_ entropy: Data) { self.entropy = entropy }
    public func deleteIdentityEntropy() { entropy = nil }
}

public actor CompanionIdentityManager {
    private let store: any CompanionSecretStore
    private let entropySource: any CompanionEntropySource

    public init(
        store: any CompanionSecretStore = KeychainCompanionSecretStore(),
        entropySource: any CompanionEntropySource = SystemCompanionEntropySource()
    ) {
        self.store = store
        self.entropySource = entropySource
    }

    public func createIdentity() async throws -> RecoveryPhrase {
        guard try await store.loadIdentityEntropy() == nil else {
            throw TohsenoCompanionError.identityAlreadyExists
        }
        let phrase = try RecoveryPhrase(entropy: entropySource.randomBytes(count: 16))
        try await store.saveIdentityEntropy(phrase.rawEntropy)
        return phrase
    }

    public func restoreIdentity(from phrase: RecoveryPhrase) async throws {
        try await store.saveIdentityEntropy(phrase.rawEntropy)
    }

    public func deleteIdentity() async throws {
        try await store.deleteIdentityEntropy()
    }

    public func publicIdentity() async throws -> CompanionIdentityDescription {
        try await identity().description
    }

    func identity() async throws -> CompanionIdentity {
        guard let entropy = try await store.loadIdentityEntropy() else {
            throw TohsenoCompanionError.identityMissing
        }
        return try CompanionIdentity(phrase: RecoveryPhrase(entropy: entropy))
    }
}

struct CompanionIdentity: Sendable {
    let signingKey: Curve25519.Signing.PrivateKey
    let agreementKey: Curve25519.KeyAgreement.PrivateKey
    let storageKey: SymmetricKey
    let description: CompanionIdentityDescription

    init(phrase: RecoveryPhrase) throws {
        let derived = try CompanionKeyDerivation.derive(phrase: phrase)
        signingKey = try Curve25519.Signing.PrivateKey(rawRepresentation: derived.signingPrivateKey)
        agreementKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: derived.agreementPrivateKey)
        storageKey = SymmetricKey(data: derived.storageKey)
        let signingPublic = signingKey.publicKey.rawRepresentation
        let agreementPublic = agreementKey.publicKey.rawRepresentation
        var identifierInput = Data("tohseno.companion.device-id.v1\0".utf8)
        identifierInput.append(signingPublic)
        identifierInput.append(agreementPublic)
        let deviceDigest = identifierInput.companionSHA256
        description = CompanionIdentityDescription(
            deviceID: "device_\(Base64URL.encode(deviceDigest.prefix(18)))",
            signingPublicKey: Base64URL.encode(signingPublic),
            agreementPublicKey: Base64URL.encode(agreementPublic)
        )
    }

    init(signingPrivateKey: Data, agreementPrivateKey: Data, storageKey: Data = Data(repeating: 0, count: 32)) throws {
        signingKey = try Curve25519.Signing.PrivateKey(rawRepresentation: signingPrivateKey)
        agreementKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: agreementPrivateKey)
        self.storageKey = SymmetricKey(data: storageKey)
        description = Self.describe(
            signingPublicKey: signingKey.publicKey.rawRepresentation,
            agreementPublicKey: agreementKey.publicKey.rawRepresentation
        )
    }

    func sign(domain: String, message: Data) throws -> Data {
        var signingBytes = Data(domain.utf8)
        signingBytes.append(0)
        signingBytes.append(message)
        return try signingKey.signature(for: signingBytes)
    }

    static func verify(
        publicKey: Data,
        domain: String,
        message: Data,
        signature: Data
    ) throws -> Bool {
        guard publicKey.count == 32, signature.count == 64 else { return false }
        var signingBytes = Data(domain.utf8)
        signingBytes.append(0)
        signingBytes.append(message)
        return try Curve25519.Signing.PublicKey(rawRepresentation: publicKey)
            .isValidSignature(signature, for: signingBytes)
    }

    static func deviceID(signingPublicKey: Data, agreementPublicKey: Data) -> String {
        describe(signingPublicKey: signingPublicKey, agreementPublicKey: agreementPublicKey).deviceID
    }

    private static func describe(
        signingPublicKey: Data,
        agreementPublicKey: Data
    ) -> CompanionIdentityDescription {
        var identifierInput = Data("tohseno.companion.device-id.v1\0".utf8)
        identifierInput.append(signingPublicKey)
        identifierInput.append(agreementPublicKey)
        return CompanionIdentityDescription(
            deviceID: "device_\(Base64URL.encode(identifierInput.companionSHA256.prefix(18)))",
            signingPublicKey: Base64URL.encode(signingPublicKey),
            agreementPublicKey: Base64URL.encode(agreementPublicKey)
        )
    }
}

struct CompanionDerivedKeys: Equatable {
    let signingPrivateKey: Data
    let agreementPrivateKey: Data
    let storageKey: Data
}

enum CompanionKeyDerivation {
    static let signingDomain = "tohseno.companion.signing.v1"
    static let agreementDomain = "tohseno.companion.agreement.v1"
    static let storageDomain = "tohseno.companion.storage.v1"

    static func derive(phrase: RecoveryPhrase) throws -> CompanionDerivedKeys {
        let seed = phrase.seed()
        return CompanionDerivedKeys(
            signingPrivateKey: derive(seed: seed, domain: signingDomain),
            agreementPrivateKey: derive(seed: seed, domain: agreementDomain),
            storageKey: derive(seed: seed, domain: storageDomain)
        )
    }

    private static func derive(seed: Data, domain: String) -> Data {
        let key = HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: seed),
            salt: Data("tohseno.companion.hkdf-sha256.v1".utf8),
            info: Data(domain.utf8),
            outputByteCount: 32
        )
        return key.withUnsafeBytes { Data($0) }
    }
}
