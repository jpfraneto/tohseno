import XCTest
@testable import Writing

/// Fake keychain: one slot per synchronizable flag, mirroring how the real
/// keychain treats the two variants as distinct items.
private final class FakeSeedPhraseKeychain: SeedPhraseKeychain {
    var items: [Bool: Data] = [:]

    func read(synchronizable: Bool) -> Data? { items[synchronizable] }
    func write(_ data: Data, synchronizable: Bool) throws { items[synchronizable] = data }
    func delete(synchronizable: Bool) { items[synchronizable] = nil }
}

/// Invariants for identity keychain ordering: an identity already in iCloud
/// Keychain is adopted, never shadowed by a fresh one; a legacy device-only
/// item migrates to synchronizable; restore overwrites the synced item.
final class KeychainSecretStoreTests: XCTestCase {
    private let phrase = "legal winner thank year wave sausage worth useful legal winner thank yellow"
    private let otherPhrase = "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"

    private func data(_ phrase: String) -> Data {
        Data(phrase.utf8)
    }

    func testSyncedIdentityIsAdoptedInsteadOfGeneratingANewOne() {
        let keychain = FakeSeedPhraseKeychain()
        keychain.items[true] = data(phrase)
        let manager = IdentityManager(store: KeychainSecretStore(keychain: keychain))

        manager.loadOrCreate()

        XCTAssertEqual(
            manager.identity?.mnemonic,
            BIP39.normalize(phrase),
            "an identity synced from another device must be adopted, never regenerated"
        )
        XCTAssertEqual(keychain.items[true], data(phrase), "adoption must not rewrite the synced item")
    }

    func testFirstLaunchWritesASynchronizableItem() {
        let keychain = FakeSeedPhraseKeychain()
        let manager = IdentityManager(store: KeychainSecretStore(keychain: keychain))

        manager.loadOrCreate()

        XCTAssertNotNil(manager.identity)
        XCTAssertNotNil(keychain.items[true], "a new identity must land in the synchronizable slot")
        XCTAssertNil(keychain.items[false], "nothing may be written device-only")
    }

    func testLegacyDeviceOnlyItemMigratesToSynchronizable() {
        let keychain = FakeSeedPhraseKeychain()
        keychain.items[false] = data(phrase)
        let store = KeychainSecretStore(keychain: keychain)

        XCTAssertEqual(store.readMnemonic(), BIP39.normalize(phrase))
        XCTAssertEqual(keychain.items[true], data(phrase), "the legacy item must be rewritten as synchronizable")
        XCTAssertNil(keychain.items[false], "the legacy device-only item must be deleted after migration")

        let manager = IdentityManager(store: store)
        manager.loadOrCreate()
        XCTAssertEqual(manager.identity?.mnemonic, BIP39.normalize(phrase), "migration must preserve the identity")
    }

    func testRestoreOverwritesTheSynchronizableItem() throws {
        let keychain = FakeSeedPhraseKeychain()
        keychain.items[true] = data(phrase)
        let manager = IdentityManager(store: KeychainSecretStore(keychain: keychain))
        manager.loadOrCreate()

        try manager.restore(phrase: otherPhrase)

        XCTAssertEqual(
            keychain.items[true],
            data(BIP39.normalize(otherPhrase).joined(separator: " ")),
            "restore must overwrite the synced item so all devices converge"
        )
        XCTAssertNil(keychain.items[false])
    }
}
