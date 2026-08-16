import CryptoKit
import Foundation

/// The encrypted phone-to-Mac rendezvous body. The relay sees only this outer
/// session ID, nonce, and ciphertext; response-mailbox capabilities are inside it.
public struct EncryptedPairingResponse: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-encrypted-pairing-response/1"
    public let schema: String
    public let sessionID: String
    public let ephemeralPublicKey: String
    public let nonce: String
    public let ciphertext: String

    enum CodingKeys: String, CodingKey {
        case schema
        case sessionID = "session_id"
        case ephemeralPublicKey = "ephemeral_public_key"
        case nonce, ciphertext
    }

    public init(
        schema: String = schemaV1,
        sessionID: String,
        ephemeralPublicKey: String,
        nonce: String,
        ciphertext: String
    ) {
        self.schema = schema
        self.sessionID = sessionID
        self.ephemeralPublicKey = ephemeralPublicKey
        self.nonce = nonce
        self.ciphertext = ciphertext
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "session_id", "ephemeral_public_key", "nonce", "ciphertext",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        sessionID = try container.decode(String.self, forKey: .sessionID)
        ephemeralPublicKey = try container.decode(String.self, forKey: .ephemeralPublicKey)
        nonce = try container.decode(String.self, forKey: .nonce)
        ciphertext = try container.decode(String.self, forKey: .ciphertext)
    }

    func canonicalHeader() throws -> Data {
        try CanonicalValue.object([
            "ephemeral_public_key": .string(ephemeralPublicKey),
            "nonce": .string(nonce), "schema": .string(schema),
            "session_id": .string(sessionID),
        ]).data()
    }

    func canonicalJSON() throws -> Data {
        try CanonicalValue.object([
            "ciphertext": .string(ciphertext),
            "ephemeral_public_key": .string(ephemeralPublicKey),
            "nonce": .string(nonce), "schema": .string(schema),
            "session_id": .string(sessionID),
        ]).data()
    }
}

/// The first decrypted object delivered by the Mac to the phone's freshly
/// created response mailbox.
public struct CompanionPairingGrantPackage: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-pairing-grant-package/1"
    public let schema: String
    public let capabilityGrant: CapabilityGrant
    public let studioAgreementPublicKey: String
    public let commandMailboxID: String
    public let commandMailboxWriteCapability: String

    enum CodingKeys: String, CodingKey {
        case schema
        case capabilityGrant = "capability_grant"
        case studioAgreementPublicKey = "studio_agreement_public_key"
        case commandMailboxID = "command_mailbox_id"
        case commandMailboxWriteCapability = "command_mailbox_write_capability"
    }

    public init(
        schema: String = schemaV1,
        capabilityGrant: CapabilityGrant,
        studioAgreementPublicKey: String,
        commandMailboxID: String,
        commandMailboxWriteCapability: String
    ) {
        self.schema = schema
        self.capabilityGrant = capabilityGrant
        self.studioAgreementPublicKey = studioAgreementPublicKey
        self.commandMailboxID = commandMailboxID
        self.commandMailboxWriteCapability = commandMailboxWriteCapability
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "capability_grant", "studio_agreement_public_key",
            "command_mailbox_id", "command_mailbox_write_capability",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        capabilityGrant = try container.decode(CapabilityGrant.self, forKey: .capabilityGrant)
        studioAgreementPublicKey = try container.decode(String.self, forKey: .studioAgreementPublicKey)
        commandMailboxID = try container.decode(String.self, forKey: .commandMailboxID)
        commandMailboxWriteCapability = try container.decode(String.self, forKey: .commandMailboxWriteCapability)
    }
}

struct PairingRelaySecrets: Sendable {
    let write: String
    let read: String
    let acknowledgement: String
    let revocation: String
    let push: String

    static func generate(entropy: any CompanionEntropySource) throws -> Self {
        try Self(
            write: Base64URL.encode(entropy.randomBytes(count: 32)),
            read: Base64URL.encode(entropy.randomBytes(count: 32)),
            acknowledgement: Base64URL.encode(entropy.randomBytes(count: 32)),
            revocation: Base64URL.encode(entropy.randomBytes(count: 32)),
            push: Base64URL.encode(entropy.randomBytes(count: 32))
        )
    }

    func verifiers() throws -> RelayMailboxVerifiers {
        try RelayMailboxVerifiers(
            writeVerifier: verifier(write),
            readVerifier: verifier(read),
            acknowledgementVerifier: verifier(acknowledgement),
            revocationVerifier: verifier(revocation),
            pushVerifier: verifier(push)
        )
    }

    func inbox(mailboxID: String) -> CompanionInboxAccess {
        CompanionInboxAccess(
            mailboxID: mailboxID,
            readCapability: read,
            acknowledgementCapability: acknowledgement,
            revocationCapability: revocation,
            pushCapability: push
        )
    }

    private func verifier(_ capability: String) throws -> String {
        try Base64URL.decode(capability, expectedBytes: 32)
            .companionSHA256
            .map { String(format: "%02x", $0) }
            .joined()
    }
}

