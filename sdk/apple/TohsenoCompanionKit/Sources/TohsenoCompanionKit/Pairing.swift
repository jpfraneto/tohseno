import CryptoKit
import Foundation

public enum CompanionCapability: String, Codable, CaseIterable, Comparable, Sendable {
    case workspaceRead = "workspace.read"
    case executionRead = "execution.read"
    case feedbackWrite = "feedback.write"
    case marketingWrite = "marketing.write"
    case shotCreate = "shot.create"
    case shotEvolve = "shot.evolve"
    case publicationAuthorize = "publication.authorize"
    case networkReceive = "network.receive"
    case preferenceWrite = "preference.write"

    public static func < (lhs: Self, rhs: Self) -> Bool {
        let order: [Self] = [
            .workspaceRead, .executionRead, .feedbackWrite,
            .marketingWrite, .shotCreate, .shotEvolve, .publicationAuthorize, .networkReceive,
            .preferenceWrite,
        ]
        return order.firstIndex(of: lhs)! < order.firstIndex(of: rhs)!
    }
}

public struct RelayEndpoint: Equatable, Sendable {
    public let id: String
    public let baseURL: URL

    public init(
        id: String,
        baseURL: URL,
        allowLoopbackHTTP: Bool = false,
        allowLocalNetworkHTTP: Bool = false
    ) throws {
        try requireIdentifier(id, field: "relay_id")
        guard baseURL.user == nil, baseURL.password == nil,
              baseURL.query == nil, baseURL.fragment == nil,
              baseURL.path.isEmpty || baseURL.path == "/",
              baseURL.host != nil
        else { throw TohsenoCompanionError.relayNotAllowed }
        let isHTTPS = baseURL.scheme == "https"
        let isLoopback = allowLoopbackHTTP
            && baseURL.scheme == "http"
            && ["127.0.0.1", "::1", "localhost"].contains(baseURL.host!)
        let isLocalNetwork = allowLocalNetworkHTTP
            && baseURL.scheme == "http"
            && (
                baseURL.host!.lowercased().hasSuffix(".local")
                    || isPrivateIPv4Address(baseURL.host!)
            )
        guard isHTTPS || isLoopback || isLocalNetwork else {
            throw TohsenoCompanionError.relayNotAllowed
        }
        self.id = id
        self.baseURL = baseURL
    }
}

