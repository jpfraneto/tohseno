import CryptoKit
import Foundation

public protocol CompanionStateStore: Sendable {
    func load() async throws -> Data?
    func save(_ bytes: Data) async throws
    func delete() async throws
}

/// Separately bounded durable storage for already encrypted outbound payloads.
/// Keeping large reference chunks out of the monolithic state record avoids
/// loading a possible eight-reference outbox into memory on every state write.
public protocol CompanionPayloadStore: Sendable {
    func load(id: String) async throws -> Data?
    func save(id: String, bytes: Data) async throws
    func delete(id: String) async throws
    func retainOnly(ids: Set<String>) async throws
    func deleteAll() async throws
}

public actor InMemoryCompanionPayloadStore: CompanionPayloadStore {
    private var values: [String: Data]

    public init(values: [String: Data] = [:]) { self.values = values }

    public func load(id: String) throws -> Data? {
        try Self.validate(id: id)
        return values[id]
    }

    public func save(id: String, bytes: Data) throws {
        try Self.validate(id: id)
        try Self.validate(bytes: bytes)
        if let existing = values[id], existing != bytes {
            throw TohsenoCompanionError.unsafeStorage
        }
        values[id] = bytes
    }

    public func delete(id: String) throws {
        try Self.validate(id: id)
        values.removeValue(forKey: id)
    }

    public func retainOnly(ids: Set<String>) throws {
        for id in ids { try Self.validate(id: id) }
        values = values.filter { ids.contains($0.key) }
    }

    public func deleteAll() { values = [:] }

    private static func validate(id: String) throws {
        try requireIdentifier(id, field: "payload_store.id")
    }

    private static func validate(bytes: Data) throws {
        guard !bytes.isEmpty, bytes.count <= CompanionLimits.maximumEnvelopeBodyBytes else {
            throw TohsenoCompanionError.unsafeStorage
        }
    }
}

/// Symlink-resistant, file-protected storage for outbound encrypted envelopes.
public actor FileCompanionPayloadStore: CompanionPayloadStore {
    private let directoryURL: URL

    public init(directoryURL: URL) throws {
        guard directoryURL.isFileURL, directoryURL.lastPathComponent != ".",
              directoryURL.lastPathComponent != ".."
        else { throw TohsenoCompanionError.unsafeStorage }
        self.directoryURL = directoryURL.standardizedFileURL
    }

    public func load(id: String) throws -> Data? {
        let url = try payloadURL(id: id)
        try prepareDirectory()
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        try rejectSymbolicLink(url)
        let values = try url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        guard values.isRegularFile == true,
              let size = values.fileSize, (1 ... CompanionLimits.maximumEnvelopeBodyBytes).contains(size)
        else { throw TohsenoCompanionError.unsafeStorage }
        let bytes = try Data(contentsOf: url, options: [.mappedIfSafe])
        guard bytes.count == size else { throw TohsenoCompanionError.unsafeStorage }
        return bytes
    }

    public func save(id: String, bytes: Data) throws {
        guard !bytes.isEmpty, bytes.count <= CompanionLimits.maximumEnvelopeBodyBytes else {
            throw TohsenoCompanionError.unsafeStorage
        }
        let url = try payloadURL(id: id)
        try prepareDirectory()
        if let existing = try load(id: id) {
            guard existing == bytes else { throw TohsenoCompanionError.unsafeStorage }
            return
        }
        try bytes.write(to: url, options: [.atomic, .completeFileProtection])
        try rejectSymbolicLink(url)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }

    public func delete(id: String) throws {
        let url = try payloadURL(id: id)
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        try rejectSymbolicLink(url)
        try FileManager.default.removeItem(at: url)
    }

    public func retainOnly(ids: Set<String>) throws {
        for id in ids { try requireIdentifier(id, field: "payload_store.id") }
        try prepareDirectory()
        var count = 0
        for url in try FileManager.default.contentsOfDirectory(
            at: directoryURL,
            includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey],
            options: []
        ) {
            count += 1
            guard count <= 4096, url.pathExtension == "envelope" else {
                throw TohsenoCompanionError.unsafeStorage
            }
            try rejectSymbolicLink(url)
            let id = url.deletingPathExtension().lastPathComponent
            try requireIdentifier(id, field: "payload_store.id")
            guard ids.contains(id) else {
                try FileManager.default.removeItem(at: url)
                continue
            }
            let values = try url.resourceValues(forKeys: [.isRegularFileKey])
            guard values.isRegularFile == true else { throw TohsenoCompanionError.unsafeStorage }
        }
    }

    public func deleteAll() throws { try retainOnly(ids: []) }

    private func payloadURL(id: String) throws -> URL {
        try requireIdentifier(id, field: "payload_store.id")
        return directoryURL.appendingPathComponent(id + ".envelope", isDirectory: false)
    }

    private func prepareDirectory() throws {
        if FileManager.default.fileExists(atPath: directoryURL.path) {
            try rejectSymbolicLink(directoryURL)
            let values = try directoryURL.resourceValues(forKeys: [.isDirectoryKey])
            guard values.isDirectory == true else { throw TohsenoCompanionError.unsafeStorage }
        } else {
            try FileManager.default.createDirectory(
                at: directoryURL,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            try rejectSymbolicLink(directoryURL)
        }
    }

    private func rejectSymbolicLink(_ url: URL) throws {
        let values = try url.resourceValues(forKeys: [.isSymbolicLinkKey])
        guard values.isSymbolicLink != true else { throw TohsenoCompanionError.unsafeStorage }
    }
}