struct PairingResponseBody: Codable, Equatable, Sendable {
    let schema: String
    let proof: PairingProof
    let responseMailboxID: String
    let responseMailboxWriteCapability: String
    let responseMailboxRevocationCapability: String

    enum CodingKeys: String, CodingKey {
        case schema, proof
        case responseMailboxID = "response_mailbox_id"
        case responseMailboxWriteCapability = "response_mailbox_write_capability"
        case responseMailboxRevocationCapability = "response_mailbox_revoke_capability"
    }

    init(
        schema: String,
        proof: PairingProof,
        responseMailboxID: String,
        responseMailboxWriteCapability: String,
        responseMailboxRevocationCapability: String
    ) {
        self.schema = schema
        self.proof = proof
        self.responseMailboxID = responseMailboxID
        self.responseMailboxWriteCapability = responseMailboxWriteCapability
        self.responseMailboxRevocationCapability = responseMailboxRevocationCapability
    }

    init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "proof", "response_mailbox_id",
            "response_mailbox_write_capability", "response_mailbox_revoke_capability",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        proof = try container.decode(PairingProof.self, forKey: .proof)
        responseMailboxID = try container.decode(String.self, forKey: .responseMailboxID)
        responseMailboxWriteCapability = try container.decode(
            String.self,
            forKey: .responseMailboxWriteCapability
        )
        responseMailboxRevocationCapability = try container.decode(
            String.self,
            forKey: .responseMailboxRevocationCapability
        )
    }

    func validate(invitation: PairingInvitation) throws {
        guard schema == "tohseno.companion-pairing-response-body/1",
              proof.sessionID == invitation.sessionID,
              proof.workspaceID == invitation.workspaceID,
              responseMailboxWriteCapability != responseMailboxRevocationCapability
        else { throw TohsenoCompanionError.invalidInvitation("pairing response binding differs") }
        try requireIdentifier(responseMailboxID, field: "response_mailbox_id")
        _ = try Base64URL.decode(responseMailboxWriteCapability, expectedBytes: 32)
        _ = try Base64URL.decode(responseMailboxRevocationCapability, expectedBytes: 32)
    }
}

enum PairingResponseCrypto {
    private static let keyDomain = "tohseno.companion.pairing-response-key.v1"

    static func seal(
        proof: PairingProof,
        invitation: PairingInvitation,
        responseMailboxID: String,
        responseMailboxWriteCapability: String,
        responseMailboxRevocationCapability: String,
        entropy: any CompanionEntropySource
    ) throws -> Data {
        try seal(
            proof: proof,
            invitation: invitation,
            responseMailboxID: responseMailboxID,
            responseMailboxWriteCapability: responseMailboxWriteCapability,
            responseMailboxRevocationCapability: responseMailboxRevocationCapability,
            responseEphemeralSecret: entropy.randomBytes(count: 32),
            nonce: entropy.randomBytes(count: 12)
        )
    }