private func isPrivateIPv4Address(_ host: String) -> Bool {
    let fields = host.split(separator: ".", omittingEmptySubsequences: false)
    guard fields.count == 4 else { return false }
    let octets = fields.compactMap { UInt8($0) }
    guard octets.count == fields.count else { return false }
    return octets[0] == 10
        || (octets[0] == 172 && (16 ... 31).contains(octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 169 && octets[1] == 254)
}

public struct RelayAllowlist: Sendable {
    private let endpoints: [String: RelayEndpoint]

    public init(_ endpoints: [RelayEndpoint]) throws {
        guard !endpoints.isEmpty,
              Dictionary(grouping: endpoints, by: \.id).values.allSatisfy({ $0.count == 1 })
        else { throw TohsenoCompanionError.relayNotAllowed }
        self.endpoints = Dictionary(uniqueKeysWithValues: endpoints.map { ($0.id, $0) })
    }

    public func endpoint(for id: String) throws -> RelayEndpoint {
        guard let endpoint = endpoints[id] else { throw TohsenoCompanionError.relayNotAllowed }
        return endpoint
    }
}

public struct PairingInvitation: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-pairing-invitation/1"
    public static let uriPrefix = "tohseno://pair/v1/"
    static let signatureDomain = "tohseno.companion.pairing-invitation.v1"

    public let schema: String
    public let sessionID: String
    public let workspaceID: String
    public let studioDeviceID: String
    public let studioSigningPublicKey: String
    public let studioEphemeralAgreementPublicKey: String
    public let relayID: String
    public let issuedAt: String
    public let expiresAt: String
    public let signature: String

    public init(
        schema: String = schemaV1,
        sessionID: String,
        workspaceID: String,
        studioDeviceID: String,
        studioSigningPublicKey: String,
        studioEphemeralAgreementPublicKey: String,
        relayID: String,
        issuedAt: String,
        expiresAt: String,
        signature: String
    ) {
        self.schema = schema
        self.sessionID = sessionID
        self.workspaceID = workspaceID
        self.studioDeviceID = studioDeviceID
        self.studioSigningPublicKey = studioSigningPublicKey
        self.studioEphemeralAgreementPublicKey = studioEphemeralAgreementPublicKey
        self.relayID = relayID
        self.issuedAt = issuedAt
        self.expiresAt = expiresAt
        self.signature = signature
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "session_id", "workspace_id", "studio_device_id",
            "studio_signing_public_key", "studio_ephemeral_agreement_public_key",
            "relay_id", "issued_at", "expires_at", "signature",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        sessionID = try container.decode(String.self, forKey: .sessionID)
        workspaceID = try container.decode(String.self, forKey: .workspaceID)
        studioDeviceID = try container.decode(String.self, forKey: .studioDeviceID)
        studioSigningPublicKey = try container.decode(String.self, forKey: .studioSigningPublicKey)
        studioEphemeralAgreementPublicKey = try container.decode(
            String.self,
            forKey: .studioEphemeralAgreementPublicKey
        )
        relayID = try container.decode(String.self, forKey: .relayID)
        issuedAt = try container.decode(String.self, forKey: .issuedAt)
        expiresAt = try container.decode(String.self, forKey: .expiresAt)
        signature = try container.decode(String.self, forKey: .signature)
    }

    enum CodingKeys: String, CodingKey {
        case schema
        case sessionID = "session_id"
        case workspaceID = "workspace_id"
        case studioDeviceID = "studio_device_id"
        case studioSigningPublicKey = "studio_signing_public_key"
        case studioEphemeralAgreementPublicKey = "studio_ephemeral_agreement_public_key"
        case relayID = "relay_id"
        case issuedAt = "issued_at"
        case expiresAt = "expires_at"
        case signature
    }

    public static func parse(
        uri: String,
        allowlist: RelayAllowlist,
        now: Date = Date(),
        trustedStudioSigningKey: Data? = nil
    ) throws -> (PairingInvitation, RelayEndpoint) {
        guard uri.utf8.count <= 32 * 1024, uri.hasPrefix(uriPrefix) else {
            throw TohsenoCompanionError.invalidInvitation("unsupported pairing URI")
        }
        let payload = String(uri.dropFirst(uriPrefix.count))
        let bytes = try Base64URL.decode(payload)
        guard bytes.count <= 16 * 1024 else { throw TohsenoCompanionError.responseTooLarge }
        let invitation = try StrictJSON.decode(Self.self, from: bytes, maximumBytes: 16 * 1024)
        guard try invitation.canonicalJSON() == bytes else {
            throw TohsenoCompanionError.invalidInvitation("URI JSON is not canonical")
        }
        let endpoint = try allowlist.endpoint(for: invitation.relayID)
        try invitation.verify(now: now, trustedStudioSigningKey: trustedStudioSigningKey)
        return (invitation, endpoint)
    }

    public func verify(now: Date, trustedStudioSigningKey: Data? = nil) throws {
        guard schema == Self.schemaV1 else {
            throw TohsenoCompanionError.invalidInvitation("unsupported schema")
        }
        try requireIdentifier(sessionID, field: "session_id")
        try requireIdentifier(workspaceID, field: "workspace_id")
        try requireIdentifier(studioDeviceID, field: "studio_device_id")
        try requireIdentifier(relayID, field: "relay_id")
        let embeddedKey = try Base64URL.decode(studioSigningPublicKey, expectedBytes: 32)
        _ = try Base64URL.decode(studioEphemeralAgreementPublicKey, expectedBytes: 32)
        if let trustedStudioSigningKey, trustedStudioSigningKey != embeddedKey {
            throw TohsenoCompanionError.invalidInvitation("Studio signing key is not trusted")
        }
        let issued = try CompanionTimestamp.parse(issuedAt)
        let expires = try CompanionTimestamp.parse(expiresAt)
        guard expires.timeIntervalSince(issued) > 0,
              expires.timeIntervalSince(issued) <= 120 else {
            throw TohsenoCompanionError.invalidInvitation("lifetime exceeds two minutes")
        }
        if now < issued.addingTimeInterval(-30) { throw TohsenoCompanionError.invitationNotYetValid }
        if now > expires.addingTimeInterval(30) { throw TohsenoCompanionError.invitationExpired }
        let signatureBytes = try Base64URL.decode(signature, expectedBytes: 64)
        guard try CompanionIdentity.verify(
            publicKey: embeddedKey,
            domain: Self.signatureDomain,
            message: canonicalBody(),
            signature: signatureBytes
        ) else {
            throw TohsenoCompanionError.invalidInvitation("signature verification failed")
        }
    }

    public func canonicalBody() throws -> Data {
        try CanonicalValue.object([
            "expires_at": .string(expiresAt),
            "issued_at": .string(issuedAt),
            "relay_id": .string(relayID),
            "schema": .string(schema),
            "session_id": .string(sessionID),
            "studio_device_id": .string(studioDeviceID),
            "studio_ephemeral_agreement_public_key": .string(studioEphemeralAgreementPublicKey),
            "studio_signing_public_key": .string(studioSigningPublicKey),
            "workspace_id": .string(workspaceID),
        ]).data()
    }

    public func canonicalJSON() throws -> Data {
        try CanonicalValue.object([
            "expires_at": .string(expiresAt),
            "issued_at": .string(issuedAt),
            "relay_id": .string(relayID),
            "schema": .string(schema),
            "session_id": .string(sessionID),
            "signature": .string(signature),
            "studio_device_id": .string(studioDeviceID),
            "studio_ephemeral_agreement_public_key": .string(studioEphemeralAgreementPublicKey),
            "studio_signing_public_key": .string(studioSigningPublicKey),
            "workspace_id": .string(workspaceID),
        ]).data()
    }

    public func digest() throws -> Data { try canonicalJSON().companionSHA256 }
}