public actor InMemoryCompanionStateStore: CompanionStateStore {
    private var bytes: Data?

    public init(bytes: Data? = nil) { self.bytes = bytes }
    public func load() -> Data? { bytes }
    public func save(_ bytes: Data) { self.bytes = bytes }
    public func delete() { bytes = nil }
}

public actor FileCompanionStateStore: CompanionStateStore {
    private let fileURL: URL
    private let directoryURL: URL

    public init(fileURL: URL) throws {
        guard fileURL.isFileURL, fileURL.lastPathComponent != ".", fileURL.lastPathComponent != ".." else {
            throw TohsenoCompanionError.unsafeStorage
        }
        self.fileURL = fileURL.standardizedFileURL
        directoryURL = fileURL.deletingLastPathComponent().standardizedFileURL
    }

    public func load() throws -> Data? {
        try prepareDirectory()
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return nil }
        try rejectSymbolicLink(fileURL)
        let bytes = try Data(contentsOf: fileURL, options: [.mappedIfSafe])
        guard !bytes.isEmpty, bytes.count <= 64 * 1024 * 1024 else {
            throw TohsenoCompanionError.unsafeStorage
        }
        return bytes
    }

    public func save(_ bytes: Data) throws {
        guard !bytes.isEmpty, bytes.count <= 64 * 1024 * 1024 else {
            throw TohsenoCompanionError.unsafeStorage
        }
        try prepareDirectory()
        if FileManager.default.fileExists(atPath: fileURL.path) { try rejectSymbolicLink(fileURL) }
        try bytes.write(to: fileURL, options: [.atomic, .completeFileProtection])
        try rejectSymbolicLink(fileURL)
    }

    public func delete() throws {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return }
        try rejectSymbolicLink(fileURL)
        try FileManager.default.removeItem(at: fileURL)
    }

    private func prepareDirectory() throws {
        if FileManager.default.fileExists(atPath: directoryURL.path) {
            try rejectSymbolicLink(directoryURL)
            let values = try directoryURL.resourceValues(forKeys: [.isDirectoryKey])
            guard values.isDirectory == true else { throw TohsenoCompanionError.unsafeStorage }
        } else {
            try FileManager.default.createDirectory(
                at: directoryURL,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            try rejectSymbolicLink(directoryURL)
        }
    }

    private func rejectSymbolicLink(_ url: URL) throws {
        let values = try url.resourceValues(forKeys: [.isSymbolicLinkKey])
        guard values.isSymbolicLink != true else { throw TohsenoCompanionError.unsafeStorage }
    }
}

public struct CompanionInboxAccess: Codable, Equatable, Sendable {
    public let mailboxID: String
    public let readCapability: String
    public let acknowledgementCapability: String
    public let revocationCapability: String
    public let pushCapability: String

