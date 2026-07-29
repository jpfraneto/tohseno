import Foundation

public struct Provenance: Equatable, Sendable {
    public let metadata: TohsenoMetadata

    public init(metadata: TohsenoMetadata) throws {
        try metadata.validate()
        self.metadata = metadata
    }

    public static func current(
        bundle: Bundle = .main
    ) throws -> Provenance {
        let metadata = try TohsenoMetadata.loadEmbedded(from: bundle)
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
        metadata.sequence
    }

    public var evolutionCommitment: String {
        metadata.evolutionCommitment
    }

    public var isPublished: Bool {
        metadata.distribution.state != .local && metadata.registry != nil
    }
}