    static func seal(
        proof: PairingProof,
        invitation: PairingInvitation,
        responseMailboxID: String,
        responseMailboxWriteCapability: String,
        responseMailboxRevocationCapability: String,
        responseEphemeralSecret: Data,
        nonce: Data
    ) throws -> Data {
        try requireIdentifier(responseMailboxID, field: "response_mailbox_id")
        _ = try Base64URL.decode(responseMailboxWriteCapability, expectedBytes: 32)
        _ = try Base64URL.decode(responseMailboxRevocationCapability, expectedBytes: 32)
        guard responseMailboxWriteCapability != responseMailboxRevocationCapability,
              proof.sessionID == invitation.sessionID,
              proof.workspaceID == invitation.workspaceID,
              responseEphemeralSecret.count == 32,
              nonce.count == 12
        else { throw TohsenoCompanionError.invalidInvitation("pairing response binding differs") }
        let body = PairingResponseBody(
            schema: "tohseno.companion-pairing-response-body/1",
            proof: proof,
            responseMailboxID: responseMailboxID,
            responseMailboxWriteCapability: responseMailboxWriteCapability,
            responseMailboxRevocationCapability: responseMailboxRevocationCapability
        )
        let plaintext = try body.canonicalJSON()
        let studio = try Curve25519.KeyAgreement.PublicKey(
            rawRepresentation: Base64URL.decode(
                invitation.studioEphemeralAgreementPublicKey,
                expectedBytes: 32
            )
        )
        let responseEphemeral = try Curve25519.KeyAgreement.PrivateKey(
            rawRepresentation: responseEphemeralSecret
        )
        let draft = EncryptedPairingResponse(
            sessionID: invitation.sessionID,
            ephemeralPublicKey: Base64URL.encode(responseEphemeral.publicKey.rawRepresentation),
            nonce: Base64URL.encode(nonce),
            ciphertext: "pending"
        )
        let associatedData = try draft.canonicalHeader()
        let shared = try responseEphemeral.sharedSecretFromKeyAgreement(with: studio)
        let digest = try invitation.digest()
        let key = shared.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: digest,
            sharedInfo: Data(keyDomain.utf8),
            outputByteCount: 32
        )
        let box = try ChaChaPoly.seal(
            plaintext,
            using: key,
            nonce: ChaChaPoly.Nonce(data: nonce),
            authenticating: associatedData
        )
        var encrypted = box.ciphertext
        encrypted.append(box.tag)
        return try EncryptedPairingResponse(
            sessionID: draft.sessionID,
            ephemeralPublicKey: draft.ephemeralPublicKey,
            nonce: draft.nonce,
            ciphertext: Base64URL.encode(encrypted)
        ).canonicalJSON()
    }

    static func open(
        _ response: EncryptedPairingResponse,
        invitation: PairingInvitation,
        studioEphemeralPrivateKey: Data
    ) throws -> PairingResponseBody {
        guard response.schema == EncryptedPairingResponse.schemaV1,
              response.sessionID == invitation.sessionID,
              studioEphemeralPrivateKey.count == 32
        else { throw TohsenoCompanionError.invalidInvitation("encrypted response binding differs") }
        let ephemeral = try Curve25519.KeyAgreement.PublicKey(
            rawRepresentation: Base64URL.decode(response.ephemeralPublicKey, expectedBytes: 32)
        )
        let nonce = try Base64URL.decode(response.nonce, expectedBytes: 12)
        let encrypted = try Base64URL.decode(response.ciphertext)
        guard encrypted.count >= 16, encrypted.count <= 256 * 1024 else {
            throw TohsenoCompanionError.responseTooLarge
        }
        let studio = try Curve25519.KeyAgreement.PrivateKey(
            rawRepresentation: studioEphemeralPrivateKey
        )
        let shared = try studio.sharedSecretFromKeyAgreement(with: ephemeral)
        let key = shared.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: try invitation.digest(),
            sharedInfo: Data(keyDomain.utf8),
            outputByteCount: 32
        )
        let split = encrypted.count - 16
        let box = try ChaChaPoly.SealedBox(
            nonce: ChaChaPoly.Nonce(data: nonce),
            ciphertext: encrypted.prefix(split),
            tag: encrypted.suffix(16)
        )
        let plaintext: Data
        do {
            plaintext = try ChaChaPoly.open(
                box,
                using: key,
                authenticating: response.canonicalHeader()
            )
        } catch {
            throw TohsenoCompanionError.invalidInvitation("pairing response authentication failed")
        }
        let body = try StrictJSON.decode(
            PairingResponseBody.self,
            from: plaintext,
            maximumBytes: 256 * 1024
        )
        guard try body.canonicalJSON() == plaintext else {
            throw TohsenoCompanionError.invalidEncoding("pairing response body is not canonical")
        }
        try body.validate(invitation: invitation)
        return body
    }
}

extension PairingResponseBody {
    func canonicalJSON() throws -> Data {
        try CanonicalValue.object([
            "proof": proof.canonicalValue(),
            "response_mailbox_id": .string(responseMailboxID),
            "response_mailbox_revoke_capability": .string(responseMailboxRevocationCapability),
            "response_mailbox_write_capability": .string(responseMailboxWriteCapability),
            "schema": .string(schema),
        ]).data()
    }
}

extension CompanionPairingGrantPackage {
    func canonicalJSON() throws -> Data {
        var grant: [String: CanonicalValue] = [
            "allowed_actions": .array(capabilityGrant.allowedActions.map { .string($0.rawValue) }),
            "capability_id": .string(capabilityGrant.capabilityID),
            "device_id": .string(capabilityGrant.deviceID),
            "issued_at": .string(capabilityGrant.issuedAt),
            "revocation_epoch": .unsigned(capabilityGrant.revocationEpoch),
            "schema": .string(capabilityGrant.schema),
            "signature": .string(capabilityGrant.signature),
            "studio_signing_public_key": .string(capabilityGrant.studioSigningPublicKey),
            "workspace_id": .string(capabilityGrant.workspaceID),
        ]
        if let expiresAt = capabilityGrant.expiresAt {
            grant["expires_at"] = .string(expiresAt)
        }
        return try CanonicalValue.object([
            "capability_grant": .object(grant),
            "command_mailbox_id": .string(commandMailboxID),
            "command_mailbox_write_capability": .string(commandMailboxWriteCapability),
            "schema": .string(schema),
            "studio_agreement_public_key": .string(studioAgreementPublicKey),
        ]).data()
    }
}

private extension PairingProof {
    func canonicalValue() -> CanonicalValue {
        .object([
            "companion_agreement_public_key": .string(companionAgreementPublicKey),
            "companion_device_id": .string(companionDeviceID),
            "companion_display_name": .string(companionDisplayName),
            "companion_signing_public_key": .string(companionSigningPublicKey),
            "created_at": .string(createdAt),
            "invitation_digest": .string(invitationDigest),
            "key_confirmation": .string(keyConfirmation),
            "schema": .string(schema), "session_id": .string(sessionID),
            "signature": .string(signature), "workspace_id": .string(workspaceID),
        ])
    }
}