    public init(
        mailboxID: String,
        readCapability: String,
        acknowledgementCapability: String,
        revocationCapability: String,
        pushCapability: String
    ) {
        self.mailboxID = mailboxID
        self.readCapability = readCapability
        self.acknowledgementCapability = acknowledgementCapability
        self.revocationCapability = revocationCapability
        self.pushCapability = pushCapability
    }
}

public struct CompanionOutboxAccess: Codable, Equatable, Sendable {
    public let mailboxID: String
    public let writeCapability: String

    public init(mailboxID: String, writeCapability: String) {
        self.mailboxID = mailboxID
        self.writeCapability = writeCapability
    }
}

public struct CompanionPairingCompletion: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-pairing-completion/1"
    public let schema: String
    public let capabilityGrant: CapabilityGrant
    public let studioAgreementPublicKey: String
    public let inbox: CompanionInboxAccess
    public let outbox: CompanionOutboxAccess

    public init(
        schema: String = schemaV1,
        capabilityGrant: CapabilityGrant,
        studioAgreementPublicKey: String,
        inbox: CompanionInboxAccess,
        outbox: CompanionOutboxAccess
    ) {
        self.schema = schema
        self.capabilityGrant = capabilityGrant
        self.studioAgreementPublicKey = studioAgreementPublicKey
        self.inbox = inbox
        self.outbox = outbox
    }
}

struct CompanionPairingRecord: Codable, Equatable, Sendable {
    let relayID: String
    let relayBaseURL: String
    let studioDeviceID: String
    let studioSigningPublicKey: String
    let studioAgreementPublicKey: String
    let grant: CapabilityGrant
    let inbox: CompanionInboxAccess
    let outbox: CompanionOutboxAccess
    var cursor: UInt64
    var nextSenderSequence: UInt64
    var revoked: Bool
}

struct PendingCompanionCommand: Codable, Equatable, Sendable {
    let command: CompanionCommand
    var envelope: OpaqueCompanionEnvelope
    var uploaded: Bool
}

struct PendingReferenceChunk: Codable, Equatable, Sendable {
    let commandID: String
    let blobID: String
    let chunkIndex: UInt64
    let chunkCount: UInt64
    let envelopeID: String
    /// An identity-key-encrypted copy of the canonical chunk payload. This is
    /// never uploaded. It lets the phone reseal the same payload after the
    /// relay's bounded envelope lifetime without retaining plaintext image
    /// bytes outside protected companion storage.
    let localPayloadID: String?
    var uploaded: Bool

    private enum CodingKeys: String, CodingKey {
        case commandID = "command_id"
        case blobID = "blob_id"
        case chunkIndex = "chunk_index"
        case chunkCount = "chunk_count"
        case envelopeID = "envelope_id"
        case localPayloadID = "local_payload_id"
        case uploaded
    }
}

struct CompanionPersistentState: Codable, Equatable, Sendable {
    static let schemaV1 = "tohseno.companion-ios-state/1"
    var schema = schemaV1
    var pairing: CompanionPairingRecord?
    var workspace: WorkspaceSnapshot?
    var outbox: [PendingCompanionCommand] = []
    var referenceOutbox: [PendingReferenceChunk] = []
    var replay = CompanionReplayProtection.State()
    /// Bounded icon cache encrypted together with companion state.
    var iconBlobs: [String: CompanionIconBlob] = [:]

    private enum CodingKeys: String, CodingKey {
        case schema, pairing, workspace, outbox, replay
        case referenceOutbox = "reference_outbox"
        case iconBlobs = "icon_blobs"
    }

