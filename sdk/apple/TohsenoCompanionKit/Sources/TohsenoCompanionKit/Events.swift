import CryptoKit
import Foundation

public struct ProductEntitlementProjection: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.private-product-entitlement/1"
    public let schema: String
    public let phase: String
    public let successfulDays: UInt8
    public let requiredSuccessfulDays: UInt8
    public let factoryMutationsAllowed: Bool
    public let purchaseAllowed: Bool

    enum CodingKeys: String, CodingKey {
        case schema, phase
        case successfulDays = "successful_days"
        case requiredSuccessfulDays = "required_successful_days"
        case factoryMutationsAllowed = "factory_mutations_allowed"
        case purchaseAllowed = "purchase_allowed"
    }

    public init(
        schema: String = Self.schemaV1,
        phase: String,
        successfulDays: UInt8,
        requiredSuccessfulDays: UInt8 = 5,
        factoryMutationsAllowed: Bool,
        purchaseAllowed: Bool
    ) {
        self.schema = schema
        self.phase = phase
        self.successfulDays = successfulDays
        self.requiredSuccessfulDays = requiredSuccessfulDays
        self.factoryMutationsAllowed = factoryMutationsAllowed
        self.purchaseAllowed = purchaseAllowed
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "phase", "successful_days", "required_successful_days",
            "factory_mutations_allowed", "purchase_allowed",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        phase = try container.decode(String.self, forKey: .phase)
        successfulDays = try container.decode(UInt8.self, forKey: .successfulDays)
        requiredSuccessfulDays = try container.decode(UInt8.self, forKey: .requiredSuccessfulDays)
        factoryMutationsAllowed = try container.decode(Bool.self, forKey: .factoryMutationsAllowed)
        purchaseAllowed = try container.decode(Bool.self, forKey: .purchaseAllowed)
        try validate()
    }

    public func validate() throws {
        let phases = [
            "genesis_incomplete", "trial_active", "trial_qualified", "trial_expired",
            "pro_monthly", "pro_yearly", "pro_lapsed",
        ]
        guard schema == Self.schemaV1, phases.contains(phase), requiredSuccessfulDays == 5,
              successfulDays <= requiredSuccessfulDays,
              factoryMutationsAllowed == ["trial_active", "pro_monthly", "pro_yearly"].contains(phase),
              purchaseAllowed == ["trial_qualified", "pro_lapsed"].contains(phase)
        else { throw TohsenoCompanionError.invalidEncoding("invalid private product entitlement") }
    }
}

public struct BuilderFollowProjection: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.private-builder-follows/1"
    public let schema: String
    public let builderIDs: [String]
    public let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case schema
        case builderIDs = "builder_ids"
        case updatedAt = "updated_at"
    }

    public init(
        schema: String = Self.schemaV1,
        builderIDs: [String],
        updatedAt: String
    ) {
        self.schema = schema
        self.builderIDs = builderIDs
        self.updatedAt = updatedAt
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "builder_ids", "updated_at"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        builderIDs = try container.decode([String].self, forKey: .builderIDs)
        updatedAt = try container.decode(String.self, forKey: .updatedAt)
        try validate()
    }

    public func validate() throws {
        guard schema == Self.schemaV1, builderIDs.count <= 10_000,
              builderIDs == Array(Set(builderIDs)).sorted(),
              builderIDs.allSatisfy({
                  $0.range(of: #"^eip155:4663:0x[0-9a-f]{40}$"#, options: .regularExpression) != nil
                      && $0 != "eip155:4663:0x0000000000000000000000000000000000000000"
              }) else {
            throw TohsenoCompanionError.invalidEncoding("invalid private Builder follow projection")
        }
        _ = try CompanionTimestamp.parse(updatedAt)
    }
}

public enum PrivateUpdateKind: String, Codable, CaseIterable, Sendable {
    case claimed
    case claimedAppUpdated = "claimed_app_updated"
    case preparationReady = "preparation_ready"
    case forkShipped = "fork_shipped"
    case editionClosed = "edition_closed"
    case aliasApproved = "alias_approved"
    case publicationApproval = "publication_approval"
    case evolutionFinished = "evolution_finished"
}

