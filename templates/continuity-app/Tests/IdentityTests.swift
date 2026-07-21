import XCTest
@testable import Writing

/// Invariants for the identity spine: silent creation on first launch,
/// deterministic derivation, and restore-replaces-identity.
final class IdentityTests: XCTestCase {
    private let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow"

    func testFirstLoadSilentlyCreatesAPersistentIdentity() {
        let store = InMemorySecretStore()
        let manager = IdentityManager(store: store)
        XCTAssertNil(manager.identity)

        manager.loadOrCreate()
        let created = manager.identity
        XCTAssertNotNil(created, "first launch must create an identity with no interaction")
        XCTAssertNotNil(store.readMnemonic(), "the phrase must persist in the secret store")

        // A second load returns the same identity, not a new one.
        let again = IdentityManager(store: store)
        again.loadOrCreate()
        XCTAssertEqual(again.identity, created)
    }

    func testDerivationIsDeterministic() throws {
        let words = BIP39.normalize(phrase)
        let first = try Identity.from(mnemonic: words)
        let second = try Identity.from(mnemonic: words)
        XCTAssertEqual(first.userID, second.userID)
        XCTAssertEqual(first.userID.count, 64)
        XCTAssertTrue(first.fingerprint.contains("…"))
    }

    func testRestoreReplacesLocalIdentity() throws {
        let store = InMemorySecretStore()
        let manager = IdentityManager(store: store)
        manager.loadOrCreate()
        let original = manager.identity?.userID

        try manager.restore(phrase: phrase)
        XCTAssertNotEqual(manager.identity?.userID, original)
        XCTAssertEqual(store.readMnemonic(), BIP39.normalize(phrase))
    }

    func testRestoreRejectsInvalidPhrase() {
        let manager = IdentityManager(store: InMemorySecretStore())
        manager.loadOrCreate()
        let before = manager.identity
        XCTAssertThrowsError(try manager.restore(phrase: "totally not a seed phrase"))
        XCTAssertEqual(manager.identity, before, "a failed restore must not touch the current identity")
    }

    func testSigningKeyActuallySigns() throws {
        let identity = try Identity.from(mnemonic: BIP39.normalize(phrase))
        let message = Data("session-challenge".utf8)
        let signature = try identity.signingKey.signature(for: message)
        XCTAssertTrue(identity.signingKey.publicKey.isValidSignature(signature, for: message))
    }
}