    init(
        schema: String = Self.schemaV1,
        pairing: CompanionPairingRecord? = nil,
        workspace: WorkspaceSnapshot? = nil,
        outbox: [PendingCompanionCommand] = [],
        referenceOutbox: [PendingReferenceChunk] = [],
        replay: CompanionReplayProtection.State = .init(),
        iconBlobs: [String: CompanionIconBlob] = [:]
    ) {
        self.schema = schema
        self.pairing = pairing
        self.workspace = workspace
        self.outbox = outbox
        self.referenceOutbox = referenceOutbox
        self.replay = replay
        self.iconBlobs = iconBlobs
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        pairing = try container.decodeIfPresent(CompanionPairingRecord.self, forKey: .pairing)
        workspace = try container.decodeIfPresent(WorkspaceSnapshot.self, forKey: .workspace)
        outbox = try container.decodeIfPresent([PendingCompanionCommand].self, forKey: .outbox) ?? []
        referenceOutbox = try container.decodeIfPresent(
            [PendingReferenceChunk].self,
            forKey: .referenceOutbox
        ) ?? []
        replay = try container.decodeIfPresent(CompanionReplayProtection.State.self, forKey: .replay) ?? .init()
        iconBlobs = try container.decodeIfPresent([String: CompanionIconBlob].self, forKey: .iconBlobs) ?? [:]
    }
}

enum CompanionStateCodec {
    private static let magic = Data("TOHSENO-COMPANION-STATE-1\0".utf8)

    static func seal(_ state: CompanionPersistentState, key: SymmetricKey) throws -> Data {
        let plaintext = try StrictJSON.encode(state)
        let box = try ChaChaPoly.seal(plaintext, using: key, authenticating: magic)
        var result = magic
        result.append(box.combined)
        return result
    }

    static func open(_ data: Data, key: SymmetricKey) throws -> CompanionPersistentState {
        guard data.count > magic.count + 28, data.prefix(magic.count) == magic else {
            throw TohsenoCompanionError.unsafeStorage
        }
        do {
            let box = try ChaChaPoly.SealedBox(combined: data.dropFirst(magic.count))
            let plaintext = try ChaChaPoly.open(box, using: key, authenticating: magic)
            let state = try StrictJSON.decode(
                CompanionPersistentState.self,
                from: plaintext,
                maximumBytes: 32 * 1024 * 1024
            )
            guard state.schema == CompanionPersistentState.schemaV1 else {
                throw TohsenoCompanionError.unsafeStorage
            }
            return state
        } catch let error as TohsenoCompanionError {
            throw error
        } catch {
            throw TohsenoCompanionError.unsafeStorage
        }
    }
}

/// Encrypts large reference-chunk plaintext independently of the monolithic
/// companion state. The binding prevents swapping one pending command's local
/// payload into another record even if the filesystem store is tampered with.
enum CompanionLocalPayloadCodec {
    private static let magic = Data("TOHSENO-COMPANION-LOCAL-PAYLOAD-1\0".utf8)

    static func seal(_ plaintext: Data, key: SymmetricKey, binding: Data) throws -> Data {
        guard !plaintext.isEmpty,
              plaintext.count <= CompanionLimits.maximumEnvelopeBodyBytes,
              !binding.isEmpty, binding.count <= 1024
        else { throw TohsenoCompanionError.unsafeStorage }
        let box = try ChaChaPoly.seal(
            plaintext,
            using: key,
            authenticating: authenticatedBinding(binding)
        )
        var result = magic
        result.append(box.combined)
        guard result.count <= CompanionLimits.maximumEnvelopeBodyBytes else {
            throw TohsenoCompanionError.unsafeStorage
        }
        return result
    }

    static func open(_ sealed: Data, key: SymmetricKey, binding: Data) throws -> Data {
        guard sealed.count > magic.count + 28,
              sealed.count <= CompanionLimits.maximumEnvelopeBodyBytes,
              sealed.prefix(magic.count) == magic,
              !binding.isEmpty, binding.count <= 1024
        else { throw TohsenoCompanionError.unsafeStorage }
        do {
            let box = try ChaChaPoly.SealedBox(combined: sealed.dropFirst(magic.count))
            let plaintext = try ChaChaPoly.open(
                box,
                using: key,
                authenticating: authenticatedBinding(binding)
            )
            guard !plaintext.isEmpty,
                  plaintext.count <= CompanionLimits.maximumEnvelopeBodyBytes
            else { throw TohsenoCompanionError.unsafeStorage }
            return plaintext
        } catch let error as TohsenoCompanionError {
            throw error
        } catch {
            throw TohsenoCompanionError.unsafeStorage
        }
    }

    private static func authenticatedBinding(_ binding: Data) -> Data {
        var result = magic
        result.append(binding)
        return result
    }
}
