import CryptoKit
import XCTest
@testable import TohsenoCompanionKit

final class CryptoTests: XCTestCase {
    func testOfficialTwelveWordBIP39VectorAndRestoration() throws {
        let entropy = Data(repeating: 0, count: 16)
        let phrase = try RecoveryPhrase(entropy: entropy)
        XCTAssertEqual(
            phrase.reveal(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        )
        let restored = try RecoveryPhrase(phrase.reveal())
        XCTAssertEqual(restored, phrase)
        XCTAssertEqual(
            phrase.seed(passphrase: "TREZOR").map { String(format: "%02x", $0) }.joined(),
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e5349553"
                + "1f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"
        )
        XCTAssertFalse(String(describing: phrase).contains("abandon"))
    }

    func testDomainSeparatedIdentityIsDeterministicAndRecoveryDoesNotRestoreCapabilities() async throws {
        let phrase = try RecoveryPhrase(entropy: Data(repeating: 7, count: 16))
        let first = try CompanionIdentity(phrase: phrase)
        let second = try CompanionIdentity(phrase: try RecoveryPhrase(phrase.reveal()))
        XCTAssertEqual(first.description, second.description)
        XCTAssertNotEqual(
            first.signingKey.rawRepresentation,
            first.agreementKey.rawRepresentation,
            "signing and agreement derivation must remain domain-separated"
        )

        let stateStore = InMemoryCompanionStateStore(bytes: Data("old capability".utf8))
        let secretStore = InMemoryCompanionSecretStore()
        let allowlist = try RelayAllowlist([
            RelayEndpoint(
                id: "official-v1",
                baseURL: URL(string: "http://127.0.0.1:3100")!,
                allowLoopbackHTTP: true
            ),
        ])
        let client = TohsenoCompanionClient(
            identityStore: secretStore,
            stateStore: stateStore,
            payloadStore: InMemoryCompanionPayloadStore(),
            relay: NoopRelay(),
            relayAllowlist: allowlist
        )
        try await client.restoreIdentity(from: phrase)
        let restoredState = await stateStore.load()
        XCTAssertNil(restoredState)
    }

    func testDeterministicEnvelopeRoundTripTamperAndReplay() async throws {
        let sender = try CompanionIdentity(phrase: RecoveryPhrase(entropy: Data(repeating: 30, count: 16)))
        let recipient = try CompanionIdentity(phrase: RecoveryPhrase(entropy: Data(repeating: 31, count: 16)))
        let envelope = try CompanionEnvelopeCrypto.seal(
            sender: sender,
            recipientAgreementPublicKey: recipient.agreementKey.publicKey.rawRepresentation,
            metadata: CompanionEnvelopeMetadata(
                envelopeID: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
                mailboxID: "mailbox_fixture",
                recipientDeviceID: recipient.description.deviceID,
                senderSequence: 42,
                createdAt: "2026-08-15T12:00:00Z",
                expiresAt: "2026-08-16T12:00:00Z"
            ),
            plaintext: Data("private".utf8),
            ephemeralSecret: Data(repeating: 31, count: 32),
            nonce: Data(repeating: 32, count: 12)
        )
        let replay = try CompanionReplayProtection(capacity: 128)
        let opened = try await CompanionEnvelopeCrypto.open(
            envelope,
            expectedSenderSigningPublicKey: sender.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: sender.description.deviceID,
            recipient: recipient,
            now: try CompanionTimestamp.parse("2026-08-15T12:01:00Z"),
            replay: replay
        )
        XCTAssertEqual(opened, Data("private".utf8))
        await XCTAssertThrowsErrorAsync {
            _ = try await CompanionEnvelopeCrypto.open(
                envelope,
                expectedSenderSigningPublicKey: sender.signingKey.publicKey.rawRepresentation,
                expectedSenderDeviceID: sender.description.deviceID,
                recipient: recipient,
                now: try CompanionTimestamp.parse("2026-08-15T12:01:00Z"),
                replay: replay
            )
        }
        let tampered = OpaqueCompanionEnvelope(
            envelopeID: envelope.envelopeID,
            mailboxID: envelope.mailboxID,
            senderDeviceID: envelope.senderDeviceID,
            recipientDeviceID: envelope.recipientDeviceID,
            senderSequence: envelope.senderSequence,
            createdAt: envelope.createdAt,
            expiresAt: envelope.expiresAt,
            ephemeralPublicKey: envelope.ephemeralPublicKey,
            nonce: envelope.nonce,
            ciphertext: "A" + envelope.ciphertext.dropFirst(),
            signature: envelope.signature
        )
        await XCTAssertThrowsErrorAsync {
            _ = try await CompanionEnvelopeCrypto.open(
                tampered,
                expectedSenderSigningPublicKey: sender.signingKey.publicKey.rawRepresentation,
                expectedSenderDeviceID: sender.description.deviceID,
                recipient: recipient,
                now: try CompanionTimestamp.parse("2026-08-15T12:01:00Z"),
                replay: try CompanionReplayProtection(capacity: 128)
            )
        }
    }

    func testEncryptedStateRejectsWrongKeyAndSymlink() async throws {
        let state = CompanionPersistentState()
        let key = SymmetricKey(data: Data(repeating: 1, count: 32))
        let bytes = try CompanionStateCodec.seal(state, key: key)
        XCTAssertEqual(try CompanionStateCodec.open(bytes, key: key), state)
        XCTAssertThrowsError(try CompanionStateCodec.open(
            bytes,
            key: SymmetricKey(data: Data(repeating: 2, count: 32))
        ))

        let payload = Data("exact reference chunk".utf8)
        let binding = Data("command_fixture\0blob_fixture\00\01".utf8)
        let sealedPayload = try CompanionLocalPayloadCodec.seal(
            payload,
            key: key,
            binding: binding
        )
        XCTAssertEqual(
            try CompanionLocalPayloadCodec.open(sealedPayload, key: key, binding: binding),
            payload
        )
        XCTAssertThrowsError(try CompanionLocalPayloadCodec.open(
            sealedPayload,
            key: key,
            binding: Data("different binding".utf8)
        ))
        XCTAssertThrowsError(try CompanionLocalPayloadCodec.open(
            sealedPayload,
            key: SymmetricKey(data: Data(repeating: 2, count: 32)),
            binding: binding
        ))

        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-state-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let target = root.appendingPathComponent("target")
        try Data("unsafe".utf8).write(to: target)
        let link = root.appendingPathComponent("state.bin")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: target)
        let store = try FileCompanionStateStore(fileURL: link)
        await XCTAssertThrowsErrorAsync { _ = try await store.load() }
    }

