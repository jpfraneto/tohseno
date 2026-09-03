import CryptoKit
import Foundation

public enum WorkshopBase64URL {
    public static func encode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    public static func decode(_ value: String, expectedBytes: Int? = nil) throws -> Data {
        guard value.range(of: #"^[A-Za-z0-9_-]*$"#, options: .regularExpression) != nil else {
            throw WorkshopRuntimeError.invalidCredential
        }
        let remainder = value.count % 4
        guard remainder != 1 else { throw WorkshopRuntimeError.invalidCredential }
        let padded = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
            + String(repeating: "=", count: (4 - remainder) % 4)
        guard let data = Data(base64Encoded: padded), expectedBytes == nil || data.count == expectedBytes else {
            throw WorkshopRuntimeError.invalidCredential
        }
        return data
    }
}

public struct WorkshopHostCredential: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.workshop-host/1"
    public static let signatureDomain = "tohseno.workshop.host.v1"

    public let schema: String
    public let workspaceID: String
    public let studioDeviceID: WorkshopDeviceID
    public let sessionID: WorkshopSessionID
    public let challenge: String
    public let issuedAt: String
    public let expiresAt: String
    public let signature: String

    public init(
        schema: String = schemaV1,
        workspaceID: String,
        studioDeviceID: WorkshopDeviceID,
        sessionID: WorkshopSessionID,
        challenge: String,
        issuedAt: String,
        expiresAt: String,
        signature: String
    ) {
        self.schema = schema
        self.workspaceID = workspaceID
        self.studioDeviceID = studioDeviceID
        self.sessionID = sessionID
        self.challenge = challenge
        self.issuedAt = issuedAt
        self.expiresAt = expiresAt
        self.signature = signature
    }

    public func signingBody() -> Data {
        Self.joined([schema, workspaceID, studioDeviceID.rawValue, sessionID.rawValue, challenge, issuedAt, expiresAt])
    }

    public func digest() -> Data { Data(SHA256.hash(data: signingBody())) }

    public func verify(
        studioSigningPublicKey: Data,
        expectedWorkspaceID: String,
        expectedStudioDeviceID: WorkshopDeviceID,
        now: Date = Date()
    ) throws {
        guard schema == Self.schemaV1,
              workspaceID == expectedWorkspaceID,
              studioDeviceID == expectedStudioDeviceID,
              try WorkshopBase64URL.decode(challenge, expectedBytes: 32).count == 32
        else { throw WorkshopRuntimeError.invalidCredential }
        guard let issued = WorkshopTimestamp.parse(issuedAt),
              let expires = WorkshopTimestamp.parse(expiresAt),
              expires > issued,
              expires.timeIntervalSince(issued) <= 180,
              now >= issued.addingTimeInterval(-30),
              now <= expires.addingTimeInterval(30)
        else { throw WorkshopRuntimeError.expiredCredential }
        let signature = try WorkshopBase64URL.decode(signature, expectedBytes: 64)
        guard studioSigningPublicKey.count == 32,
              try Curve25519.Signing.PublicKey(rawRepresentation: studioSigningPublicKey)
                .isValidSignature(signature, for: Self.domainMessage(Self.signatureDomain, signingBody()))
        else { throw WorkshopRuntimeError.invalidCredential }
    }

    static func joined(_ values: [String]) -> Data {
        var bytes = Data()
        for (index, value) in values.enumerated() {
            if index > 0 { bytes.append(0) }
            bytes.append(contentsOf: value.utf8)
        }
        return bytes
    }

    public static func domainMessage(_ domain: String, _ message: Data) -> Data {
        var result = Data(domain.utf8)
        result.append(0)
        result.append(message)
        return result
    }
}

