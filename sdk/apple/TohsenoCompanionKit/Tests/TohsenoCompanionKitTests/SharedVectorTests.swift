import CryptoKit
import Foundation
import XCTest
@testable import TohsenoCompanionKit

final class SharedVectorTests: XCTestCase {
    func testSingleSharedRustSwiftFixtureByteForByte() async throws {
        let fixture = try loadSharedFixture()
        XCTAssertEqual(Set(fixture.keys), [
            "schema", "test_only", "bip39_official", "companion_identity",
            "workspace_service_identity", "pairing", "capability", "command",
            "snapshot_request_command", "pairing_acceptance", "icon_blob", "reference_blob",
            "envelope", "relay",
            "negative",
        ])
        XCTAssertEqual(fixture["schema"] as? String, "tohseno.companion-test-vectors/1")
        XCTAssertEqual(fixture["test_only"] as? Bool, true)

        let identityVector = try object(fixture, "companion_identity")
        let phrase = try RecoveryPhrase(try string(identityVector, "mnemonic"))
        XCTAssertEqual(
            Base64URL.encode(phrase.rawEntropy),
            try string(identityVector, "entropy_base64url")
        )
        XCTAssertEqual(Base64URL.encode(phrase.seed()), try string(identityVector, "seed_base64url"))
        let keys = try CompanionKeyDerivation.derive(phrase: phrase)
        XCTAssertEqual(
            Base64URL.encode(keys.signingPrivateKey),
            try string(identityVector, "signing_secret_key_base64url")
        )
        XCTAssertEqual(
            Base64URL.encode(keys.agreementPrivateKey),
            try string(identityVector, "agreement_secret_key_base64url")
        )
        XCTAssertEqual(Base64URL.encode(keys.storageKey), try string(identityVector, "storage_key_base64url"))
        let phone = try CompanionIdentity(phrase: phrase)
        XCTAssertEqual(phone.description.signingPublicKey, try string(identityVector, "signing_public_key_base64url"))
        XCTAssertEqual(phone.description.agreementPublicKey, try string(identityVector, "agreement_public_key_base64url"))
        XCTAssertEqual(phone.description.deviceID, try string(identityVector, "device_id"))

        let workspaceVector = try object(fixture, "workspace_service_identity")
        let workspace = try CompanionIdentity(
            signingPrivateKey: Base64URL.decode(
                try string(workspaceVector, "signing_secret_key_base64url"),
                expectedBytes: 32
            ),
            agreementPrivateKey: Base64URL.decode(
                try string(workspaceVector, "agreement_secret_key_base64url"),
                expectedBytes: 32
            )
        )
        XCTAssertEqual(workspace.description.signingPublicKey, try string(workspaceVector, "signing_public_key_base64url"))
        XCTAssertEqual(workspace.description.agreementPublicKey, try string(workspaceVector, "agreement_public_key_base64url"))
        XCTAssertEqual(workspace.description.deviceID, try string(workspaceVector, "device_id"))

        let pairing = try object(fixture, "pairing")
        let invitation = try decode(PairingInvitation.self, pairing["invitation"])
        XCTAssertEqual(
            Base64URL.encode(try invitation.canonicalBody()),
            try string(pairing, "invitation_body_canonical_base64url")
        )
        XCTAssertEqual(
            PairingInvitation.uriPrefix + Base64URL.encode(try invitation.canonicalJSON()),
            try string(pairing, "invitation_uri")
        )
        let relayAllowlist = try RelayAllowlist([
            RelayEndpoint(id: "official-v1", baseURL: URL(string: "https://companion.tohseno.com")!),
        ])
        let parsed = try PairingInvitation.parse(
            uri: try string(pairing, "invitation_uri"),
            allowlist: relayAllowlist,
            now: try CompanionTimestamp.parse("2026-08-15T12:01:00Z"),
            trustedStudioSigningKey: workspace.signingKey.publicKey.rawRepresentation
        )
        XCTAssertEqual(parsed.0, invitation)
        let proof = try decode(PairingProof.self, pairing["proof"])
        XCTAssertEqual(
            Base64URL.encode(try proof.canonicalBody()),
            try string(pairing, "proof_body_canonical_base64url")
        )
        let recreatedProof = try PairingProof.create(
            invitation: invitation,
            identity: phone,
            displayName: "Vector iPhone",
            createdAt: "2026-08-15T12:01:00Z"
        )
        XCTAssertEqual(try recreatedProof.canonicalBody(), try proof.canonicalBody())
        XCTAssertEqual(recreatedProof.keyConfirmation, proof.keyConfirmation)
        XCTAssertTrue(try CompanionIdentity.verify(
            publicKey: phone.signingKey.publicKey.rawRepresentation,
            domain: PairingProof.signatureDomain,
            message: recreatedProof.canonicalBody(),
            signature: Base64URL.decode(recreatedProof.signature, expectedBytes: 64)
        ))
        try proof.verify(
            invitation: invitation,
            studioEphemeralPrivateKey: Base64URL.decode(
                try string(pairing, "studio_ephemeral_secret_key_base64url"),
                expectedBytes: 32
            ),
            now: try CompanionTimestamp.parse("2026-08-15T12:01:01Z")
        )
        let encryptedResponse = try decode(
            EncryptedPairingResponse.self,
            pairing["encrypted_response"]
        )
        XCTAssertEqual(
            Base64URL.encode(try encryptedResponse.canonicalJSON()),
            try string(pairing, "encrypted_response_canonical_base64url")
        )
        let responseBody = try PairingResponseCrypto.open(
            encryptedResponse,
            invitation: invitation,
            studioEphemeralPrivateKey: Base64URL.decode(
                try string(pairing, "studio_ephemeral_secret_key_base64url"),
                expectedBytes: 32
            )
        )
        XCTAssertEqual(responseBody.proof, proof)
        XCTAssertEqual(
            responseBody,
            try decode(PairingResponseBody.self, pairing["response_body"])
        )
        XCTAssertEqual(
            Base64URL.encode(try responseBody.canonicalJSON()),
            try string(pairing, "response_body_canonical_base64url")
        )
        XCTAssertEqual(
            try PairingResponseCrypto.seal(
                proof: proof,
                invitation: invitation,
                responseMailboxID: responseBody.responseMailboxID,
                responseMailboxWriteCapability: responseBody.responseMailboxWriteCapability,
                responseMailboxRevocationCapability: responseBody.responseMailboxRevocationCapability,
                responseEphemeralSecret: Data(repeating: 10, count: 32),
                nonce: Data(repeating: 11, count: 12)
            ),
            try encryptedResponse.canonicalJSON()
        )

        let capability = try object(fixture, "capability")
        let grant = try decode(CapabilityGrant.self, capability["grant"])
        XCTAssertEqual(
            Base64URL.encode(try grant.canonicalBody()),
            try string(capability, "body_canonical_base64url")
        )
        try grant.verify(
            trustedStudioSigningKey: workspace.signingKey.publicKey.rawRepresentation,
            expectedWorkspaceID: "workspace_vector_001",
            expectedDeviceID: phone.description.deviceID,
            now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z")
        )
        for action in CompanionCapability.allCases { try grant.require(action) }

        let acceptanceVector = try object(fixture, "pairing_acceptance")
        let acceptance = try decode(
            CompanionPairingGrantPackage.self,
            acceptanceVector["acceptance"]
        )
        XCTAssertEqual(acceptance.capabilityGrant, grant)
        XCTAssertEqual(
            Base64URL.encode(try acceptance.canonicalJSON()),
            try string(acceptanceVector, "canonical_base64url")
        )

        let commandVector = try object(fixture, "command")
        let command = try decode(CompanionCommand.self, commandVector["command"])
        XCTAssertEqual(
            Base64URL.encode(try command.canonicalBody()),
            try string(commandVector, "body_canonical_base64url")
        )
        try command.verify(
            expectedSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedDeviceID: phone.description.deviceID,
            now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z")
        )
        let snapshotCommandVector = try object(fixture, "snapshot_request_command")
        let snapshotCommand = try decode(
            CompanionCommand.self,
            snapshotCommandVector["command"]
        )
        XCTAssertEqual(snapshotCommand.payload, .workspaceSnapshotRequest)
        XCTAssertEqual(
            Base64URL.encode(try snapshotCommand.canonicalBody()),
            try string(snapshotCommandVector, "body_canonical_base64url")
        )
        try snapshotCommand.verify(
            expectedSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedDeviceID: phone.description.deviceID,
            now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z")
        )

        let iconVector = try object(fixture, "icon_blob")
        let iconBlob = try decode(CompanionIconBlob.self, iconVector["blob"])
        try iconBlob.validate()
        XCTAssertEqual(iconBlob.width, 1)
        XCTAssertEqual(iconBlob.height, 1)
        XCTAssertEqual(
            Base64URL.encode(try StrictJSON.encode(iconBlob)),
            try string(iconVector, "canonical_base64url")
        )

        let referenceVector = try object(fixture, "reference_blob")
        let referenceBlob = try decode(CompanionReferenceBlob.self, referenceVector["blob"])
        try referenceBlob.validate()
        XCTAssertEqual(
            Base64URL.encode(try StrictJSON.encode(referenceBlob)),
            try string(referenceVector, "canonical_base64url")
        )
        let chunkValues = try XCTUnwrap(referenceVector["chunks"] as? [Any])
        let chunkCanonical = try XCTUnwrap(
            referenceVector["chunk_canonical_base64url"] as? [String]
        )
        XCTAssertEqual(chunkValues.count, chunkCanonical.count)
        var referenceAssembler = CompanionReferenceBlobAssembler()
        for (index, value) in chunkValues.enumerated() {
            let chunk = try decode(CompanionReferenceBlobChunk.self, value)
            XCTAssertEqual(
                Base64URL.encode(try StrictJSON.encode(chunk)),
                chunkCanonical[index]
            )
            let admission = try referenceAssembler.admit(chunk)
            if index + 1 == chunkValues.count {
                XCTAssertEqual(admission, .complete(referenceBlob))
            }
        }

        let envelopeVector = try object(fixture, "envelope")
        let envelope = try decode(OpaqueCompanionEnvelope.self, envelopeVector["envelope"])
        XCTAssertEqual(
            Base64URL.encode(try envelope.canonicalHeader()),
            try string(envelopeVector, "header_canonical_base64url")
        )
        XCTAssertEqual(
            Base64URL.encode(try envelope.canonicalUnsigned()),
            try string(envelopeVector, "unsigned_canonical_base64url")
        )
        let plaintext = try Base64URL.decode(try string(envelopeVector, "plaintext_base64url"))
        let recreatedEnvelope = try CompanionEnvelopeCrypto.seal(
            sender: phone,
            recipientAgreementPublicKey: workspace.agreementKey.publicKey.rawRepresentation,
            metadata: CompanionEnvelopeMetadata(
                envelopeID: envelope.envelopeID,
                mailboxID: envelope.mailboxID,
                recipientDeviceID: envelope.recipientDeviceID,
                senderSequence: envelope.senderSequence,
                createdAt: envelope.createdAt,
                expiresAt: envelope.expiresAt
            ),
            plaintext: plaintext,
            ephemeralSecret: Data(repeating: 31, count: 32),
            nonce: Data(repeating: 32, count: 12)
        )
        XCTAssertEqual(try recreatedEnvelope.canonicalHeader(), try envelope.canonicalHeader())
        XCTAssertEqual(recreatedEnvelope.ciphertext, envelope.ciphertext)
        XCTAssertEqual(try recreatedEnvelope.canonicalUnsigned(), try envelope.canonicalUnsigned())
        XCTAssertTrue(try CompanionIdentity.verify(
            publicKey: phone.signingKey.publicKey.rawRepresentation,
            domain: OpaqueCompanionEnvelope.signatureDomain,
            message: recreatedEnvelope.canonicalUnsigned(),
            signature: Base64URL.decode(recreatedEnvelope.signature, expectedBytes: 64)
        ))
        let replay = try CompanionReplayProtection(capacity: 128)
        let opened = try await CompanionEnvelopeCrypto.open(
            envelope,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            recipient: workspace,
            now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z"),
            replay: replay
        )
        XCTAssertEqual(opened, plaintext)

        let relay = try object(fixture, "relay")
        XCTAssertEqual(try decode(OpaqueCompanionEnvelope.self, relay["direct_envelope"]), envelope)
        let mailboxPage = try decode(RelayMailboxPage.self, relay["mailbox_page"])
        XCTAssertEqual(mailboxPage.envelopes.first?.envelope, envelope)
        try mailboxPage.validateRouting(mailboxID: envelope.mailboxID, afterCursor: 0)
        XCTAssertThrowsError(
            try mailboxPage.validateRouting(mailboxID: "another_mailbox", afterCursor: 0)
        )

        try await verifyNegativeVectors(
            fixture: fixture,
            phone: phone,
            workspace: workspace,
            invitation: invitation,
            proof: proof,
            grant: grant,
            command: command,
            envelope: envelope,
            pairing: pairing,
            allowlist: relayAllowlist
        )
    }

