import CryptoKit
import Foundation

/// Exact owner-supplied image bytes for a create or evolve request.
///
/// The logical object is never relay metadata. CompanionKit deterministically
/// divides it into signed, recipient-encrypted transport chunks before it
/// queues the command descriptor that commits to these exact bytes.
public struct CompanionReferenceBlob: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-reference-blob/1"
    public static let maximumByteLength = 64 * 1024 * 1024
    public static let maximumChunkByteLength = 8 * 1024 * 1024

    public let schema: String
    public let blobID: String
    public let originName: String
    public let mediaType: String
    public let byteLength: UInt64
    public let sha256: String
    public let bytes: Data

    private enum CodingKeys: String, CodingKey {
        case schema
        case blobID = "blob_id"
        case originName = "origin_name"
        case mediaType = "media_type"
        case byteLength = "byte_length"
        case sha256, bytes
    }

    public init(
        blobID: String? = nil,
        originName: String,
        mediaType: String,
        bytes: Data
    ) throws {
        schema = Self.schemaV1
        self.originName = originName
        self.mediaType = mediaType
        byteLength = UInt64(bytes.count)
        sha256 = Base64URL.encode(bytes.companionSHA256)
        self.bytes = bytes
        self.blobID = blobID ?? Self.stableBlobID(
            originName: originName,
            mediaType: mediaType,
            bytes: bytes
        )
        try validate()
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "blob_id", "origin_name", "media_type", "byte_length", "sha256", "bytes",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        blobID = try container.decode(String.self, forKey: .blobID)
        originName = try container.decode(String.self, forKey: .originName)
        mediaType = try container.decode(String.self, forKey: .mediaType)
        byteLength = try container.decode(UInt64.self, forKey: .byteLength)
        sha256 = try container.decode(String.self, forKey: .sha256)
        let encodedBytes = try container.decode(String.self, forKey: .bytes)
        guard encodedBytes.utf8.count <= Self.maximumEncodedLength(Self.maximumByteLength) else {
            throw TohsenoCompanionError.responseTooLarge
        }
        bytes = try Base64URL.decode(encodedBytes)
        try validate()
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(schema, forKey: .schema)
        try container.encode(blobID, forKey: .blobID)
        try container.encode(originName, forKey: .originName)
        try container.encode(mediaType, forKey: .mediaType)
        try container.encode(byteLength, forKey: .byteLength)
        try container.encode(sha256, forKey: .sha256)
        try container.encode(Base64URL.encode(bytes), forKey: .bytes)
    }

    public var descriptor: CompanionReferenceDescriptor {
        CompanionReferenceDescriptor(
            blobID: blobID,
            originName: originName,
            mediaType: mediaType,
            byteLength: byteLength,
            sha256: sha256
        )
    }

    public func validate() throws {
        guard schema == Self.schemaV1,
              (1 ... UInt64(Self.maximumByteLength)).contains(byteLength),
              byteLength == UInt64(bytes.count),
              Base64URL.encode(bytes.companionSHA256) == sha256
        else { throw TohsenoCompanionError.invalidEncoding("invalid reference blob commitment") }
        try Self.validateMetadata(
            blobID: blobID,
            originName: originName,
            mediaType: mediaType,
            byteLength: byteLength,
            sha256: sha256
        )
        try Self.validateImage(mediaType: mediaType, bytes: bytes)
    }

    public func transportChunks() throws -> [CompanionReferenceBlobChunk] {
        try validate()
        let count = bytes.count.quotientAndRemainder(dividingBy: Self.maximumChunkByteLength)
        let chunkCount = count.quotient + (count.remainder == 0 ? 0 : 1)
        guard (1 ... 8).contains(chunkCount) else {
            throw TohsenoCompanionError.invalidEncoding("reference chunk count")
        }
        return try (0 ..< chunkCount).map { index in
            let lower = index * Self.maximumChunkByteLength
            let upper = min(lower + Self.maximumChunkByteLength, bytes.count)
            return try CompanionReferenceBlobChunk(
                blob: self,
                chunkIndex: UInt64(index),
                chunkCount: UInt64(chunkCount),
                bytes: bytes.subdata(in: lower ..< upper)
            )
        }
    }

    static func validateMetadata(
        blobID: String,
        originName: String,
        mediaType: String,
        byteLength: UInt64,
        sha256: String
    ) throws {
        try requireIdentifier(blobID, field: "reference.blob_id")
        try requireBoundedText(originName, field: "reference.origin_name", maximum: 512)
        guard originName != ".", originName != "..",
              !originName.contains("/"), !originName.contains("\\"), !originName.contains("\0")
        else { throw TohsenoCompanionError.invalidEncoding("reference origin must be a filename") }
        guard mediaType == "image/png" || mediaType == "image/jpeg",
              (1 ... UInt64(maximumByteLength)).contains(byteLength)
        else { throw TohsenoCompanionError.invalidEncoding("reference media type or size") }
        _ = try Base64URL.decode(sha256, expectedBytes: 32)
    }

    private static func stableBlobID(originName: String, mediaType: String, bytes: Data) -> String {
        var committed = Data("tohseno.companion.reference-blob-id.v1".utf8)
        committed.append(0)
        committed.append(Data(originName.utf8))
        committed.append(0)
        committed.append(Data(mediaType.utf8))
        committed.append(0)
        committed.append(bytes)
        return "reference_" + Base64URL.encode(committed.companionSHA256)
    }

    static func maximumEncodedLength(_ byteLength: Int) -> Int {
        byteLength / 3 * 4 + (byteLength % 3 == 0 ? 0 : byteLength % 3 + 1)
    }

    private static func validateImage(mediaType: String, bytes: Data) throws {
        let values = [UInt8](bytes)
        let valid = switch mediaType {
        case "image/png": validPNG(values)
        case "image/jpeg": validJPEG(values)
        default: false
        }
        guard valid else {
            throw TohsenoCompanionError.invalidEncoding("reference bytes differ from declared image type")
        }
    }

    private static func validPNG(_ bytes: [UInt8]) -> Bool {
        let signature: [UInt8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
        guard bytes.count >= 24, Array(bytes[0 ..< 8]) == signature,
              read32(bytes, at: 8) == 13, Array(bytes[12 ..< 16]) == Array("IHDR".utf8)
        else { return false }
        return read32(bytes, at: 16) > 0 && read32(bytes, at: 20) > 0
    }

    private static func validJPEG(_ bytes: [UInt8]) -> Bool {
        guard bytes.count >= 4, bytes[0] == 0xff, bytes[1] == 0xd8 else { return false }
        var offset = 2
        while offset < bytes.count {
            guard bytes[offset] == 0xff else { return false }
            while offset < bytes.count, bytes[offset] == 0xff { offset += 1 }
            guard offset < bytes.count else { return false }
            let marker = bytes[offset]
            offset += 1
            if marker == 0 || marker == 0xd9 || marker == 0xda { return false }
            if marker == 0x01 || (0xd0 ... 0xd8).contains(marker) { continue }
            guard offset + 1 < bytes.count else { return false }
            let length = Int(bytes[offset]) << 8 | Int(bytes[offset + 1])
            guard length >= 2, offset <= bytes.count - length else { return false }
            let start = offset + 2
            let frames: Set<UInt8> = [
                0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
            ]
            if frames.contains(marker) {
                guard length >= 7, start + 4 < bytes.count else { return false }
                let height = UInt16(bytes[start + 1]) << 8 | UInt16(bytes[start + 2])
                let width = UInt16(bytes[start + 3]) << 8 | UInt16(bytes[start + 4])
                return width > 0 && height > 0
            }
            offset += length
        }
        return false
    }

    private static func read32(_ bytes: [UInt8], at offset: Int) -> UInt32 {
        UInt32(bytes[offset]) << 24 | UInt32(bytes[offset + 1]) << 16
            | UInt32(bytes[offset + 2]) << 8 | UInt32(bytes[offset + 3])
    }
}

