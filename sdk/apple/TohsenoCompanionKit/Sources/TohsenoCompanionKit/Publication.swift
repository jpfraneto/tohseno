import CryptoKit
import Foundation

public struct BuilderDeviceAnnouncement: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.builder-device-announcement/1"
    public let schema: String
    public let keyID: String
    public let x: String
    public let y: String
    public let securityLevel: String
    public let testOnly: Bool

    enum CodingKeys: String, CodingKey {
        case schema, x, y
        case keyID = "key_id"
        case securityLevel = "security_level"
        case testOnly = "test_only"
    }

    public init(publicIdentity: BuilderDevicePublicIdentity) {
        schema = Self.schemaV1
        keyID = publicIdentity.keyID
        x = publicIdentity.x
        y = publicIdentity.y
        securityLevel = publicIdentity.securityLevel
        testOnly = publicIdentity.testOnly
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "key_id", "x", "y", "security_level", "test_only"])
        let value = try decoder.container(keyedBy: CodingKeys.self)
        schema = try value.decode(String.self, forKey: .schema)
        keyID = try value.decode(String.self, forKey: .keyID)
        x = try value.decode(String.self, forKey: .x)
        y = try value.decode(String.self, forKey: .y)
        securityLevel = try value.decode(String.self, forKey: .securityLevel)
        testOnly = try value.decode(Bool.self, forKey: .testOnly)
        try validate()
    }

    public func validate(allowSoftwareTest: Bool = false) throws {
        guard schema == Self.schemaV1,
              let xBytes = Self.hex32(x), let yBytes = Self.hex32(y),
              Self.hex32(keyID) == Keccak256.hash(xBytes + yBytes),
              ["secure_enclave", "software_test"].contains(securityLevel),
              testOnly == (securityLevel == "software_test"),
              allowSoftwareTest || !testOnly
        else { throw TohsenoCompanionError.invalidEncoding("invalid Builder DeviceKey announcement") }
    }

    func canonicalValue() -> CanonicalValue {
        .object([
            "key_id": .string(keyID), "schema": .string(schema),
            "security_level": .string(securityLevel), "test_only": .bool(testOnly),
            "x": .string(x), "y": .string(y),
        ])
    }

    static func hex32(_ value: String) -> Data? {
        guard value.hasPrefix("0x"), value.count == 66,
              value.dropFirst(2).allSatisfy({ $0.isNumber || ("a" ... "f").contains($0) })
        else { return nil }
        var result = Data(capacity: 32)
        var index = value.index(value.startIndex, offsetBy: 2)
        for _ in 0 ..< 32 {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index ..< next], radix: 16) else { return nil }
            result.append(byte)
            index = next
        }
        return result
    }
}

public struct ClaimEditionPolicySummary: Codable, Equatable, Sendable {
    public let kind: ClaimEditionPolicy.Kind
    public let maxClaims: UInt64
    public let closesAt: UInt64

    enum CodingKeys: String, CodingKey {
        case kind
        case maxClaims = "max_claims"
        case closesAt = "closes_at"
    }

    public init(policy: ClaimEditionPolicy) {
        kind = policy.kind
        maxClaims = policy.maxClaims
        closesAt = policy.closesAt
    }

    public func policy() throws -> ClaimEditionPolicy {
        let value = try ClaimEditionPolicy(maxClaims: maxClaims, closesAt: closesAt)
        guard value.kind == kind else {
            throw TohsenoCompanionError.invalidEncoding("Claim Edition policy shape is invalid")
        }
        return value
    }
}

public struct ClaimEditionApprovalContext: Codable, Equatable, Sendable {
    public let claimsContract: String
    public let claimsActivationSigningDigest: String
    public let controller: String
    public let editionNonce: UInt64
    public let actionDeadline: UInt64
    public let requestedPolicy: ClaimEditionPolicySummary?

    enum CodingKeys: String, CodingKey {
        case claimsContract = "claims_contract"
        case claimsActivationSigningDigest = "claims_activation_signing_digest"
        case controller
        case editionNonce = "edition_nonce"
        case actionDeadline = "action_deadline"
        case requestedPolicy = "requested_policy"
    }

