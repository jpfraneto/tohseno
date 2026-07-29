import Foundation

public enum TohsenoCapability: String, Codable, CaseIterable, Sendable {
    case localStorage = "local_storage"
    case networkAccess = "network_access"
    case privateCloudKitSync = "private_cloudkit_sync"
    case storeKit = "storekit"
    case notifications
    case camera
    case microphone
    case location
    case contacts
    case health
    case bluetooth
    case otherAppleEntitlements = "other_apple_entitlements"
}

public struct TohsenoCapabilityDeclaration: Codable, Equatable, Sendable {
    public let capability: TohsenoCapability
    public let purpose: String
    public let details: [String]

    public init(
        capability: TohsenoCapability,
        purpose: String,
        details: [String] = []
    ) {
        self.capability = capability
        self.purpose = purpose
        self.details = details
    }
}

public struct TohsenoNetworkDeclaration: Codable, Equatable, Sendable {
    public let endpoint: String
    public let purpose: String

    public init(endpoint: String, purpose: String) {
        self.endpoint = endpoint
        self.purpose = purpose
    }
}

public struct TohsenoFactoryReference: Codable, Equatable, Sendable {
    public let implementation: String
    public let version: String
    public let sourceCommit: String

    public init(
        implementation: String,
        version: String,
        sourceCommit: String
    ) {
        self.implementation = implementation
        self.version = version
        self.sourceCommit = sourceCommit
    }

    private enum CodingKeys: String, CodingKey {
        case implementation
        case version
        case sourceCommit = "source_commit"
    }
}

public struct TohsenoRegistryReference: Codable, Equatable, Sendable {
    public let chainID: UInt64
    public let contract: String
    public let transaction: String?

    public init(chainID: UInt64, contract: String, transaction: String? = nil) {
        self.chainID = chainID
        self.contract = contract
        self.transaction = transaction
    }

    private enum CodingKeys: String, CodingKey {
        case chainID = "chain_id"
        case contract
        case transaction
    }
}

public enum TohsenoMetadataOriginKind: String, Codable, Sendable {
    case legacyAdoption = "legacy_adoption"
}

public struct TohsenoMetadataOrigin: Codable, Equatable, Sendable {
    public let kind: TohsenoMetadataOriginKind
    public let legacyLatestShot: UInt32
    public let legacySourceSHA256: String

    public init(
        kind: TohsenoMetadataOriginKind = .legacyAdoption,
        legacyLatestShot: UInt32,
        legacySourceSHA256: String
    ) {
        self.kind = kind
        self.legacyLatestShot = legacyLatestShot
        self.legacySourceSHA256 = legacySourceSHA256
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case legacyLatestShot = "legacy_latest_shot"
        case legacySourceSHA256 = "legacy_source_sha256"
    }
}

public enum TohsenoDistributionState: String, Codable, Sendable {
    case local
    case published
    case appStore = "app_store"
}

public enum TohsenoAppleSurface: String, Codable, Sendable {
    case iPhone = "iphone"
    case iPad = "ipad"
    case vision
}

public struct TohsenoDistribution: Codable, Equatable, Sendable {
    public let state: TohsenoDistributionState
    public let supportedAppleSurfaces: [TohsenoAppleSurface]
    public let appStoreID: UInt64?

    public init(
        state: TohsenoDistributionState,
        supportedAppleSurfaces: [TohsenoAppleSurface],
        appStoreID: UInt64? = nil
    ) {
        self.state = state
        self.supportedAppleSurfaces = supportedAppleSurfaces
        self.appStoreID = appStoreID
    }

    private enum CodingKeys: String, CodingKey {
        case state
        case supportedAppleSurfaces = "supported_apple_surfaces"
        case appStoreID = "app_store_id"
    }
}

