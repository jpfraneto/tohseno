import Foundation
import Security
import Testing
@testable import TohsenoAppleIdentity

private let identityStoreTestLock = NSLock()

private final class IsolatedAppleIdentityStore {
    let store: AppleIdentityStore
    let root: URL
    let path: String
    private let keychain: SecKeychain

    init() throws {
        root = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-apple-identity-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(
            at: root,
            withIntermediateDirectories: false,
            attributes: [.posixPermissions: 0o700]
        )
        let keychainPath = root.appendingPathComponent("verification.keychain-db").path
        path = keychainPath
        let password = Data()
        var created: SecKeychain?
        let status = password.withUnsafeBytes { bytes in
            SecKeychainCreate(
                keychainPath,
                UInt32(bytes.count),
                bytes.baseAddress,
                false,
                nil,
                &created
            )
        }
        guard status == errSecSuccess, let created else {
            try? FileManager.default.removeItem(at: root)
            throw AppleIdentityError.keychain("test Keychain creation", status)
        }
        keychain = created
        let unlockStatus = password.withUnsafeBytes { bytes in
            SecKeychainUnlock(created, UInt32(bytes.count), bytes.baseAddress, false)
        }
        guard unlockStatus == errSecSuccess else {
            SecKeychainDelete(created)
            try? FileManager.default.removeItem(at: root)
            throw AppleIdentityError.keychain("test Keychain unlock", unlockStatus)
        }
        store = AppleIdentityStore(keychain: created)
    }

    deinit {
        SecKeychainDelete(keychain)
        try? FileManager.default.removeItem(at: root)
    }
}

@Test
func verificationConfigurationScopesKeysAndRefusesSecureEnclave() throws {
    identityStoreTestLock.lock()
    defer { identityStoreTestLock.unlock() }
    let isolated = try IsolatedAppleIdentityStore()
    let store = try AppleIdentityStore.configured(environment: [
        "TOHSENO_VERIFICATION_MODE": "1",
        "TOHSENO_VERIFICATION_KEYCHAIN_PATH": isolated.path,
    ])
    let tag = "org.tohseno.test.\(UUID().uuidString)"
    defer { _ = try? store.delete(tag: tag) }

    let created = try store.create(tag: tag, backend: .softwareTest)
    #expect(created.testOnly)
    #expect(try isolated.store.publicIdentity(tag: tag) == created)
    #expect(throws: AppleIdentityError.verificationModeRequiresSoftwareTest) {
        try store.create(tag: "\(tag).secure", backend: .secureEnclave)
    }

    let symlink = isolated.root.appendingPathComponent("verification-link.keychain-db")
    try FileManager.default.createSymbolicLink(
        at: symlink,
        withDestinationURL: URL(fileURLWithPath: isolated.path)
    )
    #expect(throws: AppleIdentityError.keychain(
        "verification Keychain inspection",
        errSecParam
    )) {
        try AppleIdentityStore.configured(environment: [
            "TOHSENO_VERIFICATION_MODE": "1",
            "TOHSENO_VERIFICATION_KEYCHAIN_PATH": symlink.path,
        ])
    }
}

