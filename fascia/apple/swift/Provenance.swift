import Foundation

public struct Provenance: Equatable, Sendable {
    public let metadata: TohsenoEmbeddedMetadata

    public init(metadata: TohsenoMetadata) throws {
        try metadata.validate()
        self.metadata = .v1(metadata)
    }

    public init(metadata: TohsenoMetadataV2) throws {
        try metadata.validate()
        self.metadata = .v2(metadata)
    }

    public init(metadata: TohsenoEmbeddedMetadata) throws {
        switch metadata {
        case let .v1(value):
            try value.validate()
        case let .v2(value):
            try value.validate()
        }
        self.metadata = metadata
    }

    public static func current(
        bundle: Bundle = .main
    ) throws -> Provenance {
        let metadata = try TohsenoEmbeddedMetadata.loadEmbedded(from: bundle)
        guard bundle.bundleIdentifier == metadata.bundleID,
              let rawVersion = bundle.object(
                  forInfoDictionaryKey: "CFBundleVersion"
              ) as? String,
              UInt32(rawVersion) == metadata.bundleVersion
        else {
            throw TohsenoMetadataError.bundleMismatch
        }
        return try Provenance(metadata: metadata)
    }

    public var shotID: String {
        metadata.shotID
    }

    public var builderID: String {
        metadata.builderID
    }

    public var evolution: UInt32 {
        metadata.evolution
    }

    public var evolutionCommitment: String {
        metadata.lineageHead
    }

    public var expressionID: String? {
        metadata.expressionID
    }

    public var versionID: String? {
        metadata.versionID
    }

    public var genomeRevision: UInt64? {
        metadata.genomeRevision
    }

    public var genomeDigest: String? {
        metadata.genomeDigest
    }

    public var isPublished: Bool {
        metadata.distribution.state != .local && metadata.registry != nil
    }
}
