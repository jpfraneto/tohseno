import Foundation

public enum ShotKind: String, Codable, Sendable {
    case factoryShot = "factory_shot"
    case recordingOnly = "recording_only"
    case adoptedProject = "adopted_project"
}

public enum ExecutionStatus: String, Codable, CaseIterable, Sendable {
    case queued
    case planning
    case conception
    case materializing
    case building
    case testing
    case verifying
    case repairing
    case waitingForDevice = "waiting_for_device"
    case installing
    case launching
    case accepted
    case failed

    public var isTerminal: Bool { self == .accepted || self == .failed }
}

public struct IconDescriptor: Codable, Equatable, Sendable {
    public let blobID: String
    public let revision: UInt64
    public let mediaType: String
    public let byteLength: UInt64
    public let width: UInt32
    public let height: UInt32
    public let placeholder: Bool

    enum CodingKeys: String, CodingKey {
        case blobID = "blob_id"
        case revision
        case mediaType = "media_type"
        case byteLength = "byte_length"
        case width, height, placeholder
    }

    public init(
        blobID: String,
        revision: UInt64,
        mediaType: String,
        byteLength: UInt64,
        width: UInt32,
        height: UInt32,
        placeholder: Bool
    ) {
        self.blobID = blobID
        self.revision = revision
        self.mediaType = mediaType
        self.byteLength = byteLength
        self.width = width
        self.height = height
        self.placeholder = placeholder
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "blob_id", "revision", "media_type", "byte_length", "width", "height", "placeholder",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        blobID = try container.decode(String.self, forKey: .blobID)
        revision = try container.decode(UInt64.self, forKey: .revision)
        mediaType = try container.decode(String.self, forKey: .mediaType)
        byteLength = try container.decode(UInt64.self, forKey: .byteLength)
        width = try container.decode(UInt32.self, forKey: .width)
        height = try container.decode(UInt32.self, forKey: .height)
        placeholder = try container.decode(Bool.self, forKey: .placeholder)
    }

    public func validate() throws {
        try requireIdentifier(blobID, field: "icon.blob_id")
        guard revision > 0, (1 ... 2 * 1024 * 1024).contains(byteLength),
              (1 ... 2048).contains(width), (1 ... 2048).contains(height),
              mediaType == "image/png" || mediaType == "image/jpeg"
        else { throw TohsenoCompanionError.invalidEncoding("invalid icon descriptor") }
    }
}

public struct ExecutionSummary: Codable, Equatable, Sendable {
    public let executionID: String
    public let shotID: String
    public let state: ExecutionStatus
    public let updatedAt: String
    public let failureCode: String?

    enum CodingKeys: String, CodingKey {
        case executionID = "execution_id"
        case shotID = "shot_id"
        case state
        case updatedAt = "updated_at"
        case failureCode = "failure_code"
    }

    public init(
        executionID: String,
        shotID: String,
        state: ExecutionStatus,
        updatedAt: String,
        failureCode: String? = nil
    ) {
        self.executionID = executionID
        self.shotID = shotID
        self.state = state
        self.updatedAt = updatedAt
        self.failureCode = failureCode
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        var expected: Set<String> = ["execution_id", "shot_id", "state", "updated_at"]
        if container.contains(.failureCode) { expected.insert("failure_code") }
        try requireExactKeys(decoder, expected)
        executionID = try container.decode(String.self, forKey: .executionID)
        shotID = try container.decode(String.self, forKey: .shotID)
        state = try container.decode(ExecutionStatus.self, forKey: .state)
        updatedAt = try container.decode(String.self, forKey: .updatedAt)
        failureCode = try container.decodeIfPresent(String.self, forKey: .failureCode)
    }

    public func validate() throws {
        try requireIdentifier(executionID, field: "execution_id")
        try requireIdentifier(shotID, field: "shot_id")
        _ = try CompanionTimestamp.parse(updatedAt)
        if let failureCode { try requireIdentifier(failureCode, field: "failure_code") }
        guard state == .failed || failureCode == nil else {
            throw TohsenoCompanionError.invalidEncoding("only failed executions have a failure code")
        }
    }
}

public struct EvolutionHistorySummary: Codable, Equatable, Sendable, Identifiable {
    public let evolutionID: String
    public let requestedAt: String
    public let requestSummary: String
    public let status: String
    public let completionSummary: String?
    public let installationSummary: String?

    public var id: String { evolutionID }

