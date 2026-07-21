import CommonCrypto
import CryptoKit
import Foundation

/// The identity spine: a BIP39 seed phrase silently generated on first
/// launch, a keypair derived from it, and a short fingerprint that serves
/// as the app's user ID.
///
/// First launch = identity exists. No account, no login, no email.
/// Recovery is the phrase; restoring a phrase replaces the local identity.
struct Identity: Equatable {
    let mnemonic: [String]
    let signingKey: Curve25519.Signing.PrivateKey

    /// The app's user ID: a hex fingerprint of the public key.
    var fingerprint: String {
        let digest = SHA256.hash(data: signingKey.publicKey.rawRepresentation)
        return digest.map { String(format: "%02x", $0) }.prefix(4).joined()
            + "…"
            + digest.map { String(format: "%02x", $0) }.suffix(4).joined()
    }

    /// The full stable identifier (64 hex characters) for programmatic use.
    var userID: String {
        SHA256.hash(data: signingKey.publicKey.rawRepresentation)
            .map { String(format: "%02x", $0) }
            .joined()
    }

    static func == (left: Identity, right: Identity) -> Bool {
        left.mnemonic == right.mnemonic
    }

    /// Derives the identity deterministically from a mnemonic using the
    /// standard BIP39 seed derivation (PBKDF2-HMAC-SHA512, 2048 rounds,
    /// salt "mnemonic"), taking the first 32 bytes as the Curve25519 key.
    static func from(mnemonic: [String]) throws -> Identity {
        try BIP39.entropy(fromMnemonic: mnemonic)
        let seed = pbkdf2SHA512(
            password: mnemonic.joined(separator: " "),
            salt: "mnemonic",
            rounds: 2048,
            length: 64
        )
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: seed.prefix(32))
        return Identity(mnemonic: mnemonic, signingKey: key)
    }

    private static func pbkdf2SHA512(password: String, salt: String, rounds: UInt32, length: Int) -> Data {
        let passwordBytes = Array(password.decomposedStringWithCompatibilityMapping.utf8)
        let saltBytes = Array(salt.decomposedStringWithCompatibilityMapping.utf8)
        var derived = [UInt8](repeating: 0, count: length)
        passwordBytes.withUnsafeBufferPointer { passwordBuffer in
            _ = CCKeyDerivationPBKDF(
                CCPBKDFAlgorithm(kCCPBKDF2),
                passwordBuffer.baseAddress.map { UnsafeRawPointer($0).assumingMemoryBound(to: Int8.self) },
                passwordBytes.count,
                saltBytes,
                saltBytes.count,
                CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA512),
                rounds,
                &derived,
                length
            )
        }
        return Data(derived)
    }
}

/// Where the mnemonic lives. The app uses the keychain; tests use memory.
protocol SecretStore {
    func readMnemonic() -> [String]?
    func writeMnemonic(_ words: [String]) throws
}

/// Loads the identity, silently creating one on first launch.
final class IdentityManager: ObservableObject {
    @Published private(set) var identity: Identity?

    private let store: SecretStore

    init(store: SecretStore) {
        self.store = store
    }

    /// Idempotent: returns the existing identity or silently creates one.
    /// Never shows UI and never blocks the writing surface.
    func loadOrCreate() {
        if let words = store.readMnemonic(), let existing = try? Identity.from(mnemonic: words) {
            identity = existing
            return
        }
        guard let words = try? BIP39.generateMnemonic(),
              let created = try? Identity.from(mnemonic: words) else { return }
        try? store.writeMnemonic(words)
        identity = created
    }

    /// Replaces the local identity with one restored from a phrase.
    /// Session content on this device is untouched; only identity changes.
    func restore(phrase: String) throws {
        let words = BIP39.normalize(phrase)
        let restored = try Identity.from(mnemonic: words)
        try store.writeMnemonic(words)
        identity = restored
    }
}

/// Keychain-backed secret store. Device-only, available after first unlock,
/// never synchronized to iCloud.
struct KeychainSecretStore: SecretStore {
    enum KeychainError: Error {
        case writeFailed(OSStatus)
    }

    private var service: String {
        (Bundle.main.bundleIdentifier ?? "com.tohseno.base-writing") + ".identity"
    }
    private let account = "seed-phrase"

    func readMnemonic() -> [String]? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
        ]
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data,
              let phrase = String(data: data, encoding: .utf8) else { return nil }
        return BIP39.normalize(phrase)
    }

    func writeMnemonic(_ words: [String]) throws {
        let data = Data(words.joined(separator: " ").utf8)
        let base: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(base as CFDictionary)
        var attributes = base
        attributes[kSecValueData as String] = data
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else { throw KeychainError.writeFailed(status) }
    }
}

/// In-memory store for tests and previews.
final class InMemorySecretStore: SecretStore {
    private var words: [String]?

    func readMnemonic() -> [String]? { words }
    func writeMnemonic(_ words: [String]) throws { self.words = words }
}