public struct PrivateUpdateItem: Codable, Equatable, Identifiable, Sendable {
    public static let schemaV1 = "tohseno.private-update/1"
    public let schema: String
    public let updateID: String
    public let kind: PrivateUpdateKind
    public let subjectID: String
    public let evidenceID: String
    public let title: String
    public let detail: String
    public let occurredAt: String
    public let readAt: String?
    public var id: String { updateID }

    enum CodingKeys: String, CodingKey {
        case schema, kind, title, detail
        case updateID = "update_id"
        case subjectID = "subject_id"
        case evidenceID = "evidence_id"
        case occurredAt = "occurred_at"
        case readAt = "read_at"
    }

    public init(
        schema: String = Self.schemaV1,
        updateID: String? = nil,
        kind: PrivateUpdateKind,
        subjectID: String,
        evidenceID: String,
        title: String,
        detail: String,
        occurredAt: String,
        readAt: String? = nil
    ) {
        self.schema = schema
        self.updateID = updateID ?? Self.stableID(
            kind: kind,
            subjectID: subjectID,
            evidenceID: evidenceID
        )
        self.kind = kind
        self.subjectID = subjectID
        self.evidenceID = evidenceID
        self.title = title
        self.detail = detail
        self.occurredAt = occurredAt
        self.readAt = readAt
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        var expected: Set<String> = [
            "schema", "update_id", "kind", "subject_id", "evidence_id",
            "title", "detail", "occurred_at",
        ]
        if container.contains(.readAt) { expected.insert("read_at") }
        try requireExactKeys(decoder, expected)
        schema = try container.decode(String.self, forKey: .schema)
        updateID = try container.decode(String.self, forKey: .updateID)
        kind = try container.decode(PrivateUpdateKind.self, forKey: .kind)
        subjectID = try container.decode(String.self, forKey: .subjectID)
        evidenceID = try container.decode(String.self, forKey: .evidenceID)
        title = try container.decode(String.self, forKey: .title)
        detail = try container.decode(String.self, forKey: .detail)
        occurredAt = try container.decode(String.self, forKey: .occurredAt)
        readAt = try container.decodeIfPresent(String.self, forKey: .readAt)
        try validate()
    }

    public static func stableID(
        kind: PrivateUpdateKind,
        subjectID: String,
        evidenceID: String
    ) -> String {
        var material = Data("TOHSENO-PRIVATE-UPDATE-V1\0".utf8)
        material.append(contentsOf: kind.rawValue.utf8)
        material.append(0)
        material.append(contentsOf: subjectID.utf8)
        material.append(0)
        material.append(contentsOf: evidenceID.utf8)
        return "update_" + Base64URL.encode(Data(SHA256.hash(data: material)))
    }

    public func validate() throws {
        guard schema == Self.schemaV1 else {
            throw TohsenoCompanionError.invalidEncoding("unsupported private Update schema")
        }
        try requireIdentifier(updateID, field: "private_update.update_id")
        try requireBoundedText(subjectID, field: "private_update.subject_id", maximum: 256)
        try requireBoundedText(evidenceID, field: "private_update.evidence_id", maximum: 256)
        try requireBoundedText(title, field: "private_update.title", maximum: 160)
        try requireBoundedText(detail, field: "private_update.detail", maximum: 512)
        _ = try CompanionTimestamp.parse(occurredAt)
        if let readAt { _ = try CompanionTimestamp.parse(readAt) }
        guard updateID == Self.stableID(kind: kind, subjectID: subjectID, evidenceID: evidenceID) else {
            throw TohsenoCompanionError.invalidEncoding("private Update ID differs from evidence")
        }
    }

    func canonicalValue() -> CanonicalValue {
        var value: [String: CanonicalValue] = [
            "detail": .string(detail),
            "evidence_id": .string(evidenceID),
            "kind": .string(kind.rawValue),
            "occurred_at": .string(occurredAt),
            "schema": .string(schema),
            "subject_id": .string(subjectID),
            "title": .string(title),
            "update_id": .string(updateID),
        ]
        if let readAt { value["read_at"] = .string(readAt) }
        return .object(value)
    }
}