    private func verifyNegativeVectors(
        fixture: [String: Any],
        phone: CompanionIdentity,
        workspace: CompanionIdentity,
        invitation: PairingInvitation,
        proof: PairingProof,
        grant: CapabilityGrant,
        command: CompanionCommand,
        envelope: OpaqueCompanionEnvelope,
        pairing: [String: Any],
        allowlist: RelayAllowlist
    ) async throws {
        let negatives = try XCTUnwrap(fixture["negative"] as? [[String: Any]])
        XCTAssertEqual(Set(negatives.compactMap { $0["name"] as? String }), [
            "invitation_signature_tamper", "invitation_unallowlisted_relay",
            "pairing_key_confirmation_tamper", "pairing_response_ciphertext_tamper",
            "capability_signature_tamper",
            "command_signature_tamper", "envelope_ciphertext_tamper", "envelope_replay",
            "icon_blob_bytes_tamper", "reference_blob_bytes_tamper",
            "reference_chunk_bytes_tamper",
        ])
        for vector in negatives {
            let name = try string(vector, "name")
            switch name {
            case "invitation_signature_tamper", "invitation_unallowlisted_relay":
                let mutated: PairingInvitation = try mutatedValue(
                    base: pairing["invitation"],
                    vector: vector
                )
                XCTAssertThrowsError(try mutated.verify(
                    now: try CompanionTimestamp.parse("2026-08-15T12:01:00Z"),
                    trustedStudioSigningKey: workspace.signingKey.publicKey.rawRepresentation
                ))
            case "pairing_key_confirmation_tamper":
                let mutated: PairingProof = try mutatedValue(base: pairing["proof"], vector: vector)
                XCTAssertThrowsError(try mutated.verify(
                    invitation: invitation,
                    studioEphemeralPrivateKey: Base64URL.decode(
                        try string(pairing, "studio_ephemeral_secret_key_base64url"),
                        expectedBytes: 32
                    ),
                    now: try CompanionTimestamp.parse("2026-08-15T12:01:01Z")
                ))
            case "pairing_response_ciphertext_tamper":
                let mutated: EncryptedPairingResponse = try mutatedValue(
                    base: pairing["encrypted_response"],
                    vector: vector
                )
                XCTAssertThrowsError(try PairingResponseCrypto.open(
                    mutated,
                    invitation: invitation,
                    studioEphemeralPrivateKey: Base64URL.decode(
                        try string(pairing, "studio_ephemeral_secret_key_base64url"),
                        expectedBytes: 32
                    )
                ))
            case "capability_signature_tamper":
                let mutated: CapabilityGrant = try mutatedValue(base: try object(fixture, "capability")["grant"], vector: vector)
                XCTAssertThrowsError(try mutated.verify(
                    trustedStudioSigningKey: workspace.signingKey.publicKey.rawRepresentation,
                    now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z")
                ))
            case "command_signature_tamper":
                let mutated: CompanionCommand = try mutatedValue(base: try object(fixture, "command")["command"], vector: vector)
                XCTAssertThrowsError(try mutated.verify(
                    expectedSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
                    expectedDeviceID: phone.description.deviceID,
                    now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z")
                ))
            case "envelope_ciphertext_tamper":
                let mutated: OpaqueCompanionEnvelope = try mutatedValue(base: try object(fixture, "envelope")["envelope"], vector: vector)
                await XCTAssertThrowsErrorAsync {
                    _ = try await CompanionEnvelopeCrypto.open(
                        mutated,
                        expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
                        expectedSenderDeviceID: phone.description.deviceID,
                        recipient: workspace,
                        now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z"),
                        replay: try CompanionReplayProtection(capacity: 128)
                    )
                }
            case "envelope_replay":
                let replay = try CompanionReplayProtection(capacity: 128)
                _ = try await CompanionEnvelopeCrypto.open(
                    envelope,
                    expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
                    expectedSenderDeviceID: phone.description.deviceID,
                    recipient: workspace,
                    now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z"),
                    replay: replay
                )
                await XCTAssertThrowsErrorAsync {
                    _ = try await CompanionEnvelopeCrypto.open(
                        envelope,
                        expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
                        expectedSenderDeviceID: phone.description.deviceID,
                        recipient: workspace,
                        now: try CompanionTimestamp.parse("2026-08-15T12:02:00Z"),
                        replay: replay
                    )
                }
            case "icon_blob_bytes_tamper":
                XCTAssertThrowsError(
                    try mutatedValue(
                        base: try object(fixture, "icon_blob")["blob"],
                        vector: vector
                    ) as CompanionIconBlob
                )
            case "reference_blob_bytes_tamper":
                XCTAssertThrowsError(
                    try mutatedValue(
                        base: try object(fixture, "reference_blob")["blob"],
                        vector: vector
                    ) as CompanionReferenceBlob
                )
            case "reference_chunk_bytes_tamper":
                let reference = try object(fixture, "reference_blob")
                let chunks = try XCTUnwrap(reference["chunks"] as? [Any])
                XCTAssertThrowsError(
                    try mutatedValue(base: chunks[0], vector: vector)
                        as CompanionReferenceBlobChunk
                )
            default:
                XCTFail("unhandled shared negative vector \(name)")
            }
        }
        _ = proof
        _ = grant
        _ = command
        _ = allowlist
    }
}

