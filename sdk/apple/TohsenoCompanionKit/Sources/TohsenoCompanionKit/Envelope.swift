import CryptoKit
import Foundation

public struct CompanionEnvelopeMetadata: Equatable, Sendable {
    public let envelopeID: String
    public let mailboxID: String
    public let recipientDeviceID: String
    public let senderSequence: UInt64
    public let createdAt: String
    public let expiresAt: String

    public init(
        envelopeID: String,
        mailboxID: String,
        recipientDeviceID: String,
        senderSequence: UInt64,
        createdAt: String,
        expiresAt: String
    ) {
        self.envelopeID = envelopeID
        self.mailboxID = mailboxID
        self.recipientDeviceID = recipientDeviceID
        self.senderSequence = senderSequence
        self.createdAt = createdAt
        self.expiresAt = expiresAt
    }
}

public struct OpaqueCompanionEnvelope: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-envelope/1"
    static let signatureDomain = "tohseno.companion.envelope-signature.v1"
    static let keyDomain = "tohseno.companion.envelope-key.v1"

    public let schema: String
    public let envelopeID: String
    public let mailboxID: String
    public let senderDeviceID: String
    public let recipientDeviceID: String
    public let senderSequence: UInt64
    public let createdAt: String
    public let expiresAt: String
    public let ephemeralPublicKey: String
    public let nonce: String
    public let ciphertext: String
    public let signature: String

    enum CodingKeys: String, CodingKey {
        case schema
        case envelopeID = "envelope_id"
        case mailboxID = "mailbox_id"
        case senderDeviceID = "sender_device_id"
        case recipientDeviceID = "recipient_device_id"
        case senderSequence = "sender_sequence"
        case createdAt = "created_at"
        case expiresAt = "expires_at"
        case ephemeralPublicKey = "ephemeral_public_key"
        case nonce
        case ciphertext
        case signature
    }

    public init(
        schema: String = schemaV1,
        envelopeID: String,
        mailboxID: String,
        senderDeviceID: String,
        recipientDeviceID: String,
        senderSequence: UInt64,
        createdAt: String,
        expiresAt: String,
        ephemeralPublicKey: String,
        nonce: String,
        ciphertext: String,
        signature: String
    ) {
        self.schema = schema
        self.envelopeID = envelopeID
        self.mailboxID = mailboxID
        self.senderDeviceID = senderDeviceID
        self.recipientDeviceID = recipientDeviceID
        self.senderSequence = senderSequence
        self.createdAt = createdAt
        self.expiresAt = expiresAt
        self.ephemeralPublicKey = ephemeralPublicKey
        self.nonce = nonce
        self.ciphertext = ciphertext
        self.signature = signature
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "envelope_id", "mailbox_id", "sender_device_id",
            "recipient_device_id", "sender_sequence", "created_at", "expires_at",
            "ephemeral_public_key", "nonce", "ciphertext", "signature",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        envelopeID = try container.decode(String.self, forKey: .envelopeID)
        mailboxID = try container.decode(String.self, forKey: .mailboxID)
        senderDeviceID = try container.decode(String.self, forKey: .senderDeviceID)
        recipientDeviceID = try container.decode(String.self, forKey: .recipientDeviceID)
        senderSequence = try container.decode(UInt64.self, forKey: .senderSequence)
        createdAt = try container.decode(String.self, forKey: .createdAt)
        expiresAt = try container.decode(String.self, forKey: .expiresAt)
        ephemeralPublicKey = try container.decode(String.self, forKey: .ephemeralPublicKey)
        nonce = try container.decode(String.self, forKey: .nonce)
        ciphertext = try container.decode(String.self, forKey: .ciphertext)
        signature = try container.decode(String.self, forKey: .signature)
    }

    func validateShape() throws {
        guard schema == Self.schemaV1 else {
            throw TohsenoCompanionError.invalidEnvelope("unsupported schema")
        }
        try requireIdentifier(envelopeID, field: "envelope_id")
        try requireIdentifier(mailboxID, field: "mailbox_id")
        try requireIdentifier(senderDeviceID, field: "sender_device_id")
        try requireIdentifier(recipientDeviceID, field: "recipient_device_id")
        guard senderSequence > 0 else {
            throw TohsenoCompanionError.invalidEnvelope("sender sequence must be positive")
        }
        _ = try CompanionTimestamp.parse(createdAt)
        _ = try CompanionTimestamp.parse(expiresAt)
        _ = try Base64URL.decode(ephemeralPublicKey, expectedBytes: 32)
        _ = try Base64URL.decode(nonce, expectedBytes: 12)
    }

    func canonicalHeader() throws -> Data {
        try CanonicalValue.object([
            "created_at": .string(createdAt),
            "envelope_id": .string(envelopeID),
            "ephemeral_public_key": .string(ephemeralPublicKey),
            "expires_at": .string(expiresAt),
            "mailbox_id": .string(mailboxID),
            "nonce": .string(nonce),
            "recipient_device_id": .string(recipientDeviceID),
            "schema": .string(schema),
            "sender_device_id": .string(senderDeviceID),
            "sender_sequence": .unsigned(senderSequence),
        ]).data()
    }

    func canonicalUnsigned() throws -> Data {
        try CanonicalValue.object([
            "ciphertext": .string(ciphertext),
            "created_at": .string(createdAt),
            "envelope_id": .string(envelopeID),
            "ephemeral_public_key": .string(ephemeralPublicKey),
            "expires_at": .string(expiresAt),
            "mailbox_id": .string(mailboxID),
            "nonce": .string(nonce),
            "recipient_device_id": .string(recipientDeviceID),
            "schema": .string(schema),
            "sender_device_id": .string(senderDeviceID),
            "sender_sequence": .unsigned(senderSequence),
        ]).data()
    }
}