public struct PrivateUpdateProjection: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.private-updates/1"
    public let schema: String
    public let items: [PrivateUpdateItem]
    public let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case schema, items
        case updatedAt = "updated_at"
    }

    public init(
        schema: String = Self.schemaV1,
        items: [PrivateUpdateItem],
        updatedAt: String
    ) {
        self.schema = schema
        self.items = items
        self.updatedAt = updatedAt
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "items", "updated_at"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        items = try container.decode([PrivateUpdateItem].self, forKey: .items)
        updatedAt = try container.decode(String.self, forKey: .updatedAt)
        try validate()
    }

    public func validate() throws {
        guard schema == Self.schemaV1, items.count <= 1_000 else {
            throw TohsenoCompanionError.invalidEncoding("invalid private Updates projection")
        }
        for item in items { try item.validate() }
        for (left, right) in zip(items, items.dropFirst()) {
            guard left.occurredAt > right.occurredAt
                    || (left.occurredAt == right.occurredAt && left.updateID < right.updateID)
            else {
                throw TohsenoCompanionError.invalidEncoding("private Updates are not ordered")
            }
        }
        _ = try CompanionTimestamp.parse(updatedAt)
    }
}

public enum WorkspaceEventPayload: Codable, Equatable, Sendable {
    case workspaceSnapshot(WorkspaceSnapshot)
    case productEntitlement(ProductEntitlementProjection)
    case builderFollows(BuilderFollowProjection)
    case capabilityUpdated(CapabilityGrant)
    case privateUpdates(PrivateUpdateProjection)
    case shotUpsert(ShotSummary)
    case shotArchive(shotID: String)
    case shotRemove(shotID: String)
    case iconBlob(CompanionIconBlob)
    case versionAccepted(
        shotID: String,
        expressionID: String,
        versionID: String,
        versionOrdinal: UInt64,
        acceptedAt: String
    )
    case executionQueued(ExecutionSummary)
    case executionStarted(ExecutionSummary)
    case executionUpdated(ExecutionSummary)
    case executionWaitingForDevice(ExecutionSummary)
    case executionCompleted(ExecutionSummary)
    case executionFailed(ExecutionSummary)
    case commandAcknowledged(CommandReceipt)
    case commandRejected(CommandReceipt)
    case deviceRevoked(deviceID: String, revocationEpoch: UInt64)
    case publicationApprovalRequested(PublicationApprovalRequest)

    private enum Keys: String, CodingKey {
        case eventKind = "event_kind"
        case snapshot, entitlement, follows, capability, updates, shot, blob
        case shotID = "shot_id"
        case expressionID = "expression_id"
        case versionID = "version_id"
        case versionOrdinal = "version_ordinal"
        case acceptedAt = "accepted_at"
        case execution, receipt
        case deviceID = "device_id"
        case revocationEpoch = "revocation_epoch"
        case request
    }