public struct TohsenoMetadata: Codable, Equatable, Sendable {
    public let protocolName: String
    public let schema: String
    public let fascia: String
    public let shotID: String
    public let builderID: String
    public let sequence: UInt32
    public let previous: String?
    public let origin: TohsenoMetadataOrigin?
    public let evolutionCommitment: String
    public let sourceTreeSHA256: String
    public let fasciaSHA256: String
    public let bundleID: String
    public let bundleVersion: UInt32
    public let factory: TohsenoFactoryReference
    public let distribution: TohsenoDistribution
    public let capabilities: [TohsenoCapabilityDeclaration]
    public let network: [TohsenoNetworkDeclaration]
    public let registry: TohsenoRegistryReference?

    public init(
        protocolName: String = "tohseno",
        schema: String = "tohseno.app-metadata/1",
        fascia: String = "tohseno.apple/1",
        shotID: String,
        builderID: String,
        sequence: UInt32,
        previous: String?,
        origin: TohsenoMetadataOrigin? = nil,
        evolutionCommitment: String,
        sourceTreeSHA256: String,
        fasciaSHA256: String,
        bundleID: String,
        bundleVersion: UInt32,
        factory: TohsenoFactoryReference,
        distribution: TohsenoDistribution,
        capabilities: [TohsenoCapabilityDeclaration],
        network: [TohsenoNetworkDeclaration],
        registry: TohsenoRegistryReference?
    ) {
        self.protocolName = protocolName
        self.schema = schema
        self.fascia = fascia
        self.shotID = shotID
        self.builderID = builderID
        self.sequence = sequence
        self.previous = previous
        self.origin = origin
        self.evolutionCommitment = evolutionCommitment
        self.sourceTreeSHA256 = sourceTreeSHA256
        self.fasciaSHA256 = fasciaSHA256
        self.bundleID = bundleID
        self.bundleVersion = bundleVersion
        self.factory = factory
        self.distribution = distribution
        self.capabilities = capabilities
        self.network = network
        self.registry = registry
    }

    private enum CodingKeys: String, CodingKey {
        case protocolName = "protocol"
        case schema
        case fascia
        case shotID = "shot_id"
        case builderID = "builder_id"
        case sequence
        case previous
        case origin
        case evolutionCommitment = "evolution_commitment"
        case sourceTreeSHA256 = "source_tree_sha256"
        case fasciaSHA256 = "fascia_sha256"
        case bundleID = "bundle_id"
        case bundleVersion = "bundle_version"
        case factory
        case distribution
        case capabilities
        case network
        case registry
    }

    public func validate() throws {
        let hasNetworkCapability = capabilities.contains {
            $0.capability == .networkAccess
        }
        let lineageIsValid: Bool
        if let origin {
            let next = origin.legacyLatestShot.addingReportingOverflow(1)
            lineageIsValid = origin.kind == .legacyAdoption
                && !next.overflow
                && next.partialValue == sequence
                && origin.legacyLatestShot > 0
                && previous == nil
                && isNonzeroHex32(origin.legacySourceSHA256)
        } else {
            lineageIsValid = (sequence == 1) == (previous == nil)
                && (previous.map(isHex32) ?? (sequence == 1))
        }
        guard protocolName == "tohseno",
              schema == "tohseno.app-metadata/1",
              fascia == "tohseno.apple/1",
              sequence > 0,
              bundleVersion == sequence,
              isValidBundleID(bundleID),
              isNonzeroHex32(shotID),
              isNonzeroHex32(evolutionCommitment),
              isNonzeroHex32(sourceTreeSHA256),
              isNonzeroHex32(fasciaSHA256),
              lineageIsValid,
              isValidBuilderID(builderID),
              isBoundedToken(factory.implementation, minimum: 1, maximum: 200),
              isBoundedToken(factory.version, minimum: 1, maximum: 64),
              isLowerUnprefixedHex(factory.sourceCommit, digits: 40),
              Set(capabilities.map(\.capability)).count == capabilities.count,
              capabilities.allSatisfy({
                  isBoundedToken($0.purpose, minimum: 1, maximum: 500)
                      && $0.details.allSatisfy {
                          isBoundedToken($0, minimum: 1, maximum: 500)
                      }
              }),
              capabilities.allSatisfy({
                  ![
                      .privateCloudKitSync,
                      .otherAppleEntitlements,
                  ].contains($0.capability) || !$0.details.isEmpty
              }),
              network.allSatisfy({
                  isBoundedToken($0.endpoint, minimum: 1, maximum: 2_048)
                      && isBoundedToken(
                          $0.purpose,
                          minimum: 1,
                          maximum: 500
                      )
              }),
              Set(network.map(\.endpoint)).count == network.count,
              hasNetworkCapability == !network.isEmpty,
              !distribution.supportedAppleSurfaces.isEmpty,
              Set(distribution.supportedAppleSurfaces).count
                  == distribution.supportedAppleSurfaces.count,
              distribution.state == .appStore
                  ? distribution.appStoreID.map(isValidAppStoreID) == true
                  : distribution.appStoreID == nil,
              registry.map(isValidRegistry) ?? true
        else {
            throw TohsenoMetadataError.invalid
        }
    }

