import Foundation
import Security
import Testing
@testable import TohsenoAppleIdentity

private let identityStoreTestLock = NSLock()

@Test
func softwareTestBackendPersistsSignsAndDeletesWithoutPrivateExport() throws {
    identityStoreTestLock.lock()
    defer { identityStoreTestLock.unlock() }
    let store = AppleIdentityStore()
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
    let store = AppleIdentityStore()
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