    private enum Kind: String, Codable {
        case workspaceSnapshot = "workspace.snapshot"
        case productEntitlement = "product.entitlement"
        case builderFollows = "builder.follows"
        case capabilityUpdated = "capability.updated"
        case privateUpdates = "private.updates"
        case shotUpsert = "shot.upsert"
        case shotArchive = "shot.archive"
        case shotRemove = "shot.remove"
        case iconBlob = "icon.blob"
        case versionAccepted = "version.accepted"
        case executionQueued = "execution.queued"
        case executionStarted = "execution.started"
        case executionUpdated = "execution.updated"
        case executionWaiting = "execution.waiting_for_device"
        case executionCompleted = "execution.completed"
        case executionFailed = "execution.failed"
        case commandAcknowledged = "command.acknowledged"
        case commandRejected = "command.rejected"
        case deviceRevoked = "device.revoked"
        case publicationApprovalRequested = "publication.approval.requested"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(Kind.self, forKey: .eventKind) {
        case .workspaceSnapshot:
            try requireExactKeys(decoder, ["event_kind", "snapshot"])
            self = try .workspaceSnapshot(container.decode(WorkspaceSnapshot.self, forKey: .snapshot))
        case .productEntitlement:
            try requireExactKeys(decoder, ["event_kind", "entitlement"])
            self = try .productEntitlement(container.decode(ProductEntitlementProjection.self, forKey: .entitlement))
        case .builderFollows:
            try requireExactKeys(decoder, ["event_kind", "follows"])
            self = try .builderFollows(container.decode(BuilderFollowProjection.self, forKey: .follows))
        case .capabilityUpdated:
            try requireExactKeys(decoder, ["event_kind", "capability"])
            self = try .capabilityUpdated(container.decode(CapabilityGrant.self, forKey: .capability))
        case .privateUpdates:
            try requireExactKeys(decoder, ["event_kind", "updates"])
            self = try .privateUpdates(container.decode(PrivateUpdateProjection.self, forKey: .updates))
        case .shotUpsert:
            try requireExactKeys(decoder, ["event_kind", "shot"])
            self = try .shotUpsert(container.decode(ShotSummary.self, forKey: .shot))
        case .shotArchive:
            try requireExactKeys(decoder, ["event_kind", "shot_id"])
            self = try .shotArchive(shotID: container.decode(String.self, forKey: .shotID))
        case .shotRemove:
            try requireExactKeys(decoder, ["event_kind", "shot_id"])
            self = try .shotRemove(shotID: container.decode(String.self, forKey: .shotID))
        case .iconBlob:
            try requireExactKeys(decoder, ["event_kind", "blob"])
            self = try .iconBlob(container.decode(CompanionIconBlob.self, forKey: .blob))
        case .versionAccepted:
            try requireExactKeys(decoder, [
                "event_kind", "shot_id", "expression_id", "version_id", "version_ordinal", "accepted_at",
            ])
            self = try .versionAccepted(
                shotID: container.decode(String.self, forKey: .shotID),
                expressionID: container.decode(String.self, forKey: .expressionID),
                versionID: container.decode(String.self, forKey: .versionID),
                versionOrdinal: container.decode(UInt64.self, forKey: .versionOrdinal),
                acceptedAt: container.decode(String.self, forKey: .acceptedAt)
            )
        case .executionQueued:
            try requireExactKeys(decoder, ["event_kind", "execution"])
            self = try .executionQueued(container.decode(ExecutionSummary.self, forKey: .execution))
        case .executionStarted:
            try requireExactKeys(decoder, ["event_kind", "execution"])
            self = try .executionStarted(container.decode(ExecutionSummary.self, forKey: .execution))
        case .executionUpdated:
            try requireExactKeys(decoder, ["event_kind", "execution"])
            self = try .executionUpdated(container.decode(ExecutionSummary.self, forKey: .execution))
        case .executionWaiting:
            try requireExactKeys(decoder, ["event_kind", "execution"])
            self = try .executionWaitingForDevice(container.decode(ExecutionSummary.self, forKey: .execution))
        case .executionCompleted:
            try requireExactKeys(decoder, ["event_kind", "execution"])
            self = try .executionCompleted(container.decode(ExecutionSummary.self, forKey: .execution))
        case .executionFailed:
            try requireExactKeys(decoder, ["event_kind", "execution"])
            self = try .executionFailed(container.decode(ExecutionSummary.self, forKey: .execution))
        case .commandAcknowledged:
            try requireExactKeys(decoder, ["event_kind", "receipt"])
            self = try .commandAcknowledged(container.decode(CommandReceipt.self, forKey: .receipt))
        case .commandRejected:
            try requireExactKeys(decoder, ["event_kind", "receipt"])
            self = try .commandRejected(container.decode(CommandReceipt.self, forKey: .receipt))
        case .deviceRevoked:
            try requireExactKeys(decoder, ["event_kind", "device_id", "revocation_epoch"])
            self = try .deviceRevoked(
                deviceID: container.decode(String.self, forKey: .deviceID),
                revocationEpoch: container.decode(UInt64.self, forKey: .revocationEpoch)
            )
        case .publicationApprovalRequested:
            try requireExactKeys(decoder, ["event_kind", "request"])
            self = try .publicationApprovalRequested(
                container.decode(PublicationApprovalRequest.self, forKey: .request)
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .workspaceSnapshot(snapshot):
            try container.encode(Kind.workspaceSnapshot, forKey: .eventKind)
            try container.encode(snapshot, forKey: .snapshot)
        case let .productEntitlement(entitlement):
            try container.encode(Kind.productEntitlement, forKey: .eventKind)
            try container.encode(entitlement, forKey: .entitlement)
        case let .builderFollows(follows):
            try container.encode(Kind.builderFollows, forKey: .eventKind)
            try container.encode(follows, forKey: .follows)
        case let .capabilityUpdated(capability):
            try container.encode(Kind.capabilityUpdated, forKey: .eventKind)
            try container.encode(capability, forKey: .capability)
        case let .privateUpdates(updates):
            try container.encode(Kind.privateUpdates, forKey: .eventKind)
            try container.encode(updates, forKey: .updates)
        case let .shotUpsert(shot):
            try container.encode(Kind.shotUpsert, forKey: .eventKind)
            try container.encode(shot, forKey: .shot)
        case let .shotArchive(shotID):
            try container.encode(Kind.shotArchive, forKey: .eventKind)
            try container.encode(shotID, forKey: .shotID)
        case let .shotRemove(shotID):
            try container.encode(Kind.shotRemove, forKey: .eventKind)
            try container.encode(shotID, forKey: .shotID)
        case let .iconBlob(blob):
            try container.encode(Kind.iconBlob, forKey: .eventKind)
            try container.encode(blob, forKey: .blob)
        case let .versionAccepted(shotID, expressionID, versionID, ordinal, acceptedAt):
            try container.encode(Kind.versionAccepted, forKey: .eventKind)
            try container.encode(shotID, forKey: .shotID)
            try container.encode(expressionID, forKey: .expressionID)
            try container.encode(versionID, forKey: .versionID)
            try container.encode(ordinal, forKey: .versionOrdinal)
            try container.encode(acceptedAt, forKey: .acceptedAt)
        case let .executionQueued(value):
            try container.encode(Kind.executionQueued, forKey: .eventKind)
            try container.encode(value, forKey: .execution)
        case let .executionStarted(value):
            try container.encode(Kind.executionStarted, forKey: .eventKind)
            try container.encode(value, forKey: .execution)
        case let .executionUpdated(value):
            try container.encode(Kind.executionUpdated, forKey: .eventKind)
            try container.encode(value, forKey: .execution)
        case let .executionWaitingForDevice(value):
            try container.encode(Kind.executionWaiting, forKey: .eventKind)
            try container.encode(value, forKey: .execution)
        case let .executionCompleted(value):
            try container.encode(Kind.executionCompleted, forKey: .eventKind)
            try container.encode(value, forKey: .execution)
        case let .executionFailed(value):
            try container.encode(Kind.executionFailed, forKey: .eventKind)
            try container.encode(value, forKey: .execution)
        case let .commandAcknowledged(value):
            try container.encode(Kind.commandAcknowledged, forKey: .eventKind)
            try container.encode(value, forKey: .receipt)
        case let .commandRejected(value):
            try container.encode(Kind.commandRejected, forKey: .eventKind)
            try container.encode(value, forKey: .receipt)
        case let .deviceRevoked(deviceID, epoch):
            try container.encode(Kind.deviceRevoked, forKey: .eventKind)
            try container.encode(deviceID, forKey: .deviceID)
            try container.encode(epoch, forKey: .revocationEpoch)
        case let .publicationApprovalRequested(request):
            try container.encode(Kind.publicationApprovalRequested, forKey: .eventKind)
            try container.encode(request, forKey: .request)
        }
    }