    public static func loadEmbedded(
        from bundle: Bundle = .main
    ) throws -> TohsenoMetadata {
        let url = bundle.url(
            forResource: "embedded-provenance",
            withExtension: "json",
            subdirectory: "TOHSENO"
        ) ?? bundle.url(
            forResource: "embedded-provenance",
            withExtension: "json"
        )
        guard let url else {
            throw TohsenoMetadataError.missing
        }
        let metadata = try decodeTransportJSON(Data(contentsOf: url))
        try metadata.validate()
        return metadata
    }

    public static func decodeTransportJSON(_ data: Data) throws -> TohsenoMetadata {
        try fasciaStrictJSONPreflight(data)
        try validateMetadataJSONShape(data)
        let metadata = try JSONDecoder().decode(TohsenoMetadata.self, from: data)
        try metadata.validate()
        return metadata
    }

    public func canonicalTransportJSON() throws -> Data {
        try validate()
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }
}

public enum TohsenoMetadataError: Error, Equatable, Sendable {
    case missing
    case invalid
    case bundleMismatch
}

private func isHex32(_ value: String) -> Bool {
    guard value.count == 66, value.hasPrefix("0x") else {
        return false
    }
    return value.dropFirst(2).unicodeScalars.allSatisfy {
        ("0" ... "9").contains(Character($0))
            || ("a" ... "f").contains(Character($0))
    }
}

private func isNonzeroHex32(_ value: String) -> Bool {
    isHex32(value) && value.dropFirst(2).contains { $0 != "0" }
}

private func isValidBuilderID(_ value: String) -> Bool {
    let prefix = "eip155:4663:0x"
    guard value.hasPrefix(prefix), value.utf8.count == prefix.utf8.count + 40 else {
        return false
    }
    let account = value.dropFirst(prefix.count)
    return account.allSatisfy {
        ("0" ... "9").contains($0) || ("a" ... "f").contains($0)
    } && account.contains { $0 != "0" }
}

private func isValidBundleID(_ value: String) -> Bool {
    guard isBoundedToken(value, minimum: 3, maximum: 255) else {
        return false
    }
    let parts = value.split(separator: ".", omittingEmptySubsequences: false)
    return parts.count >= 2 && parts.allSatisfy { part in
        !part.isEmpty
            && part.first != "-"
            && part.last != "-"
            && part.utf8.allSatisfy {
                (0x30 ... 0x39).contains($0)
                    || (0x41 ... 0x5a).contains($0)
                    || (0x61 ... 0x7a).contains($0)
                    || $0 == 0x2d
            }
    }
}