    enum CodingKeys: String, CodingKey {
        case evolutionID = "evolution_id"
        case requestedAt = "requested_at"
        case requestSummary = "request_summary"
        case status
        case completionSummary = "completion_summary"
        case installationSummary = "installation_summary"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        var expected: Set<String> = [
            "evolution_id", "requested_at", "request_summary", "status",
        ]
        if container.contains(.completionSummary) { expected.insert("completion_summary") }
        if container.contains(.installationSummary) { expected.insert("installation_summary") }
        try requireExactKeys(decoder, expected)
        evolutionID = try container.decode(String.self, forKey: .evolutionID)
        requestedAt = try container.decode(String.self, forKey: .requestedAt)
        requestSummary = try container.decode(String.self, forKey: .requestSummary)
        status = try container.decode(String.self, forKey: .status)
        completionSummary = try container.decodeIfPresent(String.self, forKey: .completionSummary)
        installationSummary = try container.decodeIfPresent(String.self, forKey: .installationSummary)
    }

    public func validate() throws {
        try requireIdentifier(evolutionID, field: "evolution_id")
        _ = try CompanionTimestamp.parse(requestedAt)
        try requireBoundedText(requestSummary, field: "request_summary", maximum: 2_048)
        try requireIdentifier(status, field: "status")
        if let completionSummary {
            try requireBoundedText(
                completionSummary, field: "completion_summary", maximum: 2_048
            )
        }
        if let installationSummary {
            try requireBoundedText(
                installationSummary, field: "installation_summary", maximum: 2_048
            )
        }
    }
}

public struct ShotSummary: Codable, Equatable, Sendable {
    public let shotID: String
    public let displayName: String
    public let bundleIdentifier: String?
    public let kind: ShotKind
    public let sourceState: String?
    public let icon: IconDescriptor?
    public let iconRevision: UInt64
    public let expressionID: String?
    public let latestVersionID: String?
    public let latestVersionOrdinal: UInt64?
    public let latestVersionCreatedAt: String?
    public let execution: ExecutionSummary?
    public let recentEvolutions: [EvolutionHistorySummary]?
    public let archived: Bool
    public let retired: Bool
    public let sortIndex: Int64
    public let supportedCompanionActions: [CompanionCapability]

    enum CodingKeys: String, CodingKey {
        case shotID = "shot_id"
        case displayName = "display_name"
        case bundleIdentifier = "bundle_identifier"
        case kind, icon
        case sourceState = "source_state"
        case iconRevision = "icon_revision"
        case expressionID = "expression_id"
        case latestVersionID = "latest_version_id"
        case latestVersionOrdinal = "latest_version_ordinal"
        case latestVersionCreatedAt = "latest_version_created_at"
        case execution, archived, retired
        case recentEvolutions = "recent_evolutions"
        case sortIndex = "sort_index"
        case supportedCompanionActions = "supported_companion_actions"
    }

    public init(
        shotID: String,
        displayName: String,
        bundleIdentifier: String? = nil,
        kind: ShotKind,
        sourceState: String? = nil,
        icon: IconDescriptor? = nil,
        iconRevision: UInt64,
        expressionID: String? = nil,
        latestVersionID: String? = nil,
        latestVersionOrdinal: UInt64? = nil,
        latestVersionCreatedAt: String? = nil,
        execution: ExecutionSummary? = nil,
        recentEvolutions: [EvolutionHistorySummary]? = nil,
        archived: Bool = false,
        retired: Bool = false,
        sortIndex: Int64,
        supportedCompanionActions: [CompanionCapability]
    ) {
        self.shotID = shotID
        self.displayName = displayName
        self.bundleIdentifier = bundleIdentifier
        self.kind = kind
        self.sourceState = sourceState
        self.icon = icon
        self.iconRevision = iconRevision
        self.expressionID = expressionID
        self.latestVersionID = latestVersionID
        self.latestVersionOrdinal = latestVersionOrdinal
        self.latestVersionCreatedAt = latestVersionCreatedAt
        self.execution = execution
        self.recentEvolutions = recentEvolutions
        self.archived = archived
        self.retired = retired
        self.sortIndex = sortIndex
        self.supportedCompanionActions = supportedCompanionActions
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        var expected: Set<String> = [
            "shot_id", "display_name", "kind", "icon_revision", "archived", "retired",
            "sort_index", "supported_companion_actions",
        ]
        let optionals: [(CodingKeys, String)] = [
            (.bundleIdentifier, "bundle_identifier"), (.icon, "icon"),
            (.sourceState, "source_state"),
            (.expressionID, "expression_id"), (.latestVersionID, "latest_version_id"),
            (.latestVersionOrdinal, "latest_version_ordinal"),
            (.latestVersionCreatedAt, "latest_version_created_at"), (.execution, "execution"),
            (.recentEvolutions, "recent_evolutions"),
        ]
        for (key, name) in optionals where container.contains(key) { expected.insert(name) }
        try requireExactKeys(decoder, expected)
        shotID = try container.decode(String.self, forKey: .shotID)
        displayName = try container.decode(String.self, forKey: .displayName)
        bundleIdentifier = try container.decodeIfPresent(String.self, forKey: .bundleIdentifier)
        kind = try container.decode(ShotKind.self, forKey: .kind)
        sourceState = try container.decodeIfPresent(String.self, forKey: .sourceState)
        icon = try container.decodeIfPresent(IconDescriptor.self, forKey: .icon)
        iconRevision = try container.decode(UInt64.self, forKey: .iconRevision)
        expressionID = try container.decodeIfPresent(String.self, forKey: .expressionID)
        latestVersionID = try container.decodeIfPresent(String.self, forKey: .latestVersionID)
        latestVersionOrdinal = try container.decodeIfPresent(UInt64.self, forKey: .latestVersionOrdinal)
        latestVersionCreatedAt = try container.decodeIfPresent(String.self, forKey: .latestVersionCreatedAt)
        execution = try container.decodeIfPresent(ExecutionSummary.self, forKey: .execution)
        recentEvolutions = try container.decodeIfPresent(
            [EvolutionHistorySummary].self, forKey: .recentEvolutions
        )
        archived = try container.decode(Bool.self, forKey: .archived)
        retired = try container.decode(Bool.self, forKey: .retired)
        sortIndex = try container.decode(Int64.self, forKey: .sortIndex)
        supportedCompanionActions = try container.decode(
            [CompanionCapability].self,
            forKey: .supportedCompanionActions
        )
    }