/// One canonical transport unit. A companion envelope signs and authenticates
/// this exact object; it is not a command and never enters public lineage.
public struct CompanionReferenceBlobChunk: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-reference-blob-chunk/1"

    public let schema: String
    public let blobID: String
    public let originName: String
    public let mediaType: String
    public let byteLength: UInt64
    public let sha256: String
    public let chunkIndex: UInt64
    public let chunkCount: UInt64
    public let chunkByteLength: UInt64
    public let chunkSHA256: String
    public let bytes: Data

    private enum CodingKeys: String, CodingKey {
        case schema
        case blobID = "blob_id"
        case originName = "origin_name"
        case mediaType = "media_type"
        case byteLength = "byte_length"
        case sha256
        case chunkIndex = "chunk_index"
        case chunkCount = "chunk_count"
        case chunkByteLength = "chunk_byte_length"
        case chunkSHA256 = "chunk_sha256"
        case bytes
    }

    init(blob: CompanionReferenceBlob, chunkIndex: UInt64, chunkCount: UInt64, bytes: Data) throws {
        schema = Self.schemaV1
        blobID = blob.blobID
        originName = blob.originName
        mediaType = blob.mediaType
        byteLength = blob.byteLength
        sha256 = blob.sha256
        self.chunkIndex = chunkIndex
        self.chunkCount = chunkCount
        chunkByteLength = UInt64(bytes.count)
        chunkSHA256 = Base64URL.encode(bytes.companionSHA256)
        self.bytes = bytes
        try validate()
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "blob_id", "origin_name", "media_type", "byte_length", "sha256",
            "chunk_index", "chunk_count", "chunk_byte_length", "chunk_sha256", "bytes",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        blobID = try container.decode(String.self, forKey: .blobID)
        originName = try container.decode(String.self, forKey: .originName)
        mediaType = try container.decode(String.self, forKey: .mediaType)
        byteLength = try container.decode(UInt64.self, forKey: .byteLength)
        sha256 = try container.decode(String.self, forKey: .sha256)
        chunkIndex = try container.decode(UInt64.self, forKey: .chunkIndex)
        chunkCount = try container.decode(UInt64.self, forKey: .chunkCount)
        chunkByteLength = try container.decode(UInt64.self, forKey: .chunkByteLength)
        chunkSHA256 = try container.decode(String.self, forKey: .chunkSHA256)
        let encodedBytes = try container.decode(String.self, forKey: .bytes)
        guard encodedBytes.utf8.count <= CompanionReferenceBlob.maximumEncodedLength(
            CompanionReferenceBlob.maximumChunkByteLength
        ) else { throw TohsenoCompanionError.responseTooLarge }
        bytes = try Base64URL.decode(encodedBytes)
        try validate()
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(schema, forKey: .schema)
        try container.encode(blobID, forKey: .blobID)
        try container.encode(originName, forKey: .originName)
        try container.encode(mediaType, forKey: .mediaType)
        try container.encode(byteLength, forKey: .byteLength)
        try container.encode(sha256, forKey: .sha256)
        try container.encode(chunkIndex, forKey: .chunkIndex)
        try container.encode(chunkCount, forKey: .chunkCount)
        try container.encode(chunkByteLength, forKey: .chunkByteLength)
        try container.encode(chunkSHA256, forKey: .chunkSHA256)
        try container.encode(Base64URL.encode(bytes), forKey: .bytes)
    }

    public func validate() throws {
        guard schema == Self.schemaV1 else {
            throw TohsenoCompanionError.invalidEncoding("reference chunk schema")
        }
        try CompanionReferenceBlob.validateMetadata(
            blobID: blobID,
            originName: originName,
            mediaType: mediaType,
            byteLength: byteLength,
            sha256: sha256
        )
        let maximum = UInt64(CompanionReferenceBlob.maximumChunkByteLength)
        let expectedCount = (byteLength + maximum - 1) / maximum
        guard chunkCount == expectedCount, (1 ... 8).contains(chunkCount), chunkIndex < chunkCount else {
            throw TohsenoCompanionError.invalidEncoding("reference chunk position")
        }
        let expectedLength = chunkIndex + 1 == chunkCount
            ? ((byteLength % maximum == 0) ? maximum : byteLength % maximum)
            : maximum
        guard chunkByteLength == expectedLength, chunkByteLength == UInt64(bytes.count),
              Base64URL.encode(bytes.companionSHA256) == chunkSHA256
        else { throw TohsenoCompanionError.invalidEncoding("reference chunk commitment") }
    }
}

