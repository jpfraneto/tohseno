import Foundation

public struct CapabilityGrant: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-capability-grant/1"
    static let signatureDomain = "tohseno.companion.capability-grant.v1"

    public let schema: String
    public let capabilityID: String
    public let workspaceID: String
    public let deviceID: String
    public let allowedActions: [CompanionCapability]
    public let issuedAt: String
    public let expiresAt: String?
    public let revocationEpoch: UInt64
    public let studioSigningPublicKey: String
    public let signature: String

    public init(
        schema: String = schemaV1,
        capabilityID: String,
        workspaceID: String,
        deviceID: String,
        allowedActions: [CompanionCapability],
        issuedAt: String,
        expiresAt: String?,
        revocationEpoch: UInt64,
        studioSigningPublicKey: String,
        signature: String
    ) {
        self.schema = schema
        self.capabilityID = capabilityID
        self.workspaceID = workspaceID
        self.deviceID = deviceID
        self.allowedActions = allowedActions
        self.issuedAt = issuedAt
        self.expiresAt = expiresAt
        self.revocationEpoch = revocationEpoch
        self.studioSigningPublicKey = studioSigningPublicKey
        self.signature = signature
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let expected: Set<String> = container.contains(.expiresAt)
            ? [
                "schema", "capability_id", "workspace_id", "device_id",
                "allowed_actions", "issued_at", "expires_at", "revocation_epoch",
                "studio_signing_public_key", "signature",
            ]
            : [
                "schema", "capability_id", "workspace_id", "device_id",
                "allowed_actions", "issued_at", "revocation_epoch",
                "studio_signing_public_key", "signature",
            ]
        try requireExactKeys(decoder, expected)
        schema = try container.decode(String.self, forKey: .schema)
        capabilityID = try container.decode(String.self, forKey: .capabilityID)
        workspaceID = try container.decode(String.self, forKey: .workspaceID)
        deviceID = try container.decode(String.self, forKey: .deviceID)
        allowedActions = try container.decode([CompanionCapability].self, forKey: .allowedActions)
        issuedAt = try container.decode(String.self, forKey: .issuedAt)
        expiresAt = try container.decodeIfPresent(String.self, forKey: .expiresAt)
        revocationEpoch = try container.decode(UInt64.self, forKey: .revocationEpoch)
        studioSigningPublicKey = try container.decode(String.self, forKey: .studioSigningPublicKey)
        signature = try container.decode(String.self, forKey: .signature)
    }

    enum CodingKeys: String, CodingKey {
        case schema
        case capabilityID = "capability_id"
        case workspaceID = "workspace_id"
        case deviceID = "device_id"
        case allowedActions = "allowed_actions"
        case issuedAt = "issued_at"
        case expiresAt = "expires_at"
        case revocationEpoch = "revocation_epoch"
        case studioSigningPublicKey = "studio_signing_public_key"
        case signature
    }

    public func verify(
        trustedStudioSigningKey: Data,
        expectedWorkspaceID: String? = nil,
        expectedDeviceID: String? = nil,
        minimumRevocationEpoch: UInt64 = 0,
        now: Date = Date()
    ) throws {
        guard schema == Self.schemaV1 else {
            throw TohsenoCompanionError.invalidCapability("unsupported schema")
        }
        try requireIdentifier(capabilityID, field: "capability_id")
        try requireIdentifier(workspaceID, field: "workspace_id")
        try requireIdentifier(deviceID, field: "device_id")
        guard !allowedActions.isEmpty, allowedActions.count <= CompanionCapability.allCases.count,
              zip(allowedActions, allowedActions.dropFirst()).allSatisfy(<)
        else {
            throw TohsenoCompanionError.invalidCapability("actions are not unique and sorted")
        }
        let embeddedKey = try Base64URL.decode(studioSigningPublicKey, expectedBytes: 32)
        guard embeddedKey == trustedStudioSigningKey else {
            throw TohsenoCompanionError.invalidCapability("Studio signing key differs")
        }
        if let expectedWorkspaceID, workspaceID != expectedWorkspaceID {
            throw TohsenoCompanionError.invalidCapability("workspace differs")
        }
        if let expectedDeviceID, deviceID != expectedDeviceID {
            throw TohsenoCompanionError.invalidCapability("device differs")
        }
        guard revocationEpoch >= minimumRevocationEpoch else {
            throw TohsenoCompanionError.capabilityRevoked
        }
        let issued = try CompanionTimestamp.parse(issuedAt)
        if now < issued.addingTimeInterval(-30) {
            throw TohsenoCompanionError.invalidCapability("grant is not valid yet")
        }
        if let expiresAt {
            let expires = try CompanionTimestamp.parse(expiresAt)
            guard expires.timeIntervalSince(issued) > 0,
                  expires.timeIntervalSince(issued) <= 366 * 24 * 60 * 60 else {
                throw TohsenoCompanionError.invalidCapability("grant lifetime is invalid")
            }
            if now > expires.addingTimeInterval(30) {
                throw TohsenoCompanionError.capabilityRevoked
            }
        }
        let signatureBytes = try Base64URL.decode(signature, expectedBytes: 64)
        guard try CompanionIdentity.verify(
            publicKey: trustedStudioSigningKey,
            domain: Self.signatureDomain,
            message: canonicalBody(),
            signature: signatureBytes
        ) else {
            throw TohsenoCompanionError.invalidCapability("signature verification failed")
        }
    }

    public func require(_ action: CompanionCapability) throws {
        guard allowedActions.contains(action) else {
            throw TohsenoCompanionError.capabilityDenied(action)
        }
    }

    public func canonicalBody() throws -> Data {
        var object: [String: CanonicalValue] = [
            "allowed_actions": .array(allowedActions.map { .string($0.rawValue) }),
            "capability_id": .string(capabilityID),
            "device_id": .string(deviceID),
            "issued_at": .string(issuedAt),
            "revocation_epoch": .unsigned(revocationEpoch),
            "schema": .string(schema),
            "studio_signing_public_key": .string(studioSigningPublicKey),
            "workspace_id": .string(workspaceID),
        ]
        if let expiresAt { object["expires_at"] = .string(expiresAt) }
        return try CanonicalValue.object(object).data()
    }
}