    public func validate() throws {
        try requireIdentifier(shotID, field: "shot_id")
        try requireBoundedText(displayName, field: "display_name", maximum: 256)
        if let bundleIdentifier {
            try requireBoundedText(bundleIdentifier, field: "bundle_identifier", maximum: 255)
            guard bundleIdentifier.utf8.allSatisfy({ byte in
                (0x30 ... 0x39).contains(byte) || (0x41 ... 0x5a).contains(byte)
                    || (0x61 ... 0x7a).contains(byte) || byte == 0x2d || byte == 0x2e
            }) else { throw TohsenoCompanionError.invalidEncoding("invalid bundle identifier") }
        }
        if let icon {
            try icon.validate()
            guard icon.revision == iconRevision else {
                throw TohsenoCompanionError.invalidEncoding("icon revision differs")
            }
        }
        let versionPresence = [
            latestVersionID != nil, latestVersionOrdinal != nil, latestVersionCreatedAt != nil,
        ]
        guard versionPresence.allSatisfy({ $0 }) || versionPresence.allSatisfy({ !$0 }) else {
            throw TohsenoCompanionError.invalidEncoding("Version fields are incomplete")
        }
        if let expressionID { try requireIdentifier(expressionID, field: "expression_id") }
        if let sourceState { try requireIdentifier(sourceState, field: "source_state") }
        if let latestVersionID { try requireIdentifier(latestVersionID, field: "latest_version_id") }
        if let latestVersionOrdinal, latestVersionOrdinal == 0 {
            throw TohsenoCompanionError.invalidEncoding("Version ordinal must be positive")
        }
        if let latestVersionCreatedAt { _ = try CompanionTimestamp.parse(latestVersionCreatedAt) }
        if let execution {
            try execution.validate()
            guard execution.shotID == shotID else {
                throw TohsenoCompanionError.invalidEncoding("execution belongs to another Shot")
            }
        }
        guard (recentEvolutions?.count ?? 0) <= 20 else {
            throw TohsenoCompanionError.invalidEncoding("too many recent evolutions")
        }
        for evolution in recentEvolutions ?? [] { try evolution.validate() }
        guard kind == .adoptedProject || recentEvolutions?.isEmpty != false else {
            throw TohsenoCompanionError.invalidEncoding(
                "only adopted projects have private evolution history"
            )
        }
        guard !(kind == .recordingOnly && (expressionID != nil || latestVersionID != nil)),
              kind == .adoptedProject
                ? (sourceState != nil && expressionID == nil && latestVersionID == nil)
                : sourceState == nil,
              !(archived && retired),
              supportedCompanionActions == Array(Set(supportedCompanionActions)).sorted()
        else { throw TohsenoCompanionError.invalidEncoding("invalid Shot summary") }
    }
}

