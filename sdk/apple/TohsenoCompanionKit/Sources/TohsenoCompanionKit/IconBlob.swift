import CryptoKit
import Foundation

/// Exact private image bytes for an icon descriptor.
///
/// Icon blobs are valid only inside the recipient-encrypted companion event
/// stream. `bytes` never appears in relay routing metadata.
public struct CompanionIconBlob: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-icon-blob/1"
    public static let maximumByteLength = 2 * 1024 * 1024
    public static let maximumDimension: UInt32 = 2048

    public let schema: String
    public let blobID: String
    public let revision: UInt64
    public let mediaType: String
    public let byteLength: UInt64
    public let width: UInt32
    public let height: UInt32
    public let placeholder: Bool
    public let sha256: String
    public let bytes: Data

    private enum CodingKeys: String, CodingKey {
        case schema
        case blobID = "blob_id"
        case revision
        case mediaType = "media_type"
        case byteLength = "byte_length"
        case width, height, placeholder, sha256, bytes
    }

    public init(
        blobID: String,
        revision: UInt64,
        mediaType: String,
        placeholder: Bool,
        bytes: Data
    ) throws {
        let dimensions = try Self.imageDimensions(mediaType: mediaType, bytes: bytes)
        schema = Self.schemaV1
        self.blobID = blobID
        self.revision = revision
        self.mediaType = mediaType
        byteLength = UInt64(bytes.count)
        width = dimensions.width
        height = dimensions.height
        self.placeholder = placeholder
        sha256 = Base64URL.encode(bytes.companionSHA256)
        self.bytes = bytes
        try validate()
    }

    public init(descriptor: IconDescriptor, bytes: Data) throws {
        try self.init(
            blobID: descriptor.blobID,
            revision: descriptor.revision,
            mediaType: descriptor.mediaType,
            placeholder: descriptor.placeholder,
            bytes: bytes
        )
        try matches(descriptor)
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "blob_id", "revision", "media_type", "byte_length", "width", "height",
            "placeholder", "sha256", "bytes",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        blobID = try container.decode(String.self, forKey: .blobID)
        revision = try container.decode(UInt64.self, forKey: .revision)
        mediaType = try container.decode(String.self, forKey: .mediaType)
        byteLength = try container.decode(UInt64.self, forKey: .byteLength)
        width = try container.decode(UInt32.self, forKey: .width)
        height = try container.decode(UInt32.self, forKey: .height)
        placeholder = try container.decode(Bool.self, forKey: .placeholder)
        sha256 = try container.decode(String.self, forKey: .sha256)
        bytes = try Base64URL.decode(container.decode(String.self, forKey: .bytes))
        try validate()
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(schema, forKey: .schema)
        try container.encode(blobID, forKey: .blobID)
        try container.encode(revision, forKey: .revision)
        try container.encode(mediaType, forKey: .mediaType)
        try container.encode(byteLength, forKey: .byteLength)
        try container.encode(width, forKey: .width)
        try container.encode(height, forKey: .height)
        try container.encode(placeholder, forKey: .placeholder)
        try container.encode(sha256, forKey: .sha256)
        try container.encode(Base64URL.encode(bytes), forKey: .bytes)
    }

    public func validate() throws {
        guard schema == Self.schemaV1, revision > 0,
              mediaType == "image/png" || mediaType == "image/jpeg",
              (1 ... UInt64(Self.maximumByteLength)).contains(byteLength),
              byteLength == UInt64(bytes.count),
              (1 ... Self.maximumDimension).contains(width),
              (1 ... Self.maximumDimension).contains(height)
        else { throw TohsenoCompanionError.invalidEncoding("invalid icon blob metadata") }
        try requireIdentifier(blobID, field: "icon_blob.blob_id")
        guard Base64URL.encode(bytes.companionSHA256) == sha256 else {
            throw TohsenoCompanionError.invalidEncoding("icon blob SHA-256 differs")
        }
        let dimensions = try Self.imageDimensions(mediaType: mediaType, bytes: bytes)
        guard dimensions == (width, height) else {
            throw TohsenoCompanionError.invalidEncoding("icon blob dimensions differ")
        }
    }

    public func matches(_ descriptor: IconDescriptor) throws {
        try validate()
        try descriptor.validate()
        guard blobID == descriptor.blobID, revision == descriptor.revision,
              mediaType == descriptor.mediaType, byteLength == descriptor.byteLength,
              width == descriptor.width, height == descriptor.height,
              placeholder == descriptor.placeholder
        else { throw TohsenoCompanionError.invalidEncoding("icon blob descriptor differs") }
    }

    private static func imageDimensions(mediaType: String, bytes: Data) throws -> (width: UInt32, height: UInt32) {
        let values = [UInt8](bytes)
        let result: (UInt32, UInt32)? = switch mediaType {
        case "image/png": pngDimensions(values)
        case "image/jpeg": jpegDimensions(values)
        default: nil
        }
        guard let result,
              (1 ... maximumDimension).contains(result.0),
              (1 ... maximumDimension).contains(result.1)
        else { throw TohsenoCompanionError.invalidEncoding("invalid or oversized icon image") }
        return result
    }

    private static func pngDimensions(_ bytes: [UInt8]) -> (UInt32, UInt32)? {
        let signature: [UInt8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        guard bytes.count >= 24, Array(bytes[0 ..< 8]) == signature,
              read32(bytes, at: 8) == 13, Array(bytes[12 ..< 16]) == Array("IHDR".utf8)
        else { return nil }
        return (read32(bytes, at: 16), read32(bytes, at: 20))
    }

    private static func jpegDimensions(_ bytes: [UInt8]) -> (UInt32, UInt32)? {
        guard bytes.count >= 4, bytes[0] == 0xff, bytes[1] == 0xd8 else { return nil }
        var offset = 2
        while offset < bytes.count {
            guard bytes[offset] == 0xff else { return nil }
            while offset < bytes.count, bytes[offset] == 0xff { offset += 1 }
            guard offset < bytes.count else { return nil }
            let marker = bytes[offset]
            offset += 1
            if marker == 0 || marker == 0xd9 || marker == 0xda { return nil }
            if marker == 0x01 || (0xd0 ... 0xd8).contains(marker) { continue }
            guard offset + 1 < bytes.count else { return nil }
            let length = Int(bytes[offset]) << 8 | Int(bytes[offset + 1])
            guard length >= 2, offset <= bytes.count - length else { return nil }
            let start = offset + 2
            let frameMarkers: Set<UInt8> = [
                0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
            ]
            if frameMarkers.contains(marker) {
                guard length >= 7, start + 4 < bytes.count else { return nil }
                let height = UInt32(bytes[start + 1]) << 8 | UInt32(bytes[start + 2])
                let width = UInt32(bytes[start + 3]) << 8 | UInt32(bytes[start + 4])
                return (width, height)
            }
            offset += length
        }
        return nil
    }

    private static func read32(_ bytes: [UInt8], at offset: Int) -> UInt32 {
        UInt32(bytes[offset]) << 24 | UInt32(bytes[offset + 1]) << 16
            | UInt32(bytes[offset + 2]) << 8 | UInt32(bytes[offset + 3])
    }
}