public actor CompanionReplayProtection {
    public struct State: Codable, Equatable, Sendable {
        public var senders: [String: Sender]

        public init(senders: [String: Sender] = [:]) { self.senders = senders }
    }

    public struct Sender: Codable, Equatable, Sendable {
        public var floor: UInt64
        public var observations: [UInt64: String]

        public init(floor: UInt64 = 0, observations: [UInt64: String] = [:]) {
            self.floor = floor
            self.observations = observations
        }
    }

    private let capacity: UInt64
    private var state: State

    public init(capacity: UInt64 = 4096, state: State = State()) throws {
        guard (1 ... 65_536).contains(capacity) else {
            throw TohsenoCompanionError.invalidEnvelope("invalid replay window capacity")
        }
        self.capacity = capacity
        self.state = state
    }

    public func exportState() -> State { state }

    public func observe(sender: String, sequence: UInt64, envelopeID: String) throws {
        guard sequence > 0 else { throw TohsenoCompanionError.replayDetected }
        var senderState = state.senders[sender] ?? Sender()
        if sequence <= senderState.floor { throw TohsenoCompanionError.replayDetected }
        if let known = senderState.observations[sequence] {
            guard known == envelopeID else { throw TohsenoCompanionError.replayDetected }
            throw TohsenoCompanionError.replayDetected
        }
        senderState.observations[sequence] = envelopeID
        if let maximum = senderState.observations.keys.max(), maximum > capacity {
            let newFloor = maximum - capacity
            senderState.floor = max(senderState.floor, newFloor)
            senderState.observations = senderState.observations.filter { $0.key > senderState.floor }
        }
        state.senders[sender] = senderState
    }
}

enum CompanionEnvelopeCrypto {
    static func seal(
        sender: CompanionIdentity,
        recipientAgreementPublicKey: Data,
        metadata: CompanionEnvelopeMetadata,
        plaintext: Data,
        entropySource: any CompanionEntropySource = SystemCompanionEntropySource()
    ) throws -> OpaqueCompanionEnvelope {
        try seal(
            sender: sender,
            recipientAgreementPublicKey: recipientAgreementPublicKey,
            metadata: metadata,
            plaintext: plaintext,
            ephemeralSecret: entropySource.randomBytes(count: 32),
            nonce: entropySource.randomBytes(count: 12)
        )
    }