    func validate() throws {
        switch self {
        case let .workspaceSnapshot(snapshot): try snapshot.validate()
        case let .productEntitlement(entitlement): try entitlement.validate()
        case let .builderFollows(follows): try follows.validate()
        case let .capabilityUpdated(capability):
            let key = try Base64URL.decode(capability.studioSigningPublicKey, expectedBytes: 32)
            try capability.verify(trustedStudioSigningKey: key)
        case let .privateUpdates(updates): try updates.validate()
        case let .shotUpsert(shot): try shot.validate()
        case let .shotArchive(id), let .shotRemove(id): try requireIdentifier(id, field: "shot_id")
        case let .iconBlob(blob): try blob.validate()
        case let .versionAccepted(shotID, expressionID, versionID, ordinal, acceptedAt):
            try requireIdentifier(shotID, field: "shot_id")
            try requireIdentifier(expressionID, field: "expression_id")
            try requireIdentifier(versionID, field: "version_id")
            guard ordinal > 0 else { throw TohsenoCompanionError.invalidEncoding("Version ordinal") }
            _ = try CompanionTimestamp.parse(acceptedAt)
        case let .executionQueued(execution): try validate(execution, states: [.queued])
        case let .executionStarted(execution):
            try validate(execution, states: [.planning, .conception, .materializing])
        case let .executionUpdated(execution): try execution.validate()
        case let .executionWaitingForDevice(execution): try validate(execution, states: [.waitingForDevice])
        case let .executionCompleted(execution): try validate(execution, states: [.accepted])
        case let .executionFailed(execution): try validate(execution, states: [.failed])
        case let .commandAcknowledged(receipt):
            try receipt.validate()
            guard receipt.state != .rejected else { throw TohsenoCompanionError.invalidEncoding("ack is rejected") }
        case let .commandRejected(receipt):
            try receipt.validate()
            guard receipt.state == .rejected else { throw TohsenoCompanionError.invalidEncoding("rejection is not rejected") }
        case let .deviceRevoked(deviceID, epoch):
            try requireIdentifier(deviceID, field: "device_id")
            guard epoch > 0 else { throw TohsenoCompanionError.invalidEncoding("revocation epoch") }
        case let .publicationApprovalRequested(request):
            try request.validate()
        }
    }