@Test
func softwareTestBackendPersistsSignsAndDeletesWithoutPrivateExport() throws {
    identityStoreTestLock.lock()
    defer { identityStoreTestLock.unlock() }
    let isolated = try IsolatedAppleIdentityStore()
    let store = isolated.store
    let tag = "org.tohseno.test.\(UUID().uuidString)"
    defer { _ = try? store.delete(tag: tag) }

    let created = try store.create(tag: tag, backend: .softwareTest)
    #expect(created.tag == tag)
    #expect(created.backend == .softwareTest)
    #expect(created.testOnly)
    #expect(created.securityLevel == "software_test")
    #expect(created.keyID.hasPrefix("sha256:"))
    #expect(created.publicKey.x.count == 66)
    #expect(created.publicKey.y.count == 66)

    let loaded = try store.publicIdentity(tag: tag)
    #expect(loaded == created)

    let digest = Data(repeating: 0x42, count: 32)
    let signed = try store.sign(tag: tag, digest: digest)
    #expect(signed.identity == created)
    #expect(signed.algorithm == "p256")
    #expect(signed.digest == "0x" + String(repeating: "42", count: 32))
    #expect(signed.lowS)

    let x = try #require(Data(strictHexadecimal: created.publicKey.x, expectedBytes: 32))
    let y = try #require(Data(strictHexadecimal: created.publicKey.y, expectedBytes: 32))
    let r = try #require(Data(strictHexadecimal: signed.signature.r, expectedBytes: 32))
    let s = try #require(Data(strictHexadecimal: signed.signature.s, expectedBytes: 32))
    let publicBytes = Data([0x04]) + x + y
    var error: Unmanaged<CFError>?
    let publicKey = try #require(SecKeyCreateWithData(
        publicBytes as CFData,
        [
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeyClass: kSecAttrKeyClassPublic,
            kSecAttrKeySizeInBits: 256,
        ] as CFDictionary,
        &error
    ))
    let der = try ECDSASignatureCodec.derSignature(
        from: P256SignatureComponents(r: r, s: s)
    )
    let verified = SecKeyVerifySignature(
        publicKey,
        .ecdsaSignatureDigestX962SHA256,
        digest as CFData,
        der as CFData,
        &error
    )
    #expect(verified)

    // Exercise randomized ECDSA output repeatedly so both native high-s and
    // native low-s signatures pass through the normalization boundary.
    for byte in UInt8(0) ..< UInt8(24) {
        let trialDigest = Data(repeating: byte, count: 32)
        let trial = try store.sign(tag: tag, digest: trialDigest)
        let trialR = try #require(Data(
            strictHexadecimal: trial.signature.r,
            expectedBytes: 32
        ))
        let trialS = try #require(Data(
            strictHexadecimal: trial.signature.s,
            expectedBytes: 32
        ))
        let trialDER = try ECDSASignatureCodec.derSignature(
            from: P256SignatureComponents(r: trialR, s: trialS)
        )
        #expect(ECDSASignatureCodec.isLowS(trialS))
        #expect(SecKeyVerifySignature(
            publicKey,
            .ecdsaSignatureDigestX962SHA256,
            trialDigest as CFData,
            trialDER as CFData,
            nil
        ))
    }

    let deleted = try store.delete(tag: tag)
    #expect(deleted == created)
    #expect(throws: AppleIdentityError.identityNotFound) {
        try store.publicIdentity(tag: tag)
    }
}

@Test
func refusesDuplicateTagsAcrossBackends() throws {
    identityStoreTestLock.lock()
    defer { identityStoreTestLock.unlock() }
    let isolated = try IsolatedAppleIdentityStore()
    let store = isolated.store
    let tag = "org.tohseno.test.\(UUID().uuidString)"
    defer { _ = try? store.delete(tag: tag) }
    _ = try store.create(tag: tag, backend: .softwareTest)
    #expect(throws: AppleIdentityError.duplicateTag) {
        try store.create(tag: tag, backend: .secureEnclave)
    }
}

@Test
func parsesTheExactCommandSurface() throws {
    #expect(try AppleIdentityCommand.parse([
        "create", "--tag", "org.tohseno.test", "--backend", "software-test",
    ]) == .create(tag: "org.tohseno.test", backend: .softwareTest))
    #expect(try AppleIdentityCommand.parse([
        "public", "--tag", "org.tohseno.test",
    ]) == .showPublic(tag: "org.tohseno.test"))
    #expect(throws: AppleIdentityError.invalidDigest) {
        try AppleIdentityCommand.parse([
            "sign", "--tag", "org.tohseno.test", "--digest", "abcd",
        ])
    }
    #expect(throws: AppleIdentityError.invalidTag) {
        try AppleIdentityCommand.parse([
            "public", "--tag", "org.tohseno.tést",
        ])
    }
}