public struct PairingProof: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-pairing-proof/1"
    static let signatureDomain = "tohseno.companion.pairing-proof.v1"
    static let confirmationDomain = "tohseno.companion.pairing-confirmation.v1"

    public let schema: String
    public let sessionID: String
    public let workspaceID: String
    public let invitationDigest: String
    public let companionDeviceID: String
    public let companionDisplayName: String
    public let companionSigningPublicKey: String
    public let companionAgreementPublicKey: String
    public let createdAt: String
    public let keyConfirmation: String
    public let signature: String

    enum CodingKeys: String, CodingKey {
        case schema
        case sessionID = "session_id"
        case workspaceID = "workspace_id"
        case invitationDigest = "invitation_digest"
        case companionDeviceID = "companion_device_id"
        case companionDisplayName = "companion_display_name"
        case companionSigningPublicKey = "companion_signing_public_key"
        case companionAgreementPublicKey = "companion_agreement_public_key"
        case createdAt = "created_at"
        case keyConfirmation = "key_confirmation"
        case signature
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "session_id", "workspace_id", "invitation_digest",
            "companion_device_id", "companion_display_name", "companion_signing_public_key",
            "companion_agreement_public_key", "created_at", "key_confirmation", "signature",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        sessionID = try container.decode(String.self, forKey: .sessionID)
        workspaceID = try container.decode(String.self, forKey: .workspaceID)
        invitationDigest = try container.decode(String.self, forKey: .invitationDigest)
        companionDeviceID = try container.decode(String.self, forKey: .companionDeviceID)
        companionDisplayName = try container.decode(String.self, forKey: .companionDisplayName)
        companionSigningPublicKey = try container.decode(String.self, forKey: .companionSigningPublicKey)
        companionAgreementPublicKey = try container.decode(String.self, forKey: .companionAgreementPublicKey)
        createdAt = try container.decode(String.self, forKey: .createdAt)
        keyConfirmation = try container.decode(String.self, forKey: .keyConfirmation)
        signature = try container.decode(String.self, forKey: .signature)
    }

    static func create(
        invitation: PairingInvitation,
        identity: CompanionIdentity,
        displayName: String,
        createdAt: String
    ) throws -> PairingProof {
        try requireBoundedText(
            displayName,
            field: "companion_display_name",
            maximum: CompanionLimits.maximumDeviceNameBytes
        )
        _ = try CompanionTimestamp.parse(createdAt)
        let unsigned = PairingProof(
            schema: schemaV1,
            sessionID: invitation.sessionID,
            workspaceID: invitation.workspaceID,
            invitationDigest: Base64URL.encode(try invitation.digest()),
            companionDeviceID: identity.description.deviceID,
            companionDisplayName: displayName,
            companionSigningPublicKey: identity.description.signingPublicKey,
            companionAgreementPublicKey: identity.description.agreementPublicKey,
            createdAt: createdAt,
            keyConfirmation: "pending",
            signature: "pending"
        )
        let body = try unsigned.canonicalBody()
        let studioAgreement = try Curve25519.KeyAgreement.PublicKey(
            rawRepresentation: Base64URL.decode(
                invitation.studioEphemeralAgreementPublicKey,
                expectedBytes: 32
            )
        )
        let shared = try identity.agreementKey.sharedSecretFromKeyAgreement(with: studioAgreement)
        let confirmationKey = shared.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: try invitation.digest(),
            sharedInfo: Data(confirmationDomain.utf8),
            outputByteCount: 32
        )
        var confirmationBytes = Data(confirmationDomain.utf8)
        confirmationBytes.append(0)
        confirmationBytes.append(body)
        let confirmation = Data(HMAC<SHA256>.authenticationCode(
            for: confirmationBytes,
            using: confirmationKey
        ))
        return PairingProof(
            schema: unsigned.schema,
            sessionID: unsigned.sessionID,
            workspaceID: unsigned.workspaceID,
            invitationDigest: unsigned.invitationDigest,
            companionDeviceID: unsigned.companionDeviceID,
            companionDisplayName: unsigned.companionDisplayName,
            companionSigningPublicKey: unsigned.companionSigningPublicKey,
            companionAgreementPublicKey: unsigned.companionAgreementPublicKey,
            createdAt: unsigned.createdAt,
            keyConfirmation: Base64URL.encode(confirmation),
            signature: Base64URL.encode(try identity.sign(domain: signatureDomain, message: body))
        )
    }

    init(
        schema: String,
        sessionID: String,
        workspaceID: String,
        invitationDigest: String,
        companionDeviceID: String,
        companionDisplayName: String,
        companionSigningPublicKey: String,
        companionAgreementPublicKey: String,
        createdAt: String,
        keyConfirmation: String,
        signature: String
    ) {
        self.schema = schema
        self.sessionID = sessionID
        self.workspaceID = workspaceID
        self.invitationDigest = invitationDigest
        self.companionDeviceID = companionDeviceID
        self.companionDisplayName = companionDisplayName
        self.companionSigningPublicKey = companionSigningPublicKey
        self.companionAgreementPublicKey = companionAgreementPublicKey
        self.createdAt = createdAt
        self.keyConfirmation = keyConfirmation
        self.signature = signature
    }

    public func canonicalBody() throws -> Data {
        try CanonicalValue.object([
            "companion_agreement_public_key": .string(companionAgreementPublicKey),
            "companion_device_id": .string(companionDeviceID),
            "companion_display_name": .string(companionDisplayName),
            "companion_signing_public_key": .string(companionSigningPublicKey),
            "created_at": .string(createdAt),
            "invitation_digest": .string(invitationDigest),
            "schema": .string(schema),
            "session_id": .string(sessionID),
            "workspace_id": .string(workspaceID),
        ]).data()
    }

    func verify(
        invitation: PairingInvitation,
        studioEphemeralPrivateKey: Data,
        now: Date
    ) throws {
        guard schema == Self.schemaV1, sessionID == invitation.sessionID,
              workspaceID == invitation.workspaceID,
              invitationDigest == Base64URL.encode(try invitation.digest())
        else { throw TohsenoCompanionError.invalidInvitation("pairing proof binding differs") }
        try requireIdentifier(companionDeviceID, field: "companion_device_id")
        try requireBoundedText(
            companionDisplayName,
            field: "companion_display_name",
            maximum: CompanionLimits.maximumDeviceNameBytes
        )
        let signing = try Base64URL.decode(companionSigningPublicKey, expectedBytes: 32)
        let agreement = try Base64URL.decode(companionAgreementPublicKey, expectedBytes: 32)
        guard CompanionIdentity.deviceID(
            signingPublicKey: signing,
            agreementPublicKey: agreement
        ) == companionDeviceID else {
            throw TohsenoCompanionError.invalidInvitation("pairing proof device binding differs")
        }
        let created = try CompanionTimestamp.parse(createdAt)
        let issued = try CompanionTimestamp.parse(invitation.issuedAt)
        let expires = try CompanionTimestamp.parse(invitation.expiresAt)
        guard created >= issued.addingTimeInterval(-30),
              created <= expires.addingTimeInterval(30),
              now <= expires.addingTimeInterval(30)
        else { throw TohsenoCompanionError.invitationExpired }
        let body = try canonicalBody()
        guard try CompanionIdentity.verify(
            publicKey: signing,
            domain: Self.signatureDomain,
            message: body,
            signature: Base64URL.decode(signature, expectedBytes: 64)
        ) else { throw TohsenoCompanionError.invalidInvitation("pairing proof signature failed") }
        let studio = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: studioEphemeralPrivateKey)
        let phone = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: agreement)
        let shared = try studio.sharedSecretFromKeyAgreement(with: phone)
        let key = shared.hkdfDerivedSymmetricKey(
            using: SHA256.self,
            salt: try invitation.digest(),
            sharedInfo: Data(Self.confirmationDomain.utf8),
            outputByteCount: 32
        )
        var confirmationBytes = Data(Self.confirmationDomain.utf8)
        confirmationBytes.append(0)
        confirmationBytes.append(body)
        guard HMAC<SHA256>.isValidAuthenticationCode(
            try Base64URL.decode(keyConfirmation, expectedBytes: 32),
            authenticating: confirmationBytes,
            using: key
        ) else { throw TohsenoCompanionError.invalidInvitation("pairing key confirmation failed") }
    }
}

enum CompanionTimestamp {
    static func parse(_ value: String) throws -> Date {
        let bytes = Array(value.utf8)
        let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
        guard bytes.count == 20,
              digits.allSatisfy({ (0x30 ... 0x39).contains(bytes[$0]) }),
              bytes[4] == 0x2d, bytes[7] == 0x2d, bytes[10] == 0x54,
              bytes[13] == 0x3a, bytes[16] == 0x3a, bytes[19] == 0x5a
        else { throw TohsenoCompanionError.invalidEncoding("timestamp is not canonical RFC 3339") }
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"
        guard let date = formatter.date(from: value), formatter.string(from: date) == value else {
            throw TohsenoCompanionError.invalidEncoding("timestamp is not canonical RFC 3339")
        }
        return date
    }

    static func format(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"
        return formatter.string(from: date)
    }
}