    private func validate(_ execution: ExecutionSummary, states: Set<ExecutionStatus>) throws {
        try execution.validate()
        guard states.contains(execution.state) else {
            throw TohsenoCompanionError.invalidEncoding("execution event state differs")
        }
    }
}

public struct WorkspaceEvent: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-event/1"
    public let schema: String
    public let eventID: String
    public let workspaceID: String
    public let cursor: UInt64
    public let emittedAt: String
    public let payload: WorkspaceEventPayload

    enum CodingKeys: String, CodingKey {
        case schema
        case eventID = "event_id"
        case workspaceID = "workspace_id"
        case cursor
        case emittedAt = "emitted_at"
        case payload
    }

    public init(
        schema: String = schemaV1,
        eventID: String,
        workspaceID: String,
        cursor: UInt64,
        emittedAt: String,
        payload: WorkspaceEventPayload
    ) {
        self.schema = schema
        self.eventID = eventID
        self.workspaceID = workspaceID
        self.cursor = cursor
        self.emittedAt = emittedAt
        self.payload = payload
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "event_id", "workspace_id", "cursor", "emitted_at", "payload"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        eventID = try container.decode(String.self, forKey: .eventID)
        workspaceID = try container.decode(String.self, forKey: .workspaceID)
        cursor = try container.decode(UInt64.self, forKey: .cursor)
        emittedAt = try container.decode(String.self, forKey: .emittedAt)
        payload = try container.decode(WorkspaceEventPayload.self, forKey: .payload)
    }

    public func validate() throws {
        guard schema == Self.schemaV1, cursor > 0 else {
            throw TohsenoCompanionError.invalidEncoding("event schema or cursor")
        }
        try requireIdentifier(eventID, field: "event_id")
        try requireIdentifier(workspaceID, field: "workspace_id")
        _ = try CompanionTimestamp.parse(emittedAt)
        try payload.validate()
    }
}