public struct WorkshopClientProof: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.workshop-client-proof/1"
    public static let signatureDomain = "tohseno.workshop.client-proof.v1"

    public let schema: String
    public let sessionID: WorkshopSessionID
    public let companionDeviceID: WorkshopDeviceID
    public let revocationEpoch: UInt64
    public let hostCredentialDigest: String
    public let clientNonce: String
    public let signature: String

    public init(
        schema: String = schemaV1,
        sessionID: WorkshopSessionID,
        companionDeviceID: WorkshopDeviceID,
        revocationEpoch: UInt64,
        hostCredentialDigest: String,
        clientNonce: String,
        signature: String
    ) {
        self.schema = schema
        self.sessionID = sessionID
        self.companionDeviceID = companionDeviceID
        self.revocationEpoch = revocationEpoch
        self.hostCredentialDigest = hostCredentialDigest
        self.clientNonce = clientNonce
        self.signature = signature
    }

    public func signingBody() -> Data {
        WorkshopHostCredential.joined([
            schema, sessionID.rawValue, companionDeviceID.rawValue, String(revocationEpoch),
            hostCredentialDigest, clientNonce,
        ])
    }

    public func verify(host: WorkshopHostCredential, companionSigningPublicKey: Data) throws {
        guard schema == Self.schemaV1,
              sessionID == host.sessionID,
              hostCredentialDigest == WorkshopBase64URL.encode(host.digest()),
              try WorkshopBase64URL.decode(clientNonce, expectedBytes: 32).count == 32,
              companionSigningPublicKey.count == 32
        else { throw WorkshopRuntimeError.invalidCredential }
        let signature = try WorkshopBase64URL.decode(signature, expectedBytes: 64)
        guard try Curve25519.Signing.PublicKey(rawRepresentation: companionSigningPublicKey)
            .isValidSignature(
                signature,
                for: WorkshopHostCredential.domainMessage(Self.signatureDomain, signingBody())
            )
        else { throw WorkshopRuntimeError.invalidCredential }
    }
}

public struct WorkshopTrustedPeer: Codable, Equatable, Sendable {
    public let deviceID: WorkshopDeviceID
    public let displayName: String
    public let signingPublicKey: String
    public let sessionKey: String
    public let revocationEpoch: UInt64

    public init(
        deviceID: WorkshopDeviceID,
        displayName: String,
        signingPublicKey: String,
        sessionKey: String,
        revocationEpoch: UInt64
    ) {
        self.deviceID = deviceID
        self.displayName = displayName
        self.signingPublicKey = signingPublicKey
        self.sessionKey = sessionKey
        self.revocationEpoch = revocationEpoch
    }
}

public struct WorkshopHostAuthorization: Codable, Equatable, Sendable {
    public let credential: WorkshopHostCredential
    public let peers: [WorkshopTrustedPeer]

    public init(credential: WorkshopHostCredential, peers: [WorkshopTrustedPeer]) {
        self.credential = credential
        self.peers = peers
    }
}

public struct WorkshopClientPairing: Equatable, Sendable {
    public let workspaceID: String
    public let studioDeviceID: WorkshopDeviceID
    public let studioSigningPublicKey: Data
    public let studioAgreementPublicKey: Data
    public let companionDeviceID: WorkshopDeviceID
    public let revocationEpoch: UInt64

    public init(
        workspaceID: String,
        studioDeviceID: WorkshopDeviceID,
        studioSigningPublicKey: Data,
        studioAgreementPublicKey: Data,
        companionDeviceID: WorkshopDeviceID,
        revocationEpoch: UInt64
    ) {
        self.workspaceID = workspaceID
        self.studioDeviceID = studioDeviceID
        self.studioSigningPublicKey = studioSigningPublicKey
        self.studioAgreementPublicKey = studioAgreementPublicKey
        self.companionDeviceID = companionDeviceID
        self.revocationEpoch = revocationEpoch
    }
}

public struct WorkshopClientAuthorization: Equatable, Sendable {
    public let proof: WorkshopClientProof
    public let sessionKey: Data

    public init(proof: WorkshopClientProof, sessionKey: Data) {
        self.proof = proof
        self.sessionKey = sessionKey
    }
}

public enum WorkshopHandshake {
    /// Authenticates a client proof against the exact active pairing snapshot
    /// supplied by the Mac's durable workspace service.
    public static func authenticate(
        proof: WorkshopClientProof,
        host: WorkshopHostCredential,
        peers: [WorkshopTrustedPeer]
    ) throws -> WorkshopTrustedPeer {
        guard let peer = peers.first(where: { $0.deviceID == proof.companionDeviceID }) else {
            throw WorkshopRuntimeError.unpairedDevice
        }
        guard peer.revocationEpoch == proof.revocationEpoch else {
            throw WorkshopRuntimeError.revokedDevice
        }
        try proof.verify(
            host: host,
            companionSigningPublicKey: WorkshopBase64URL.decode(
                peer.signingPublicKey,
                expectedBytes: 32
            )
        )
        _ = try WorkshopBase64URL.decode(peer.sessionKey, expectedBytes: 32)
        return peer
    }
}

