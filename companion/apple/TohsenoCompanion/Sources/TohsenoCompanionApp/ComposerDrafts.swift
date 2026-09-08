import Foundation
import CryptoKit
import TohsenoCompanionKit

struct ComposerDraft: Codable, Equatable {
    var intention: String
    var name: String
    var base: ShotSummary?
    var images: [DraftImage]

    struct DraftImage: Codable, Equatable {
        let blobID: String
        let originName: String
        let mediaType: String
        let asset: String
    }
}

@MainActor
protocol ComposerDraftStorage {
    func contains(_ key: String) -> Bool
    func draft(for key: String) throws -> (ComposerDraft, [CompanionReferenceBlob])?
    func save(key: String, intention: String, name: String, base: ShotSummary?, images: [CompanionReferenceBlob]) throws
    func remove(key: String) throws
}

/// Local-only drafts. iOS Data Protection protects both the atomic index and
/// original image files. Images are written once, never rewritten per keystroke.
@MainActor
final class FileComposerDraftStorage: ComposerDraftStorage {
    private let directory: URL
    private var drafts: [String: ComposerDraft]

    init(directory: URL) throws {
        self.directory = directory
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var protectedDirectory = directory
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try protectedDirectory.setResourceValues(values)
        let index = directory.appendingPathComponent("drafts.json")
        if FileManager.default.fileExists(atPath: index.path) {
            drafts = try JSONDecoder().decode([String: ComposerDraft].self, from: Data(contentsOf: index))
        } else { drafts = [:] }
    }

    func contains(_ key: String) -> Bool { drafts[key] != nil }

    func draft(for key: String) throws -> (ComposerDraft, [CompanionReferenceBlob])? {
        guard let draft = drafts[key] else { return nil }
        let images = try draft.images.map { image in
            let bytes = try Data(contentsOf: assetURL(image.asset))
            guard Self.digest(bytes) == image.asset else { throw TohsenoCompanionError.unsafeStorage }
            return try CompanionReferenceBlob(blobID: image.blobID, originName: image.originName,
                mediaType: image.mediaType, bytes: bytes)
        }
        return (draft, images)
    }

    func save(key: String, intention: String, name: String, base: ShotSummary?, images: [CompanionReferenceBlob]) throws {
        var descriptors: [ComposerDraft.DraftImage] = []
        for image in images {
            let encoded = image.sha256.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
            guard let digest = Data(base64Encoded: encoded + String(repeating: "=", count: (4 - encoded.count % 4) % 4)), digest.count == 32 else {
                throw TohsenoCompanionError.unsafeStorage
            }
            let asset = digest.map { String(format: "%02x", $0) }.joined()
            let url = try assetURL(asset)
            if !FileManager.default.fileExists(atPath: url.path) {
                try image.bytes.write(to: url, options: [.atomic, .completeFileProtection])
            }
            descriptors.append(.init(blobID: image.blobID, originName: image.originName,
                mediaType: image.mediaType, asset: asset))
        }
        var next = drafts
        if intention.isEmpty && name.isEmpty && images.isEmpty { next.removeValue(forKey: key) }
        else { next[key] = ComposerDraft(intention: intention, name: name, base: base, images: descriptors) }
        try commit(next)
    }

    func remove(key: String) throws {
        var next = drafts
        next.removeValue(forKey: key)
        try commit(next)
    }

    private func commit(_ next: [String: ComposerDraft]) throws {
        try JSONEncoder().encode(next).write(to: directory.appendingPathComponent("drafts.json"),
                                            options: [.atomic, .completeFileProtection])
        let previousAssets = Set(drafts.values.flatMap { $0.images.map(\.asset) })
        let retained = Set(next.values.flatMap { $0.images.map(\.asset) })
        drafts = next
        // Remove only unreferenced assets after the new index is durable.
        for asset in previousAssets.subtracting(retained) {
            if let url = try? assetURL(asset) { try? FileManager.default.removeItem(at: url) }
        }
    }

    private func assetURL(_ name: String) throws -> URL {
        guard name.count == 64, name.allSatisfy({ $0.isHexDigit && !$0.isUppercase }) else {
            throw TohsenoCompanionError.unsafeStorage
        }
        return directory.appendingPathComponent(name + ".image")
    }

    private static func digest(_ bytes: Data) -> String {
        SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
    }
}

@MainActor
final class MemoryComposerDraftStorage: ComposerDraftStorage {
    var values: [String: (ComposerDraft, [CompanionReferenceBlob])] = [:]
    func contains(_ key: String) -> Bool { values[key] != nil }
    func draft(for key: String) -> (ComposerDraft, [CompanionReferenceBlob])? { values[key] }
    func save(key: String, intention: String, name: String, base: ShotSummary?, images: [CompanionReferenceBlob]) {
        if intention.isEmpty && name.isEmpty && images.isEmpty { values.removeValue(forKey: key) }
        else { values[key] = (ComposerDraft(intention: intention, name: name, base: base, images: []), images) }
    }
    func remove(key: String) { values.removeValue(forKey: key) }
}