    public init(from decoder: Decoder) throws {
        let value = try decoder.container(keyedBy: CodingKeys.self)
        var keys: Set<String> = ["claims_contract", "claims_activation_signing_digest",
                                 "controller", "edition_nonce", "action_deadline"]
        if value.contains(.requestedPolicy) { keys.insert("requested_policy") }
        try requireExactKeys(decoder, keys)
        claimsContract = try value.decode(String.self, forKey: .claimsContract)
        claimsActivationSigningDigest = try value.decode(String.self, forKey: .claimsActivationSigningDigest)
        controller = try value.decode(String.self, forKey: .controller)
        editionNonce = try value.decode(UInt64.self, forKey: .editionNonce)
        actionDeadline = try value.decode(UInt64.self, forKey: .actionDeadline)
        requestedPolicy = try value.decodeIfPresent(ClaimEditionPolicySummary.self, forKey: .requestedPolicy)
    }

    public func validate(request: PublicationApprovalRequest) throws {
        guard ClaimsActionEncoding.addressWord(claimsContract) != nil,
              BuilderDeviceAnnouncement.hex32(claimsActivationSigningDigest) != nil,
              controller == String(request.builderID.suffix(42)),
              editionNonce <= ClaimsActionEncoding.maximumSafeInteger,
              actionDeadline == request.actionDeadline
        else { throw TohsenoCompanionError.invalidEncoding("invalid Claim Edition approval context") }
        _ = try requestedPolicy?.policy()
    }

    public func action(
        request: PublicationApprovalRequest,
        policy: ClaimEditionPolicy
    ) throws -> OpenClaimEditionAction {
        try validate(request: request)
        if let required = try requestedPolicy?.policy(), required != policy {
            throw TohsenoCompanionError.invalidEncoding("selected Claim Edition differs from CLI policy")
        }
        return OpenClaimEditionAction(
            shotRegistry: request.shotRegistry,
            shotID: request.shotID,
            maxClaims: policy.maxClaims,
            closesAt: policy.closesAt,
            controller: controller,
            nonce: editionNonce,
            deadline: actionDeadline
        )
    }
}

public struct ApprovedClaimEdition: Codable, Equatable, Sendable {
    public let policy: ClaimEditionPolicySummary
    public let action: OpenClaimEditionAction
    public let digest: String
    public let signature: BuilderDeviceSignature

    public init(
        policy: ClaimEditionPolicy,
        action: OpenClaimEditionAction,
        digest: String,
        signature: BuilderDeviceSignature
    ) throws {
        let summary = ClaimEditionPolicySummary(policy: policy)
        guard action.maxClaims == policy.maxClaims, action.closesAt == policy.closesAt,
              BuilderDeviceAnnouncement.hex32(digest) != nil,
              signature.digest == digest
        else { throw TohsenoCompanionError.invalidEncoding("invalid approved Claim Edition") }
        self.policy = summary
        self.action = action
        self.digest = digest
        self.signature = signature
    }

    func canonicalValue() -> CanonicalValue {
        .object([
            "action": action.canonicalValue(),
            "digest": .string(digest),
            "policy": policy.canonicalValue(),
            "signature": signature.canonicalValue(),
        ])
    }
}

private extension ClaimEditionPolicySummary {
    func canonicalValue() -> CanonicalValue {
        .object([
            "closes_at": .unsigned(closesAt),
            "kind": .string(kind.rawValue),
            "max_claims": .unsigned(maxClaims),
        ])
    }
}

private extension OpenClaimEditionAction {
    func canonicalValue() -> CanonicalValue {
        .object([
            "closes_at": .unsigned(closesAt),
            "controller": .string(controller),
            "deadline": .unsigned(deadline),
            "max_claims": .unsigned(maxClaims),
            "nonce": .unsigned(nonce),
            "shot_id": .string(shotID),
            "shot_registry": .string(shotRegistry),
        ])
    }
}

public struct PublicationApprovalRequest: Codable, Equatable, Sendable, Identifiable {
    public static let schemaV1 = "tohseno.publication-approval-request/1"
    public static let schemaV2 = "tohseno.publication-approval-request/2"
    public static let activeChainID: UInt64 = 4663
    public static let activeFactory = "0xb1bd208cd2af98e701f43d06aaa889d3a594df65"
    public static let activeRegistry = "0x3fe6508ba2660bc575080024f402c192a2e035a0"
    public static let activeGeneration = "0.8.0"
    public static let activeActivationDigest = "0x2b640260595def403343810d0dc4ee231e1faff427581be4f7b40cff4c189d28"