private func loadSharedFixture() throws -> [String: Any] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0 ..< 10 {
        let candidate = directory
            .appendingPathComponent("companion")
            .appendingPathComponent("test-vectors")
            .appendingPathComponent("companion-v1.json")
        if FileManager.default.fileExists(atPath: candidate.path) {
            let data = try Data(contentsOf: candidate)
            return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        }
        directory.deleteLastPathComponent()
    }
    let bundled = try XCTUnwrap(Bundle.module.url(
        forResource: "companion-v1",
        withExtension: "json",
        subdirectory: "TestVectors"
    ))
    return try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: bundled)) as? [String: Any]
    )
}

private func object(_ object: [String: Any], _ key: String) throws -> [String: Any] {
    try XCTUnwrap(object[key] as? [String: Any])
}

private func string(_ object: [String: Any], _ key: String) throws -> String {
    try XCTUnwrap(object[key] as? String)
}

private func decode<T: Decodable>(_ type: T.Type, _ value: Any?) throws -> T {
    let value = try XCTUnwrap(value)
    let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    return try StrictJSON.decode(type, from: data, maximumBytes: 4 * 1024 * 1024)
}

private func mutatedValue<T: Decodable>(base: Any?, vector: [String: Any]) throws -> T {
    var root = try XCTUnwrap(base as? [String: Any])
    guard try string(vector, "operation") == "replace_json_value",
          let pointer = vector["json_pointer"] as? String,
          let replacement = vector["replacement"] else {
        throw TohsenoCompanionError.invalidEncoding("negative vector operation")
    }
    let components = pointer.split(separator: "/").map(String.init)
    guard components.count == 1 || components.count == 2 else {
        throw TohsenoCompanionError.invalidEncoding("negative vector pointer")
    }
    if components.count == 1 {
        root[components[0]] = replacement
    } else {
        var nested = try XCTUnwrap(root[components[0]] as? [String: Any])
        nested[components[1]] = replacement
        root[components[0]] = nested
    }
    return try decode(T.self, root)
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
