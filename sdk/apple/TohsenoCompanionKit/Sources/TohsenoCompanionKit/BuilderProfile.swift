import CryptoKit
import Foundation

public struct BuilderProfile: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.builder-profile/1"
    public let schema: String
    public let builderID: String
    public let displayName: String
    public let handle: String?
    public let avatarSHA256: String?
    public let externalAttestations: [ExternalAttestation]
    public let updatedAt: String
    public let nonce: UInt64

    public struct ExternalAttestation: Codable, Equatable, Sendable {
        public let provider: String
        public let subject: String
        public let proofURL: String
        public let verifiedAt: String

        enum CodingKeys: String, CodingKey {
            case provider, subject
            case proofURL = "proof_url"
            case verifiedAt = "verified_at"
        }
    }

    public init(
        builderID: String,
        displayName: String,
        handle: String?,
        avatarSHA256: String? = nil,
        externalAttestations: [ExternalAttestation] = [],
        updatedAt: String,
        nonce: UInt64
    ) throws {
        guard builderID.range(of: #"^eip155:4663:0x[0-9a-f]{40}$"#, options: .regularExpression) != nil,
              !displayName.isEmpty, displayName.utf8.count <= 80,
              handle.map(Self.validName) ?? true,
              avatarSHA256.map(Self.validDigest) ?? true,
              externalAttestations.count <= 8, nonce > 0,
              Self.canonicalTimestamp(updatedAt)
        else { throw TohsenoCompanionError.invalidEncoding("invalid public Builder profile") }
        schema = Self.schemaV1
        self.builderID = builderID
        self.displayName = displayName
        self.handle = handle
        self.avatarSHA256 = avatarSHA256
        self.externalAttestations = externalAttestations
        self.updatedAt = updatedAt
        self.nonce = nonce
    }

    enum CodingKeys: String, CodingKey {
        case schema
        case builderID = "builder_id"
        case displayName = "display_name"
        case handle
        case avatarSHA256 = "avatar_sha256"
        case externalAttestations = "external_attestations"
        case updatedAt = "updated_at"
        case nonce
    }

    func canonicalValue() -> CanonicalValue {
        .object([
            "avatar_sha256": avatarSHA256.map(CanonicalValue.string) ?? .null,
            "builder_id": .string(builderID),
            "display_name": .string(displayName),
            "external_attestations": .array(externalAttestations.map { value in
                .object([
                    "proof_url": .string(value.proofURL), "provider": .string(value.provider),
                    "subject": .string(value.subject), "verified_at": .string(value.verifiedAt),
                ])
            }),
            "handle": handle.map(CanonicalValue.string) ?? .null,
            "nonce": .unsigned(nonce), "schema": .string(schema),
            "updated_at": .string(updatedAt),
        ])
    }

    private static func validName(_ value: String) -> Bool {
        value.range(of: #"^[a-z0-9]+(?:-[a-z0-9]+)*$"#, options: .regularExpression) != nil
            && (2 ... 32).contains(value.count)
    }

    private static func validDigest(_ value: String) -> Bool {
        BuilderDeviceAnnouncement.hex32(value) != nil
    }

    private static func canonicalTimestamp(_ value: String) -> Bool {
        value.range(of: #"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$"#, options: .regularExpression) != nil
    }
}

public struct AliasClaim: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.alias-claim/1"
    public let schema: String
    public let builderID: String
    public let shotID: String
    public let alias: String
    public let requestID: String
    public let nonce: UInt64
    public let deadline: UInt64
    public let requestedAt: String

    public init(
        builderID: String,
        shotID: String,
        alias: String,
        requestID: String,
        nonce: UInt64,
        deadline: UInt64,
        requestedAt: String
    ) throws {
        guard builderID.range(of: #"^eip155:4663:0x[0-9a-f]{40}$"#, options: .regularExpression) != nil,
              BuilderDeviceAnnouncement.hex32(shotID) != nil,
              BuilderDeviceAnnouncement.hex32(requestID) != nil,
              alias.range(of: #"^[a-z0-9]+(?:-[a-z0-9]+)*$"#, options: .regularExpression) != nil,
              (2 ... 64).contains(alias.count), nonce > 0, deadline > 0,
              requestedAt.range(of: #"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$"#, options: .regularExpression) != nil
        else { throw TohsenoCompanionError.invalidEncoding("invalid alias claim") }
        schema = Self.schemaV1
        self.builderID = builderID
        self.shotID = shotID
        self.alias = alias
        self.requestID = requestID
        self.nonce = nonce
        self.deadline = deadline
        self.requestedAt = requestedAt
    }

    enum CodingKeys: String, CodingKey {
        case schema
        case builderID = "builder_id"
        case shotID = "shot_id"
        case alias
        case requestID = "request_id"
        case nonce, deadline
        case requestedAt = "requested_at"
    }

    func canonicalValue() -> CanonicalValue {
        .object([
            "alias": .string(alias), "builder_id": .string(builderID),
            "deadline": .unsigned(deadline), "nonce": .unsigned(nonce),
            "request_id": .string(requestID), "requested_at": .string(requestedAt),
            "schema": .string(schema), "shot_id": .string(shotID),
        ])
    }
}

public struct SignedBuilderProfile: Codable, Equatable, Sendable {
    public let schema: String
    public let profile: BuilderProfile
    public let signer: Signer
    public let authorization: Authorization

    public init(profile: BuilderProfile, signature: BuilderDeviceAuthorization) throws {
        let digest = Data(SHA256.hash(data: try profile.canonicalValue().data()))
        guard signature.digest == "0x\(digest.hexadecimal)" else {
            throw TohsenoCompanionError.invalidEncoding("profile signature digest differs")
        }
        schema = "tohseno.signed-builder-profile/1"
        self.profile = profile
        signer = Signer(x: signature.signer.x, y: signature.signer.y)
        authorization = Authorization(signature)
    }
}

public struct SignedAliasClaim: Codable, Equatable, Sendable {
    public let schema: String
    public let claim: AliasClaim
    public let signer: Signer
    public let authorization: Authorization

    public init(claim: AliasClaim, signature: BuilderDeviceAuthorization) throws {
        let digest = Data(SHA256.hash(data: try claim.canonicalValue().data()))
        guard signature.digest == "0x\(digest.hexadecimal)" else {
            throw TohsenoCompanionError.invalidEncoding("alias signature digest differs")
        }
        schema = "tohseno.signed-alias-claim/1"
        self.claim = claim
        signer = Signer(x: signature.signer.x, y: signature.signer.y)
        authorization = Authorization(signature)
    }
}

public struct Signer: Codable, Equatable, Sendable {
    public let x: String
    public let y: String
}

public struct Authorization: Codable, Equatable, Sendable {
    public struct Signature: Codable, Equatable, Sendable { public let r: String; public let s: String }
    public let algorithm: String
    public let digest: String
    public let signature: Signature
    public let lowS: Bool

    init(_ value: BuilderDeviceAuthorization) {
        algorithm = value.algorithm; digest = value.digest
        signature = Signature(r: value.r, s: value.s); lowS = value.lowS
    }

    enum CodingKeys: String, CodingKey { case algorithm, digest, signature; case lowS = "low_s" }
}

public extension BuilderDeviceIdentity {
    func sign(profile: BuilderProfile, allowSoftwareTest: Bool = false) throws -> SignedBuilderProfile {
        let digest = Data(SHA256.hash(data: try profile.canonicalValue().data()))
        return try SignedBuilderProfile(
            profile: profile,
            signature: sign(digestHex: "0x\(digest.hexadecimal)", allowSoftwareTest: allowSoftwareTest)
        )
    }

    func sign(claim: AliasClaim, allowSoftwareTest: Bool = false) throws -> SignedAliasClaim {
        let digest = Data(SHA256.hash(data: try claim.canonicalValue().data()))
        return try SignedAliasClaim(
            claim: claim,
            signature: sign(digestHex: "0x\(digest.hexadecimal)", allowSoftwareTest: allowSoftwareTest)
        )
    }
}

private extension Data {
    var hexadecimal: String { map { String(format: "%02x", $0) }.joined() }
}
