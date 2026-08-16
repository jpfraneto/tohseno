import Foundation

public enum WorkspaceEventPayload: Codable, Equatable, Sendable {
    case workspaceSnapshot(WorkspaceSnapshot)
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

    private enum Keys: String, CodingKey {
        case eventKind = "event_kind"
        case snapshot, shot, blob
        case shotID = "shot_id"
        case expressionID = "expression_id"
        case versionID = "version_id"
        case versionOrdinal = "version_ordinal"
        case acceptedAt = "accepted_at"
        case execution, receipt
        case deviceID = "device_id"
        case revocationEpoch = "revocation_epoch"
    }

    private enum Kind: String, Codable {
        case workspaceSnapshot = "workspace.snapshot"
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
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(Kind.self, forKey: .eventKind) {
        case .workspaceSnapshot:
            try requireExactKeys(decoder, ["event_kind", "snapshot"])
            self = try .workspaceSnapshot(container.decode(WorkspaceSnapshot.self, forKey: .snapshot))
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
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .workspaceSnapshot(snapshot):
            try container.encode(Kind.workspaceSnapshot, forKey: .eventKind)
            try container.encode(snapshot, forKey: .snapshot)
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
        }
    }

    func validate() throws {
        switch self {
        case let .workspaceSnapshot(snapshot): try snapshot.validate()
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