    public let schema: String
    public let jobID: String
    public let appName: String
    public let sourceFileCount: UInt64
    public let sourceByteLength: UInt64
    public let installAllowed: Bool
    public let forkAllowed: Bool
    public let requestedRoute: String
    public let chainID: UInt64
    public let builderAccountFactory: String
    public let shotRegistry: String
    public let builderID: String
    public let builderDevice: BuilderDeviceAnnouncement
    public let shotID: String
    public let checkpointSequence: UInt64
    public let actionNonce: UInt64
    public let actionDeadline: UInt64
    public let catalogReleaseJSON: String
    public let catalogDigest: String
    public let registryActionJSON: String
    public let registryDigest: String
    public let issuedAt: String
    public let expiresAt: String
    public let publicationKind: String?
    public let claimEdition: ClaimEditionApprovalContext?

    public var id: String { jobID }
    public var catalogAppSlug: String? {
        guard let release = try? Self.object(catalogReleaseJSON),
              let display = release["display"] as? [String: Any]
        else { return nil }
        return display["app_slug"] as? String
    }

    enum CodingKeys: String, CodingKey {
        case schema
        case jobID = "job_id"
        case appName = "app_name"
        case sourceFileCount = "source_file_count"
        case sourceByteLength = "source_byte_length"
        case installAllowed = "install_allowed"
        case forkAllowed = "fork_allowed"
        case requestedRoute = "requested_route"
        case chainID = "chain_id"
        case builderAccountFactory = "builder_account_factory"
        case shotRegistry = "shot_registry"
        case builderID = "builder_id"
        case builderDevice = "builder_device"
        case shotID = "shot_id"
        case checkpointSequence = "checkpoint_sequence"
        case actionNonce = "action_nonce"
        case actionDeadline = "action_deadline"
        case catalogReleaseJSON = "catalog_release_json"
        case catalogDigest = "catalog_digest"
        case registryActionJSON = "registry_action_json"
        case registryDigest = "registry_digest"
        case issuedAt = "issued_at"
        case expiresAt = "expires_at"
        case publicationKind = "publication_kind"
        case claimEdition = "claim_edition"
    }

    public init(from decoder: Decoder) throws {
        let value = try decoder.container(keyedBy: CodingKeys.self)
        let observedSchema = try value.decode(String.self, forKey: .schema)
        var keys: Set<String> = [
            "schema", "job_id", "app_name", "source_file_count", "source_byte_length",
            "install_allowed", "fork_allowed", "requested_route", "chain_id",
            "builder_account_factory", "shot_registry", "builder_id", "builder_device",
            "shot_id", "checkpoint_sequence", "action_nonce", "action_deadline",
            "catalog_release_json", "catalog_digest", "registry_action_json",
            "registry_digest", "issued_at", "expires_at",
        ]
        if observedSchema == Self.schemaV2 {
            keys.formUnion(["publication_kind", "claim_edition"])
        }
        try requireExactKeys(decoder, keys)
        schema = observedSchema
        jobID = try value.decode(String.self, forKey: .jobID)
        appName = try value.decode(String.self, forKey: .appName)
        sourceFileCount = try value.decode(UInt64.self, forKey: .sourceFileCount)
        sourceByteLength = try value.decode(UInt64.self, forKey: .sourceByteLength)
        installAllowed = try value.decode(Bool.self, forKey: .installAllowed)
        forkAllowed = try value.decode(Bool.self, forKey: .forkAllowed)
        requestedRoute = try value.decode(String.self, forKey: .requestedRoute)
        chainID = try value.decode(UInt64.self, forKey: .chainID)
        builderAccountFactory = try value.decode(String.self, forKey: .builderAccountFactory)
        shotRegistry = try value.decode(String.self, forKey: .shotRegistry)
        builderID = try value.decode(String.self, forKey: .builderID)
        builderDevice = try value.decode(BuilderDeviceAnnouncement.self, forKey: .builderDevice)
        shotID = try value.decode(String.self, forKey: .shotID)
        checkpointSequence = try value.decode(UInt64.self, forKey: .checkpointSequence)
        actionNonce = try value.decode(UInt64.self, forKey: .actionNonce)
        actionDeadline = try value.decode(UInt64.self, forKey: .actionDeadline)
        catalogReleaseJSON = try value.decode(String.self, forKey: .catalogReleaseJSON)
        catalogDigest = try value.decode(String.self, forKey: .catalogDigest)
        registryActionJSON = try value.decode(String.self, forKey: .registryActionJSON)
        registryDigest = try value.decode(String.self, forKey: .registryDigest)
        issuedAt = try value.decode(String.self, forKey: .issuedAt)
        expiresAt = try value.decode(String.self, forKey: .expiresAt)
        publicationKind = try value.decodeIfPresent(String.self, forKey: .publicationKind)
        claimEdition = try value.decodeIfPresent(ClaimEditionApprovalContext.self, forKey: .claimEdition)
        try validate()
    }