private func isBoundedToken(
    _ value: String,
    minimum: Int,
    maximum: Int
) -> Bool {
    guard (minimum ... maximum).contains(value.utf8.count),
          value.unicodeScalars.first?.properties.isWhitespace != true,
          value.unicodeScalars.last?.properties.isWhitespace != true
    else {
        return false
    }
    return value.unicodeScalars.allSatisfy {
        $0.properties.generalCategory != .control
    }
}

private func isValidRegistry(_ registry: TohsenoRegistryReference) -> Bool {
    registry.chainID == 4_663
        && isNonzeroLowerHex(registry.contract, bytes: 20)
        && (registry.transaction.map(isNonzeroHex32) ?? true)
}

private func isNonzeroLowerHex(_ value: String, bytes: Int) -> Bool {
    isLowerHex(value, bytes: bytes)
        && value.dropFirst(2).contains { $0 != "0" }
}

private func isValidAppStoreID(_ value: UInt64) -> Bool {
    value > 0 && value <= 9_007_199_254_740_991
}

private func isLowerHex(_ value: String, bytes: Int) -> Bool {
    guard value.utf8.count == 2 + bytes * 2, value.hasPrefix("0x") else {
        return false
    }
    return value.dropFirst(2).allSatisfy {
        ("0" ... "9").contains($0) || ("a" ... "f").contains($0)
    }
}

private func isLowerUnprefixedHex(_ value: String, digits: Int) -> Bool {
    value.utf8.count == digits && value.allSatisfy {
        ("0" ... "9").contains($0) || ("a" ... "f").contains($0)
    }
}

private func validateMetadataJSONShape(_ data: Data) throws {
    guard let root = try JSONSerialization.jsonObject(
        with: data,
        options: [.fragmentsAllowed]
    ) as? [String: Any] else {
        throw TohsenoMetadataError.invalid
    }
    try requireMetadataKeys(
        root,
        required: [
            "protocol", "schema", "fascia", "shot_id", "builder_id",
            "sequence", "evolution_commitment", "source_tree_sha256",
            "fascia_sha256", "bundle_id", "bundle_version", "factory",
            "distribution", "capabilities", "network",
        ],
        optional: ["previous", "origin", "registry"]
    )
    guard let factory = root["factory"] as? [String: Any],
          let distribution = root["distribution"] as? [String: Any],
          let capabilities = root["capabilities"] as? [Any],
          let network = root["network"] as? [Any]
    else {
        throw TohsenoMetadataError.invalid
    }
    try requireMetadataKeys(
        factory,
        required: ["implementation", "version", "source_commit"]
    )
    try requireMetadataKeys(
        distribution,
        required: ["state", "supported_apple_surfaces"],
        optional: ["app_store_id"]
    )
    for value in capabilities {
        guard let declaration = value as? [String: Any] else {
            throw TohsenoMetadataError.invalid
        }
        try requireMetadataKeys(
            declaration,
            required: ["capability", "purpose", "details"]
        )
    }
    for value in network {
        guard let declaration = value as? [String: Any] else {
            throw TohsenoMetadataError.invalid
        }
        try requireMetadataKeys(
            declaration,
            required: ["endpoint", "purpose"]
        )
    }
    if let value = root["origin"], !(value is NSNull) {
        guard let origin = value as? [String: Any] else {
            throw TohsenoMetadataError.invalid
        }
        try requireMetadataKeys(
            origin,
            required: ["kind", "legacy_latest_shot", "legacy_source_sha256"]
        )
    }
    if let value = root["registry"], !(value is NSNull) {
        guard let registry = value as? [String: Any] else {
            throw TohsenoMetadataError.invalid
        }
        try requireMetadataKeys(
            registry,
            required: ["chain_id", "contract"],
            optional: ["transaction"]
        )
    }
}

private func requireMetadataKeys(
    _ object: [String: Any],
    required: Set<String>,
    optional: Set<String> = []
) throws {
    let observed = Set(object.keys)
    guard required.isSubset(of: observed),
          observed.isSubset(of: required.union(optional))
    else {
        throw TohsenoMetadataError.invalid
    }
}