    func testPreIconStateDecodesWithAnEmptyCompatibleCache() throws {
        let encoded = try StrictJSON.encode(CompanionPersistentState())
        var object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )
        object.removeValue(forKey: "icon_blobs")
        object.removeValue(forKey: "reference_outbox")
        let oldShape = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        let decoded = try StrictJSON.decode(CompanionPersistentState.self, from: oldShape)
        XCTAssertTrue(decoded.iconBlobs.isEmpty)
        XCTAssertTrue(decoded.referenceOutbox.isEmpty)
    }

    func testReferencePNGJPEGChunkingTamperAndFileStoreSymlink() async throws {
        let pngPrefix = try XCTUnwrap(Data(base64Encoded:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        ))
        var largePNG = pngPrefix
        largePNG.append(Data(
            repeating: 0,
            count: CompanionReferenceBlob.maximumChunkByteLength + 1 - pngPrefix.count
        ))
        let blob = try CompanionReferenceBlob(
            blobID: "reference_large",
            originName: "large.png",
            mediaType: "image/png",
            bytes: largePNG
        )
        let chunks = try blob.transportChunks()
        XCTAssertEqual(chunks.count, 2)
        var assembler = CompanionReferenceBlobAssembler()
        XCTAssertEqual(try assembler.admit(chunks[1]), .stored)
        XCTAssertEqual(try assembler.admit(chunks[0]), .complete(blob))
        XCTAssertEqual(try assembler.admit(chunks[0]), .duplicate)

        var tampered = try XCTUnwrap(
            JSONSerialization.jsonObject(with: StrictJSON.encode(chunks[0])) as? [String: Any]
        )
        tampered["bytes"] = "AA"
        XCTAssertThrowsError(try StrictJSON.decode(
            CompanionReferenceBlobChunk.self,
            from: JSONSerialization.data(withJSONObject: tampered, options: [.sortedKeys]),
            maximumBytes: CompanionLimits.maximumEnvelopeBodyBytes
        ))

        let jpeg = Data([
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01,
            0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00, 0xff, 0xd9,
        ])
        _ = try CompanionReferenceBlob(
            blobID: "reference_jpeg",
            originName: "reference.jpg",
            mediaType: "image/jpeg",
            bytes: jpeg
        )
        XCTAssertThrowsError(try CompanionReferenceBlob(
            blobID: "reference_wrong_type",
            originName: "reference.jpg",
            mediaType: "image/jpeg",
            bytes: pngPrefix
        ))

        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-payloads-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let target = root.appendingPathComponent("target")
        try Data("unsafe".utf8).write(to: target)
        let link = root.appendingPathComponent("payload_fixture.envelope")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: target)
        let store = try FileCompanionPayloadStore(directoryURL: root)
        await XCTAssertThrowsErrorAsync { _ = try await store.load(id: "payload_fixture") }

        let clean = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-payloads-clean-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: clean) }
        let durable = try FileCompanionPayloadStore(directoryURL: clean)
        let envelopeBytes = Data("{\"encrypted\":true}".utf8)
        try await durable.save(id: "payload_fixture", bytes: envelopeBytes)
        let loaded = try await durable.load(id: "payload_fixture")
        XCTAssertEqual(loaded, envelopeBytes)
        try await durable.save(id: "payload_fixture", bytes: envelopeBytes)
        await XCTAssertThrowsErrorAsync {
            try await durable.save(id: "payload_fixture", bytes: Data("different".utf8))
        }
        try await durable.retainOnly(ids: [])
        let removed = try await durable.load(id: "payload_fixture")
        XCTAssertNil(removed)
    }

    func testRelayVerifiersDigestDecodedCapabilities() throws {
        let secrets = PairingRelaySecrets(
            write: Base64URL.encode(Data(repeating: 1, count: 32)),
            read: Base64URL.encode(Data(repeating: 2, count: 32)),
            acknowledgement: Base64URL.encode(Data(repeating: 3, count: 32)),
            revocation: Base64URL.encode(Data(repeating: 4, count: 32)),
            push: Base64URL.encode(Data(repeating: 5, count: 32))
        )
        let verifiers = try secrets.verifiers()
        XCTAssertEqual(
            verifiers.writeVerifier,
            Data(repeating: 1, count: 32).companionSHA256
                .map { String(format: "%02x", $0) }
                .joined()
        )
        XCTAssertNotEqual(
            verifiers.writeVerifier,
            Data(secrets.write.utf8).companionSHA256
                .map { String(format: "%02x", $0) }
                .joined(),
            "relay capability verifiers hash decoded secret bytes, never their base64url text"
        )
    }

    func testTimestampParserRejectsNonASCIIWithoutTrapping() {
        XCTAssertThrowsError(try CompanionTimestamp.parse("💥💥💥💥💥"))
        XCTAssertThrowsError(try CompanionTimestamp.parse("2026-08-15T12:01:00+00:00"))
    }
}