    public func validate(allowSoftwareTest: Bool = false, now: Date = Date()) throws {
        try builderDevice.validate(allowSoftwareTest: allowSoftwareTest)
        guard [Self.schemaV1, Self.schemaV2].contains(schema), chainID == Self.activeChainID,
              builderAccountFactory == Self.activeFactory, shotRegistry == Self.activeRegistry,
              Self.isActiveBuilderID(builderID),
              BuilderDeviceAnnouncement.hex32(shotID) != nil,
              sourceFileCount > 0, sourceFileCount <= 100_000,
              sourceByteLength > 0, sourceByteLength <= 2 * 1024 * 1024 * 1024,
              installAllowed, checkpointSequence > 0,
              requestedRoute.hasPrefix("/"), !requestedRoute.contains(".."),
              catalogReleaseJSON.utf8.count <= 512 * 1024,
              registryActionJSON.utf8.count <= 64 * 1024,
              let suppliedCatalog = BuilderDeviceAnnouncement.hex32(catalogDigest),
              let suppliedRegistry = BuilderDeviceAnnouncement.hex32(registryDigest)
        else { throw TohsenoCompanionError.invalidEncoding("invalid publication approval request") }
        guard try predictedBuilderID() == builderID else {
            throw TohsenoCompanionError.invalidEncoding("BuilderID is not the active factory prediction for this DeviceKey")
        }

        let canonicalRelease = try Self.canonicalObject(catalogReleaseJSON)
        guard Data(SHA256.hash(data: canonicalRelease)) == suppliedCatalog else {
            throw TohsenoCompanionError.invalidEncoding("catalog digest differs from structured release")
        }
        let release = try Self.object(catalogReleaseJSON)
        try validateRelease(release)
        let computedRegistry = try registryActionDigest()
        guard computedRegistry == suppliedRegistry else {
            throw TohsenoCompanionError.invalidEncoding("Registry digest differs from structured action")
        }
        let issued = try CompanionTimestamp.parse(issuedAt)
        let expires = try CompanionTimestamp.parse(expiresAt)
        guard expires > issued, expires.timeIntervalSince(issued) <= 24 * 60 * 60,
              actionDeadline == UInt64(expires.timeIntervalSince1970.rounded()), now <= expires
        else { throw TohsenoCompanionError.invalidEncoding("publication approval expired") }
        if schema == Self.schemaV1 {
            guard publicationKind == nil, claimEdition == nil else {
                throw TohsenoCompanionError.invalidEncoding("legacy publication request carries Claims fields")
            }
        } else {
            switch (publicationKind, claimEdition, checkpointSequence) {
            case ("ship", let context?, 1): try context.validate(request: self)
            case ("update", nil, 2...): break
            default:
                throw TohsenoCompanionError.invalidEncoding("publication kind and Claim Edition disagree")
            }
        }
    }