public protocol WorkshopHostAuthorizing: Sendable {
    func authorizeWorkshopHost(sessionID: WorkshopSessionID, challenge: Data) async throws
        -> WorkshopHostAuthorization
}

public protocol WorkshopClientAuthorizing: Sendable {
    func workshopPairing() async throws -> WorkshopClientPairing
    func authorizeWorkshopClient(host: WorkshopHostCredential, clientNonce: Data) async throws
        -> WorkshopClientAuthorization
}

public enum WorkshopSessionCrypto {
    public static func deriveSessionKey(
        sharedSecret: SharedSecret,
        challenge: Data,
        sessionID: WorkshopSessionID,
        workspaceID: String,
        companionDeviceID: WorkshopDeviceID,
        revocationEpoch: UInt64
    ) -> Data {
        deriveSessionKey(
            sharedSecretBytes: sharedSecret.withUnsafeBytes { Data($0) },
            challenge: challenge,
            sessionID: sessionID,
            workspaceID: workspaceID,
            companionDeviceID: companionDeviceID,
            revocationEpoch: revocationEpoch
        )
    }

    static func deriveSessionKey(
        sharedSecretBytes: Data,
        challenge: Data,
        sessionID: WorkshopSessionID,
        workspaceID: String,
        companionDeviceID: WorkshopDeviceID,
        revocationEpoch: UInt64
    ) -> Data {
        let info = WorkshopHostCredential.joined([
            "tohseno.workshop.session-key.v1",
            sessionID.rawValue,
            workspaceID,
            companionDeviceID.rawValue,
            String(revocationEpoch),
        ])
        let key = HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: sharedSecretBytes),
            salt: challenge,
            info: info,
            outputByteCount: 32
        )
        return key.withUnsafeBytes { Data($0) }
    }

    public static func deriveDirectionalKey(sessionKey: Data, direction: String) -> SymmetricKey {
        HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: sessionKey),
            salt: Data("tohseno.workshop.traffic.v1".utf8),
            info: Data(direction.utf8),
            outputByteCount: 32
        )
    }

    public static func seal(
        _ envelope: WorkshopEnvelope,
        sessionKey: Data,
        direction: String
    ) throws -> Data {
        try envelope.validate()
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let plaintext = try encoder.encode(envelope)
        let nonce = try ChaChaPoly.Nonce(data: nonce(sequence: envelope.sequence))
        let key = deriveDirectionalKey(sessionKey: sessionKey, direction: direction)
        return try ChaChaPoly.seal(
            plaintext,
            using: key,
            nonce: nonce,
            authenticating: Data(direction.utf8)
        ).combined
    }

    public static func open(
        _ combined: Data,
        sessionKey: Data,
        direction: String,
        expectedSessionID: WorkshopSessionID,
        afterSequence: UInt64
    ) throws -> WorkshopEnvelope {
        guard combined.count <= WorkshopEvent.maximumPayloadBytes + 64 * 1024 else {
            throw WorkshopRuntimeError.unsupportedEnvelope
        }
        let key = deriveDirectionalKey(sessionKey: sessionKey, direction: direction)
        let box = try ChaChaPoly.SealedBox(combined: combined)
        let plaintext = try ChaChaPoly.open(box, using: key, authenticating: Data(direction.utf8))
        let envelope = try JSONDecoder().decode(WorkshopEnvelope.self, from: plaintext)
        try envelope.validate(expectedSessionID: expectedSessionID)
        guard envelope.sequence > afterSequence else { throw WorkshopRuntimeError.replayedEnvelope }
        return envelope
    }

    private static func nonce(sequence: UInt64) -> Data {
        var bytes = Data(repeating: 0, count: 4)
        var bigEndian = sequence.bigEndian
        withUnsafeBytes(of: &bigEndian) { bytes.append(contentsOf: $0) }
        return bytes
    }
}

public enum WorkshopTimestamp {
    public static func parse(_ value: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.date(from: value)
    }

    public static func format(_ value: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: value)
    }
}