private actor NoopRelay: CompanionRelayTransport {
    func createMailbox(endpoint: RelayEndpoint, verifiers: RelayMailboxVerifiers) throws -> RelayCreatedMailbox {
        throw TohsenoCompanionError.transportUnavailable
    }
    func submitPairingResponse(endpoint: RelayEndpoint, sessionID: String, opaqueResponse: Data) throws {
        throw TohsenoCompanionError.transportUnavailable
    }
    func uploadEnvelope(
        endpoint: RelayEndpoint,
        mailboxID: String,
        writeCapability: String,
        envelope: OpaqueCompanionEnvelope
    ) throws -> RelayEnvelopeUploadReceipt { throw TohsenoCompanionError.transportUnavailable }
    func fetchEnvelopes(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) throws -> RelayMailboxPage { throw TohsenoCompanionError.transportUnavailable }
    func acknowledge(
        endpoint: RelayEndpoint,
        mailboxID: String,
        acknowledgementCapability: String,
        cursor: UInt64
    ) throws { throw TohsenoCompanionError.transportUnavailable }
    func liveEvents(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) throws -> AsyncThrowingStream<RelayLiveEvent, Error> {
        throw TohsenoCompanionError.transportUnavailable
    }
    func registerPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String,
        token: Data
    ) throws { throw TohsenoCompanionError.transportUnavailable }
    func unregisterPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String
    ) throws { throw TohsenoCompanionError.transportUnavailable }
}

private func XCTAssertThrowsErrorAsync(
    _ expression: @escaping () async throws -> Void,
    file: StaticString = #filePath,
    line: UInt = #line
) async {
    do {
        try await expression()
        XCTFail("expected an error", file: file, line: line)
    } catch {}
}