public enum CompanionReferenceChunkAdmission: Equatable, Sendable {
    case stored
    case duplicate
    case complete(CompanionReferenceBlob)
}

/// Deterministic reassembly law shared by conformance clients and the Mac.
public struct CompanionReferenceBlobAssembler: Sendable {
    private var chunks: [UInt64: CompanionReferenceBlobChunk] = [:]

    public init() {}

    public mutating func admit(_ chunk: CompanionReferenceBlobChunk) throws -> CompanionReferenceChunkAdmission {
        try chunk.validate()
        if let first = chunks.values.first {
            guard chunk.blobID == first.blobID, chunk.originName == first.originName,
                  chunk.mediaType == first.mediaType, chunk.byteLength == first.byteLength,
                  chunk.sha256 == first.sha256, chunk.chunkCount == first.chunkCount
            else { throw TohsenoCompanionError.invalidEncoding("reference chunks describe different blobs") }
        }
        if let existing = chunks[chunk.chunkIndex] {
            guard existing == chunk else {
                throw TohsenoCompanionError.invalidEncoding("reference chunk index reuse")
            }
            return .duplicate
        }
        chunks[chunk.chunkIndex] = chunk
        guard chunks.count == Int(chunk.chunkCount) else { return .stored }
        var bytes = Data()
        bytes.reserveCapacity(Int(chunk.byteLength))
        for index in 0 ..< chunk.chunkCount {
            guard let value = chunks[index] else {
                throw TohsenoCompanionError.invalidEncoding("reference chunk missing")
            }
            bytes.append(value.bytes)
        }
        let blob = try CompanionReferenceBlob(
            blobID: chunk.blobID,
            originName: chunk.originName,
            mediaType: chunk.mediaType,
            bytes: bytes
        )
        guard blob.sha256 == chunk.sha256, blob.byteLength == chunk.byteLength else {
            throw TohsenoCompanionError.invalidEncoding("reference whole-object commitment")
        }
        return .complete(blob)
    }
}