public struct DeviceCapabilityState: Codable, Equatable, Sendable {
    public let deviceID: String
    public let capabilityID: String
    public let revocationEpoch: UInt64
    public let allowedActions: [CompanionCapability]
    public let revoked: Bool

    enum CodingKeys: String, CodingKey {
        case deviceID = "device_id"
        case capabilityID = "capability_id"
        case revocationEpoch = "revocation_epoch"
        case allowedActions = "allowed_actions"
        case revoked
    }

    public init(
        deviceID: String,
        capabilityID: String,
        revocationEpoch: UInt64,
        allowedActions: [CompanionCapability],
        revoked: Bool
    ) {
        self.deviceID = deviceID
        self.capabilityID = capabilityID
        self.revocationEpoch = revocationEpoch
        self.allowedActions = allowedActions
        self.revoked = revoked
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "device_id", "capability_id", "revocation_epoch", "allowed_actions", "revoked",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        deviceID = try container.decode(String.self, forKey: .deviceID)
        capabilityID = try container.decode(String.self, forKey: .capabilityID)
        revocationEpoch = try container.decode(UInt64.self, forKey: .revocationEpoch)
        allowedActions = try container.decode([CompanionCapability].self, forKey: .allowedActions)
        revoked = try container.decode(Bool.self, forKey: .revoked)
    }

    public func validate() throws {
        try requireIdentifier(deviceID, field: "device_id")
        try requireIdentifier(capabilityID, field: "capability_id")
        guard allowedActions == Array(Set(allowedActions)).sorted() else {
            throw TohsenoCompanionError.invalidEncoding("capability actions are not sorted and unique")
        }
    }
}

public struct WorkspaceSnapshot: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-workspace-snapshot/1"

    public let schema: String
    public let workspaceID: String
    public let snapshotVersion: UInt64
    public let generatedAt: String
    public let serviceVersion: String
    public let shots: [ShotSummary]
    public let activeExecutions: [ExecutionSummary]
    public let deviceCapabilityState: DeviceCapabilityState
    public let nextCursor: UInt64

    enum CodingKeys: String, CodingKey {
        case schema
        case workspaceID = "workspace_id"
        case snapshotVersion = "snapshot_version"
        case generatedAt = "generated_at"
        case serviceVersion = "service_version"
        case shots
        case activeExecutions = "active_executions"
        case deviceCapabilityState = "device_capability_state"
        case nextCursor = "next_cursor"
    }

    public init(
        schema: String = schemaV1,
        workspaceID: String,
        snapshotVersion: UInt64,
        generatedAt: String,
        serviceVersion: String,
        shots: [ShotSummary],
        activeExecutions: [ExecutionSummary],
        deviceCapabilityState: DeviceCapabilityState,
        nextCursor: UInt64
    ) {
        self.schema = schema
        self.workspaceID = workspaceID
        self.snapshotVersion = snapshotVersion
        self.generatedAt = generatedAt
        self.serviceVersion = serviceVersion
        self.shots = shots
        self.activeExecutions = activeExecutions
        self.deviceCapabilityState = deviceCapabilityState
        self.nextCursor = nextCursor
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "workspace_id", "snapshot_version", "generated_at", "service_version",
            "shots", "active_executions", "device_capability_state", "next_cursor",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        workspaceID = try container.decode(String.self, forKey: .workspaceID)
        snapshotVersion = try container.decode(UInt64.self, forKey: .snapshotVersion)
        generatedAt = try container.decode(String.self, forKey: .generatedAt)
        serviceVersion = try container.decode(String.self, forKey: .serviceVersion)
        shots = try container.decode([ShotSummary].self, forKey: .shots)
        activeExecutions = try container.decode([ExecutionSummary].self, forKey: .activeExecutions)
        deviceCapabilityState = try container.decode(DeviceCapabilityState.self, forKey: .deviceCapabilityState)
        nextCursor = try container.decode(UInt64.self, forKey: .nextCursor)
    }

    public func validate() throws {
        guard schema == Self.schemaV1, snapshotVersion > 0, shots.count <= 10_000,
              activeExecutions.count <= 1_000
        else { throw TohsenoCompanionError.invalidEncoding("invalid workspace snapshot") }
        try requireIdentifier(workspaceID, field: "workspace_id")
        _ = try CompanionTimestamp.parse(generatedAt)
        try requireBoundedText(serviceVersion, field: "service_version", maximum: 64)
        for shot in shots { try shot.validate() }
        for execution in activeExecutions {
            try execution.validate()
            guard !execution.state.isTerminal else {
                throw TohsenoCompanionError.invalidEncoding("terminal execution is marked active")
            }
        }
        try deviceCapabilityState.validate()
    }
}