    static func seal(
        sender: CompanionIdentity,
        recipientAgreementPublicKey: Data,
        metadata: CompanionEnvelopeMetadata,
        plaintext: Data,
        ephemeralSecret: Data,
        nonce: Data
    ) throws -> OpaqueCompanionEnvelope {
        guard !plaintext.isEmpty, plaintext.count <= 16 * 1024 * 1024,
              recipientAgreementPublicKey.count == 32,
              ephemeralSecret.count == 32, nonce.count == 12
        else { throw TohsenoCompanionError.invalidEnvelope("invalid envelope material or size") }
        let ephemeral = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: ephemeralSecret)
        let draft = OpaqueCompanionEnvelope(
            envelopeID: metadata.envelopeID,
            mailboxID: metadata.mailboxID,
            senderDeviceID: sender.description.deviceID,
            recipientDeviceID: metadata.recipientDeviceID,
            senderSequence: metadata.senderSequence,
            createdAt: metadata.createdAt,
            expiresAt: metadata.expiresAt,
            ephemeralPublicKey: Base64URL.encode(ephemeral.publicKey.rawRepresentation),
            nonce: Base64URL.encode(nonce),
            ciphertext: "pending",
            signature: "pending"
        )
        try draft.validateShape()
        let aad = try draft.canonicalHeader()
        let recipient = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: recipientAgreementPublicKey)
        let shared = try ephemeral.sharedSecretFromKeyAgreement(with: recipient)
        let key = shared.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: aad.companionSHA256,
            sharedInfo: Data(OpaqueCompanionEnvelope.keyDomain.utf8),
            outputByteCount: 32
        )
        let sealed = try ChaChaPoly.seal(
            plaintext,
            using: key,
            nonce: try ChaChaPoly.Nonce(data: nonce),
            authenticating: aad
        )
        var encrypted = sealed.ciphertext
        encrypted.append(sealed.tag)
        let unsigned = OpaqueCompanionEnvelope(
            envelopeID: draft.envelopeID,
            mailboxID: draft.mailboxID,
            senderDeviceID: draft.senderDeviceID,
            recipientDeviceID: draft.recipientDeviceID,
            senderSequence: draft.senderSequence,
            createdAt: draft.createdAt,
            expiresAt: draft.expiresAt,
            ephemeralPublicKey: draft.ephemeralPublicKey,
            nonce: draft.nonce,
            ciphertext: Base64URL.encode(encrypted),
            signature: "pending"
        )
        return OpaqueCompanionEnvelope(
            envelopeID: unsigned.envelopeID,
            mailboxID: unsigned.mailboxID,
            senderDeviceID: unsigned.senderDeviceID,
            recipientDeviceID: unsigned.recipientDeviceID,
            senderSequence: unsigned.senderSequence,
            createdAt: unsigned.createdAt,
            expiresAt: unsigned.expiresAt,
            ephemeralPublicKey: unsigned.ephemeralPublicKey,
            nonce: unsigned.nonce,
            ciphertext: unsigned.ciphertext,
            signature: Base64URL.encode(try sender.sign(
                domain: OpaqueCompanionEnvelope.signatureDomain,
                message: unsigned.canonicalUnsigned()
            ))
        )
    }

    static func open(
        _ envelope: OpaqueCompanionEnvelope,
        expectedSenderSigningPublicKey: Data,
        expectedSenderDeviceID: String,
        expectedMailboxID: String? = nil,
        recipient: CompanionIdentity,
        now: Date = Date(),
        replay: CompanionReplayProtection
    ) async throws -> Data {
        try envelope.validateShape()
        guard envelope.senderDeviceID == expectedSenderDeviceID else {
            throw TohsenoCompanionError.invalidEnvelope("sender device differs")
        }
        if let expectedMailboxID, envelope.mailboxID != expectedMailboxID {
            throw TohsenoCompanionError.invalidEnvelope("mailbox differs")
        }
        guard envelope.recipientDeviceID == recipient.description.deviceID else {
            throw TohsenoCompanionError.invalidEnvelope("recipient device differs")
        }
        let created = try CompanionTimestamp.parse(envelope.createdAt)
        let expires = try CompanionTimestamp.parse(envelope.expiresAt)
        guard expires > created,
              expires.timeIntervalSince(created) <= 7 * 24 * 60 * 60,
              now >= created.addingTimeInterval(-30)
        else { throw TohsenoCompanionError.invalidEnvelope("invalid lifetime") }
        guard now <= expires.addingTimeInterval(30) else {
            throw TohsenoCompanionError.envelopeExpired
        }
        let ciphertextAndTag = try Base64URL.decode(envelope.ciphertext)
        guard ciphertextAndTag.count >= 16,
              ciphertextAndTag.count <= CompanionLimits.maximumEnvelopeCiphertextBytes
        else { throw TohsenoCompanionError.invalidEnvelope("ciphertext bound") }
        let signature = try Base64URL.decode(envelope.signature, expectedBytes: 64)
        guard try CompanionIdentity.verify(
            publicKey: expectedSenderSigningPublicKey,
            domain: OpaqueCompanionEnvelope.signatureDomain,
            message: envelope.canonicalUnsigned(),
            signature: signature
        ) else { throw TohsenoCompanionError.invalidEnvelope("signature verification failed") }
        let ephemeral = try Curve25519.KeyAgreement.PublicKey(
            rawRepresentation: Base64URL.decode(envelope.ephemeralPublicKey, expectedBytes: 32)
        )
        let nonceData = try Base64URL.decode(envelope.nonce, expectedBytes: 12)
        let aad = try envelope.canonicalHeader()
        let shared = try recipient.agreementKey.sharedSecretFromKeyAgreement(with: ephemeral)
        let key = shared.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: aad.companionSHA256,
            sharedInfo: Data(OpaqueCompanionEnvelope.keyDomain.utf8),
            outputByteCount: 32
        )
        let split = ciphertextAndTag.count - 16
        let sealed = try ChaChaPoly.SealedBox(
            nonce: ChaChaPoly.Nonce(data: nonceData),
            ciphertext: ciphertextAndTag.prefix(split),
            tag: ciphertextAndTag.suffix(16)
        )
        let plaintext: Data
        do {
            plaintext = try ChaChaPoly.open(sealed, using: key, authenticating: aad)
        } catch {
            throw TohsenoCompanionError.invalidEnvelope("authentication failed")
        }
        try await replay.observe(
            sender: envelope.senderDeviceID,
            sequence: envelope.senderSequence,
            envelopeID: envelope.envelopeID
        )
        return plaintext
    }
}