    static func isActiveBuilderID(_ value: String) -> Bool {
        value.range(of: #"^eip155:4663:0x[0-9a-f]{40}$"#, options: .regularExpression) != nil
    }

    private func validateRelease(_ release: [String: Any]) throws {
        try Self.requireKeys(release, [
            "schema", "generation", "shot_id", "builder_id", "release_id", "published_at",
            "display", "source", "build", "permissions", "parent", "checkpoint_sequence",
            "public_checkpoint_digest",
        ], "catalog release")
        guard let generation = release["generation"] as? [String: Any],
              let display = release["display"] as? [String: Any],
              let source = release["source"] as? [String: Any],
              let build = release["build"] as? [String: Any],
              let permissions = release["permissions"] as? [String: Any]
        else { throw TohsenoCompanionError.invalidEncoding("catalog release has invalid structured fields") }
        try Self.requireKeys(generation, [
            "contract_generation", "chain_id", "builder_account_factory", "shot_registry",
            "activation_signing_digest",
        ], "catalog generation")
        try Self.requireKeys(display, [
            "name", "description", "icon_sha256", "builder_handle", "app_slug",
        ], "catalog display")
        try Self.requireKeys(source, [
            "format", "sha256", "byte_length", "source_tree_sha256", "file_count",
            "uncompressed_byte_length",
        ], "catalog source")
        try Self.requireKeys(build, [
            "container_kind", "container_path", "scheme", "original_bundle_identifier",
            "minimum_ios", "device_families", "dependency_locks", "safety",
        ], "catalog build")
        try Self.requireKeys(permissions, [
            "install_allowed", "fork_allowed", "distributor_rights_declared", "spdx_license",
        ], "catalog permissions")
        let releaseSequence = Self.uint(release["checkpoint_sequence"])
        let generationChain = Self.uint(generation["chain_id"])
        let compressedBytes = Self.uint(source["byte_length"])
        let files = Self.uint(source["file_count"])
        let sourceBytes = Self.uint(source["uncompressed_byte_length"])
        guard release["schema"] as? String == "tohseno.catalog-release/1",
              release["shot_id"] as? String == shotID,
              release["builder_id"] as? String == builderID,
              BuilderDeviceAnnouncement.hex32(release["release_id"] as? String ?? "") != nil,
              release["published_at"] as? String == issuedAt,
              releaseSequence == checkpointSequence,
              BuilderDeviceAnnouncement.hex32(release["public_checkpoint_digest"] as? String ?? "") != nil,
              generation["contract_generation"] as? String == Self.activeGeneration,
              generationChain == chainID,
              generation["builder_account_factory"] as? String == builderAccountFactory,
              generation["shot_registry"] as? String == shotRegistry,
              generation["activation_signing_digest"] as? String == Self.activeActivationDigest,
              display["name"] as? String == appName,
              source["format"] as? String == "deterministic_tar",
              BuilderDeviceAnnouncement.hex32(source["sha256"] as? String ?? "") != nil,
              BuilderDeviceAnnouncement.hex32(source["source_tree_sha256"] as? String ?? "") != nil,
              compressedBytes != nil, compressedBytes! > 0, compressedBytes! <= 512 * 1024 * 1024,
              files == sourceFileCount,
              sourceBytes == sourceByteLength,
              permissions["install_allowed"] as? Bool == installAllowed,
              permissions["fork_allowed"] as? Bool == forkAllowed,
              permissions["distributor_rights_declared"] as? Bool == true
        else { throw TohsenoCompanionError.invalidEncoding("catalog release differs from approval summary") }
        try Self.validateDisplay(display)
        try Self.validateBuild(build)
        try Self.validatePermissions(permissions)
        try Self.validateParent(release["parent"], childShotID: shotID)
        guard requestedRoute == "/s/\(shotID.dropFirst(2))" else {
            throw TohsenoCompanionError.invalidEncoding("requested route is not the canonical Shot route")
        }
    }

    private static func validateDisplay(_ value: [String: Any]) throws {
        guard let name = value["name"] as? String, bounded(name, 1 ... 160),
              let description = value["description"] as? String, bounded(description, 1 ... 2_000),
              optionalHex32(value["icon_sha256"]),
              optionalIdentifier(value["builder_handle"], maximum: 32),
              optionalIdentifier(value["app_slug"], maximum: 64)
        else { throw TohsenoCompanionError.invalidEncoding("invalid catalog display") }
    }

    private static func validateBuild(_ value: [String: Any]) throws {
        guard let kind = value["container_kind"] as? String, ["project", "workspace"].contains(kind),
              let container = value["container_path"] as? String, safeRelativePath(container),
              let scheme = value["scheme"] as? String, bounded(scheme, 1 ... 256),
              let bundle = value["original_bundle_identifier"] as? String,
              bundle.range(of: #"^[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$"#, options: .regularExpression) != nil,
              let minimum = value["minimum_ios"] as? String,
              minimum.range(of: #"^\d+(?:\.\d+)*$"#, options: .regularExpression) != nil,
              let families = value["device_families"] as? [String],
              !families.isEmpty, families.count <= 8, sortedUnique(families),
              let locks = value["dependency_locks"] as? [[String: Any]], locks.count <= 128,
              let safety = value["safety"] as? [String: Any]
        else { throw TohsenoCompanionError.invalidEncoding("invalid catalog build recipe") }
        var prior = ""
        for lock in locks {
            try requireKeys(lock, ["path", "sha256"], "dependency lock")
            guard let path = lock["path"] as? String, safeRelativePath(path), path > prior,
                  BuilderDeviceAnnouncement.hex32(lock["sha256"] as? String ?? "") != nil
            else { throw TohsenoCompanionError.invalidEncoding("invalid catalog dependency lock") }
            prior = path
        }
        try requireKeys(safety, ["classification", "reasons"], "build safety")
        guard let classification = safety["classification"] as? String,
              ["green", "requires_mac_review", "unsupported"].contains(classification),
              let reasons = safety["reasons"] as? [String], reasons.count <= 64,
              sortedUnique(reasons), reasons.allSatisfy({ bounded($0, 1 ... 512) }),
              (classification == "green") == reasons.isEmpty
        else { throw TohsenoCompanionError.invalidEncoding("invalid catalog build safety") }
    }

    private static func validatePermissions(_ value: [String: Any]) throws {
        let license = value["spdx_license"]
        guard license is NSNull || ((license as? String).map { text in
            bounded(text, 1 ... 96)
                && text.range(of: #"^[A-Za-z0-9.+() -]+$"#, options: .regularExpression) != nil
        } ?? false)
        else { throw TohsenoCompanionError.invalidEncoding("invalid catalog permissions") }
    }

    private static func validateParent(_ value: Any?, childShotID: String) throws {
        if value is NSNull { return }
        guard let parent = value as? [String: Any] else {
            throw TohsenoCompanionError.invalidEncoding("invalid catalog parent")
        }
        try requireKeys(parent, ["parent_shot_id", "parent_release_digest"], "catalog parent")
        guard let shot = parent["parent_shot_id"] as? String, shot != childShotID,
              BuilderDeviceAnnouncement.hex32(shot) != nil,
              BuilderDeviceAnnouncement.hex32(parent["parent_release_digest"] as? String ?? "") != nil
        else { throw TohsenoCompanionError.invalidEncoding("invalid catalog parent") }
    }

    private static func requireKeys(_ value: [String: Any], _ expected: Set<String>, _ name: String) throws {
        guard Set(value.keys) == expected else {
            throw TohsenoCompanionError.invalidEncoding("\(name) has unknown or missing fields")
        }
    }

    private static func uint(_ value: Any?) -> UInt64? {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
              number.doubleValue >= 0, number.doubleValue.rounded() == number.doubleValue,
              number.doubleValue <= 9_007_199_254_740_991
        else { return nil }
        return number.uint64Value
    }

    private static func bounded(_ value: String, _ bounds: ClosedRange<Int>) -> Bool {
        bounds.contains(value.utf8.count) && !value.unicodeScalars.contains { $0.value < 0x20 || $0.value == 0x7f }
    }

    private static func optionalHex32(_ value: Any?) -> Bool {
        value is NSNull || BuilderDeviceAnnouncement.hex32(value as? String ?? "") != nil
    }

    private static func optionalIdentifier(_ value: Any?, maximum: Int) -> Bool {
        if value is NSNull { return true }
        guard let text = value as? String, (2 ... maximum).contains(text.count) else { return false }
        return text.range(of: #"^[a-z0-9]+(?:-[a-z0-9]+)*$"#, options: .regularExpression) != nil
    }

    private static func safeRelativePath(_ value: String) -> Bool {
        !value.isEmpty && !value.hasPrefix("/") && !value.contains("\\")
            && value.split(separator: "/", omittingEmptySubsequences: false).allSatisfy { $0 != "." && $0 != ".." && !$0.isEmpty }
    }

    private static func sortedUnique(_ values: [String]) -> Bool {
        values == values.sorted() && Set(values).count == values.count
    }

    func registryActionDigest() throws -> Data {
        let action = try Self.object(registryActionJSON)
        let release = try Self.object(catalogReleaseJSON)
        guard let publicCheckpoint = release["public_checkpoint_digest"] as? String else {
            throw TohsenoCompanionError.invalidEncoding("catalog release has no public checkpoint")
        }
        guard let type = action["type"] as? String,
              action["shot_id"] as? String == shotID,
              let shot = BuilderDeviceAnnouncement.hex32(shotID),
              let nonce = Self.uint(action["nonce"]), nonce == actionNonce,
              let deadline = Self.uint(action["deadline"]), deadline == actionDeadline
        else { throw TohsenoCompanionError.invalidEncoding("Registry action identity differs") }

        let typeHash: Data
        var words: [Data]
        switch type {
        case "REGISTER_SHOT":
            try Self.requireKeys(action, [
                "type", "shot_id", "controller", "head", "salt", "nonce", "deadline",
            ], "RegisterShot action")
            guard checkpointSequence == 1, nonce == 0,
                  action["controller"] as? String == String(builderID.suffix(42)),
                  action["head"] as? String == publicCheckpoint,
                  let controller = Self.addressWord(action["controller"] as? String),
                  let head = BuilderDeviceAnnouncement.hex32(action["head"] as? String ?? ""),
                  let salt = BuilderDeviceAnnouncement.hex32(action["salt"] as? String ?? "")
            else { throw TohsenoCompanionError.invalidEncoding("invalid RegisterShot approval") }
            typeHash = try Self.hex("c356ba3244a346558a5821261a4eccfb38382e0f90a60dc903003a671d5e828c")
            words = [typeHash, shot, controller, head, salt, Self.word(nonce), Self.word(deadline)]
        case "APPEND_CHECKPOINT":
            try Self.requireKeys(action, [
                "type", "shot_id", "previous_head", "new_head", "checkpoint_sequence", "nonce", "deadline",
            ], "AppendCheckpoint action")
            guard checkpointSequence >= 2, nonce + 1 == checkpointSequence,
                  let previous = BuilderDeviceAnnouncement.hex32(action["previous_head"] as? String ?? ""),
                  let next = BuilderDeviceAnnouncement.hex32(action["new_head"] as? String ?? ""),
                  action["new_head"] as? String == publicCheckpoint,
                  Self.uint(action["checkpoint_sequence"]) == checkpointSequence
            else { throw TohsenoCompanionError.invalidEncoding("invalid AppendCheckpoint approval") }
            typeHash = try Self.hex("4ada9482c2ee717b1b8faa0707d2096906a4cc7d3e9ab28cf94f2b8d220e22f5")
            words = [typeHash, shot, previous, next, Self.word(checkpointSequence), Self.word(nonce), Self.word(deadline)]
        default:
            throw TohsenoCompanionError.invalidEncoding("publication cannot approve this Registry action")
        }
        let structHash = Keccak256.hash(words.reduce(into: Data()) { $0.append($1) })
        let domainType = Keccak256.hash(Data("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)".utf8))
        let name = Keccak256.hash(Data("TOHSENO ShotRegistry".utf8))
        let version = Keccak256.hash(Data("2".utf8))
        guard let registry = Self.addressWord(shotRegistry) else {
            throw TohsenoCompanionError.invalidEncoding("invalid Registry address")
        }
        let domain = Keccak256.hash(domainType + name + version + Self.word(chainID) + registry)
        return Keccak256.hash(Data([0x19, 0x01]) + domain + structHash)
    }

    private func predictedBuilderID() throws -> String {
        guard let keyID = BuilderDeviceAnnouncement.hex32(builderDevice.keyID),
              let x = BuilderDeviceAnnouncement.hex32(builderDevice.x),
              let y = BuilderDeviceAnnouncement.hex32(builderDevice.y),
              let factory = Self.addressBytes(builderAccountFactory),
              let url = Bundle.module.url(
                forResource: "BuilderAccount.creation",
                withExtension: "hex"
              )
        else { throw TohsenoCompanionError.invalidEncoding("active BuilderAccount definition is unavailable") }
        let encoded = try String(contentsOf: url, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard encoded.hasPrefix("0x") else {
            throw TohsenoCompanionError.invalidEncoding("active BuilderAccount bytecode is invalid")
        }
        let creation = try Self.hex(String(encoded.dropFirst(2)))
        let accountSalt = Data(SHA256.hash(
            data: Data("TOHSENO-BUILDER-SALT-V1\0".utf8) + keyID
        ))
        let initHash = Keccak256.hash(creation + x + y)
        let create2 = Keccak256.hash(Data([0xff]) + factory + accountSalt + initHash)
        return "eip155:4663:0x\(create2.suffix(20).map { String(format: "%02x", $0) }.joined())"
    }

    private static func object(_ json: String) throws -> [String: Any] {
        let value = try JSONSerialization.jsonObject(with: Data(json.utf8), options: [])
        guard let object = value as? [String: Any] else {
            throw TohsenoCompanionError.invalidEncoding("publication payload must be an object")
        }
        return object
    }

    private static func canonicalObject(_ json: String) throws -> Data {
        let object = try self.object(json)
        let canonical = try canonicalJSON(object)
        guard canonical == Data(json.utf8) else {
            throw TohsenoCompanionError.invalidEncoding("publication payload is not canonical JSON")
        }
        return canonical
    }

    private static func canonicalJSON(_ value: Any) throws -> Data {
        if value is NSNull { return Data("null".utf8) }
        if let string = value as? String {
            return try JSONSerialization.data(withJSONObject: [string]).dropFirst().dropLast()
        }
        if let number = value as? NSNumber {
            if CFGetTypeID(number) == CFBooleanGetTypeID() {
                return Data(number.boolValue ? "true".utf8 : "false".utf8)
            }
            guard number.doubleValue >= 0, number.doubleValue.rounded() == number.doubleValue,
                  number.doubleValue <= 9_007_199_254_740_991
            else { throw TohsenoCompanionError.invalidEncoding("noncanonical publication number") }
            return Data(String(number.uint64Value).utf8)
        }
        if let array = value as? [Any] {
            var data = Data("[".utf8)
            for (index, item) in array.enumerated() {
                if index > 0 { data.append(Data(",".utf8)) }
                data.append(try canonicalJSON(item))
            }
            data.append(Data("]".utf8)); return data
        }
        if let object = value as? [String: Any] {
            var data = Data("{".utf8)
            for (index, key) in object.keys.sorted().enumerated() {
                if index > 0 { data.append(Data(",".utf8)) }
                data.append(try canonicalJSON(key)); data.append(Data(":".utf8))
                data.append(try canonicalJSON(object[key]!))
            }
            data.append(Data("}".utf8)); return data
        }
        throw TohsenoCompanionError.invalidEncoding("unsupported publication JSON value")
    }

    private static func addressWord(_ value: String?) -> Data? {
        guard let value, value.hasPrefix("0x"), value.count == 42,
              let bytes = try? hex(String(value.dropFirst(2))) else { return nil }
        return Data(repeating: 0, count: 12) + bytes
    }

    private static func addressBytes(_ value: String?) -> Data? {
        guard let value, value.hasPrefix("0x"), value.count == 42 else { return nil }
        return try? hex(String(value.dropFirst(2)))
    }

    private static func word(_ value: UInt64) -> Data {
        var data = Data(repeating: 0, count: 32)
        for offset in 0 ..< 8 { data[31 - offset] = UInt8(truncatingIfNeeded: value >> UInt64(offset * 8)) }
        return data
    }

    private static func hex(_ value: String) throws -> Data {
        guard value.count.isMultiple(of: 2) else { throw TohsenoCompanionError.invalidEncoding("invalid hex") }
        var data = Data(); var index = value.startIndex
        while index < value.endIndex {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index ..< next], radix: 16) else {
                throw TohsenoCompanionError.invalidEncoding("invalid hex")
            }
            data.append(byte); index = next
        }
        return data
    }
}

public struct BuilderDeviceSignature: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.builder-device-signature/1"
    public let schema: String
    public let signer: BuilderDeviceAnnouncement
    public let algorithm: String
    public let digest: String
    public let r: String
    public let s: String
    public let lowS: Bool

    enum CodingKeys: String, CodingKey { case schema, signer, algorithm, digest, r, s; case lowS = "low_s" }

    public init(_ value: BuilderDeviceAuthorization) {
        schema = Self.schemaV1
        signer = BuilderDeviceAnnouncement(publicIdentity: value.signer)
        algorithm = value.algorithm
        digest = value.digest
        r = value.r
        s = value.s
        lowS = value.lowS
    }

    func canonicalValue() -> CanonicalValue {
        .object([
            "algorithm": .string(algorithm), "digest": .string(digest),
            "low_s": .bool(lowS), "r": .string(r), "s": .string(s),
            "schema": .string(schema), "signer": signer.canonicalValue(),
        ])
    }
}
