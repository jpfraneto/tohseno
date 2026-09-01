import CryptoKit
import Foundation

public enum PresentedState: String, Codable, CaseIterable, Sendable {
    case waiting
    case building
    case readyForPhone = "ready_for_phone"
    case installing
    case installed
    case failed

    public var isInFlight: Bool {
        self == .waiting || self == .building || self == .installing
    }
}

public struct Presentation: Codable, Equatable, Sendable {
    public let state: PresentedState
    public let headline: String
    public let detail: String?

    public init(state: PresentedState, headline: String, detail: String? = nil) {
        self.state = state
        self.headline = headline
        self.detail = detail
    }
}

public struct IconDescriptor: Codable, Equatable, Sendable {
    public let revision: String
    public let blobID: String
    public let mediaType: String
    public let byteLength: UInt64
    public let placeholder: Bool
}

public struct ExecutionSummary: Codable, Equatable, Sendable {
    public let executionID: String
    public let shotID: String
    public let state: String
    public let versionOrdinal: UInt64
    public let startedAt: String
    public let elapsedSeconds: UInt64
    public let updatedAt: String
}

public struct ProjectEvolutionSummary: Codable, Equatable, Identifiable, Sendable {
    public let evolutionID: String
    public let requestedAt: String
    public let requestSummary: String
    public let status: String
    public let completionSummary: String?
    public let installationSummary: String?

    public var id: String { evolutionID }
}

public struct AppSummary: Codable, Equatable, Identifiable, Sendable {
    public let shotID: String
    public let displayName: String
    public let bundleIdentifier: String?
    public var sourceState: String? = nil
    public let icon: IconDescriptor
    public let expressionID: String?
    public let latestVersionID: String?
    public let latestVersionOrdinal: UInt64?
    public let latestVersionCreatedAt: String?
    public let execution: ExecutionSummary?
    public var recentEvolutions: [ProjectEvolutionSummary]? = nil
    public let presentation: Presentation
    public let archived: Bool
    public let retired: Bool
    public let sortIndex: UInt64

    public var id: String { shotID }
}

public struct WorkspaceSnapshot: Codable, Equatable, Sendable {
    public let schema: String
    public let workspaceID: String
    public let snapshotVersion: UInt64
    public let generatedAt: String
    public let serviceVersion: String
    public let shots: [AppSummary]
    public let activeExecutions: [ExecutionSummary]

    public var visibleApps: [AppSummary] {
        shots.filter { !$0.retired && !$0.archived }
            .sorted { ($0.sortIndex, $0.displayName) < ($1.sortIndex, $1.displayName) }
    }

    public var archivedApps: [AppSummary] {
        shots.filter { $0.retired || $0.archived }
            .sorted { ($0.sortIndex, $0.displayName) < ($1.sortIndex, $1.displayName) }
    }
}

public enum HarnessAuthentication: String, Codable, Sendable {
    case authenticated
    case notDetected = "not_detected"
    case unknown
}

public struct FactoryModelOption: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let label: String
    public let isDefault: Bool
}

public struct FactoryHarnessOption: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let label: String
    public let installed: Bool
    public let selected: Bool
    public let authentication: HarnessAuthentication
    public let models: [FactoryModelOption]
    public let routes: [FactoryRouteOption]
}

public struct FactoryRouteOption: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let label: String
    public let billing: String
    public let available: Bool
    public let estimatedAdditionalCostUSD: Double?
    public let costEstimation: Bool
}

public struct FactoryDefaults: Codable, Equatable, Sendable {
    public let schema: String
    public let ready: Bool
    public let harnessID: String?
    public let harnessLabel: String?
    public let modelID: String?
    public let modelLabel: String?
    public let routeID: String?
    public let routeLabel: String?
    public let harnesses: [FactoryHarnessOption]
}

public struct AdoptedProject: Codable, Equatable, Identifiable, Sendable {
    public let projectID: String
    public let displayName: String
    public let sourcePath: String
    public let containerPath: String
    public let scheme: String
    public let bundleIdentifier: String
    public let currentSourceState: String
    public let build: AdoptedProjectBuild
    public let recovery: String?

    public var id: String { projectID }
}

public struct AdoptedProjectBuild: Codable, Equatable, Sendable {
    public let status: String
    public let failureCategory: String?
    public let summary: String?
}

public struct ProjectAdoptionResult: Codable, Equatable, Sendable {
    public let schema: String
    public let status: String
    public let schemeCandidates: [String]
    public let project: AdoptedProject?
    public let message: String?
}

public struct PairedCompanionDevice: Codable, Equatable, Identifiable, Sendable {
    public let deviceID: String
    public let deviceIDAbbreviation: String
    public let displayName: String
    public let pairedAt: String
    public let lastSeen: String
    public let syncState: String
    public let revoked: Bool

    public var id: String { deviceID }
}

public struct CompanionPairingSession: Codable, Equatable, Identifiable, Sendable {
    public let schema: String
    public let sessionID: String
    public let state: String
    public let expiresAt: String
    public let pairingURI: String
    public let deviceName: String?

    public var id: String { sessionID }
}

public struct ReferenceDraft: Equatable, Identifiable, Sendable {
    public let id: UUID
    public let filename: String
    public let mediaType: String
    public let data: Data
    public let origin: String

    public init(
        id: UUID = UUID(),
        filename: String,
        mediaType: String,
        data: Data,
        origin: String
    ) {
        self.id = id
        self.filename = filename
        self.mediaType = mediaType
        self.data = data
        self.origin = origin
    }
}

public struct CreationDraft: Equatable, Sendable {
    public var name: String
    public var intention: String
    public var references: [ReferenceDraft]
    public var harness: String?
    public var model: String?
    public var managedPrivacy: String
    public var managedMaximumMicrousd: UInt64?
    public var managedConsent: Bool

    public init(
        name: String = "",
        intention: String = "",
        references: [ReferenceDraft] = [],
        harness: String? = nil,
        model: String? = nil,
        managedPrivacy: String = "standard",
        managedMaximumMicrousd: UInt64? = nil,
        managedConsent: Bool = false
    ) {
        self.name = name
        self.intention = intention
        self.references = references
        self.harness = harness
        self.model = model
        self.managedPrivacy = managedPrivacy
        self.managedMaximumMicrousd = managedMaximumMicrousd
        self.managedConsent = managedConsent
    }
}

public struct EvolutionDraft: Equatable, Sendable {
    public var intention: String
    public var references: [ReferenceDraft]
    public var harness: String?
    public var model: String?
    public var managedPrivacy: String
    public var managedMaximumMicrousd: UInt64?
    public var managedConsent: Bool

    public init(
        intention: String = "",
        references: [ReferenceDraft] = [],
        harness: String? = nil,
        model: String? = nil,
        managedPrivacy: String = "standard",
        managedMaximumMicrousd: UInt64? = nil,
        managedConsent: Bool = false
    ) {
        self.intention = intention
        self.references = references
        self.harness = harness
        self.model = model
        self.managedPrivacy = managedPrivacy
        self.managedMaximumMicrousd = managedMaximumMicrousd
        self.managedConsent = managedConsent
    }
}

public struct ManagedStatus: Codable, Equatable, Sendable {
    public let schema: String
    public let installationBinding: String
    public let serviceOrigin: String
    public let welcomeContactURL: String?
    public let automaticFallback: Bool
}

public struct ManagedLedgerEntry: Codable, Equatable, Identifiable, Sendable {
    public let entryID: String
    public let amountMicrousd: Int64
    public let entryType: String
    public let bucket: String
    public let createdAt: String
    public let description: String
    public let relatedExecutionID: String?
    public let relatedProviderID: String?
    public let relatedModel: String?
    public let privacyTier: String?
    public let reconciliationStatus: String?
    public var id: String { entryID }
}

public struct ManagedBalance: Codable, Equatable, Sendable {
    public let schema: String
    public let installationBinding: String
    public let paidMicrousd: Int64
    public let promotionalMicrousd: Int64
    public let reservedMicrousd: Int64
    public let spendableMicrousd: Int64
    public let currency: String
    public let transactions: [ManagedLedgerEntry]
}

public struct ManagedModel: Codable, Equatable, Identifiable, Sendable {
    public let model: String
    public let inputMicrousdPerMillion: UInt64
    public let outputMicrousdPerMillion: UInt64
    public let privacyTiers: [String]
    public let snapshotAt: String
    public var id: String { model }
}

public struct ManagedCatalog: Codable, Equatable, Sendable {
    public let schema: String
    public let models: [ManagedModel]
}

public struct ManagedEstimate: Codable, Equatable, Sendable {
    public let schema: String
    public let estimatorVersion: String
    public let model: String
    public let privacy: String
    public let pricingSnapshotAt: String
    public let lowMicrousd: UInt64
    public let highMicrousd: UInt64
    public let recommendedMaximumMicrousd: UInt64
    public let expectedInputTokensLow: UInt64
    public let expectedInputTokensHigh: UInt64
    public let expectedOutputTokensLow: UInt64
    public let expectedOutputTokensHigh: UInt64
    public let invocationLimit: UInt8
    public let provenance: [String]
}

public struct ManagedCheckout: Codable, Equatable, Sendable {
    public let schema: String
    public let checkoutURL: String
}

public struct BuilderIdentityView: Codable, Equatable, Sendable {
    public let builderID: String
    public let chainID: UInt64
    public let accountAddress: String
    public let identityGeneration: String
    public let scope: String
    public let authorityStatus: String
    public let deploymentStatus: String
    public let deviceKeyID: String
    public let securityLevel: String
    public let testOnly: Bool
}

public struct RegistryNetworkStatus: Codable, Equatable, Sendable {
    public let schema: String
    public let productVersion: String
    public let activeGeneration: String
    public let ready: Bool
    public let rpcChecked: Bool
    public let publicAuthorityAvailable: Bool
    public let publishingAvailable: Bool
    public let reason: String
}

public struct LocalRegistryRecord: Codable, Equatable, Identifiable, Sendable {
    public let schema: String
    public let appName: String
    public let shotID: String
    public let localHead: String
    public let localSequence: UInt64
    public let localState: String
    public let localVerified: Bool
    public let activeGeneration: String
    public let publicChecked: Bool
    public let publicAuthorityAvailable: Bool
    public let reason: String

    public var id: String { shotID }
}

public struct PublicRegistryRelease: Codable, Equatable, Identifiable, Sendable {
    public struct Release: Codable, Equatable, Sendable {
        public struct Display: Codable, Equatable, Sendable {
            public let name: String
            public let description: String
            public let builderHandle: String?

            enum CodingKeys: String, CodingKey {
                case name, description
                case builderHandle = "builder_handle"
            }
        }

        public struct Permissions: Codable, Equatable, Sendable {
            public let installAllowed: Bool
            public let forkAllowed: Bool

            enum CodingKeys: String, CodingKey {
                case installAllowed = "install_allowed"
                case forkAllowed = "fork_allowed"
            }
        }

        public let shotID: String
        public let builderID: String
        public let checkpointSequence: UInt64
        public let display: Display
        public let permissions: Permissions

        enum CodingKeys: String, CodingKey {
            case shotID = "shot_id"
            case builderID = "builder_id"
            case checkpointSequence = "checkpoint_sequence"
            case display, permissions
        }
    }

    public let releaseDigest: String
    public let route: String
    public let release: Release
    public let sourceURL: String

    public var id: String { releaseDigest }

    enum CodingKeys: String, CodingKey {
        case releaseDigest = "release_digest"
        case route, release
        case sourceURL = "source_url"
    }
}

public struct PublicTimelineEvent: Codable, Equatable, Identifiable, Sendable {
    public struct Parent: Codable, Equatable, Sendable {
        public let shotID: String
        public let releaseDigest: String
        enum CodingKeys: String, CodingKey {
            case shotID = "shot_id"
            case releaseDigest = "release_digest"
        }
    }

    public let schema: String
    public let eventID: String
    public let kind: String
    public let shotID: String
    public let builderID: String
    public let releaseDigest: String
    public let checkpointSequence: UInt64
    public let occurredAt: String
    public let parent: Parent?
    public let closureReason: String?
    public var id: String { eventID }

    enum CodingKeys: String, CodingKey {
        case schema, kind
        case eventID = "event_id"
        case shotID = "shot_id"
        case builderID = "builder_id"
        case releaseDigest = "release_digest"
        case checkpointSequence = "checkpoint_sequence"
        case occurredAt = "occurred_at"
        case parent
        case closureReason = "closure_reason"
    }
}

public struct NetworkFollowProjection: Codable, Equatable, Sendable {
    public let schema: String
    public let builderIDs: [String]
    public let updatedAt: String
}

public enum PrivateUpdateKind: String, Codable, Sendable {
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

    public init(
        schema: String = "tohseno.private-update/1",
        kind: PrivateUpdateKind,
        subjectID: String,
        evidenceID: String,
        title: String,
        detail: String,
        occurredAt: String,
        readAt: String? = nil
    ) {
        self.schema = schema
        self.updateID = Self.stableID(kind: kind, subjectID: subjectID, evidenceID: evidenceID)
        self.kind = kind
        self.subjectID = subjectID
        self.evidenceID = evidenceID
        self.title = title
        self.detail = detail
        self.occurredAt = occurredAt
        self.readAt = readAt
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
        let digest = Data(SHA256.hash(data: material)).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        return "update_\(digest)"
    }
}

public struct PrivateUpdateProjection: Codable, Equatable, Sendable {
    public let schema: String
    public let items: [PrivateUpdateItem]
    public let updatedAt: String
}

struct PublicTimelinePage: Codable, Sendable {
    let schema: String
    let events: [PublicTimelineEvent]
}

public struct NetworkReviewRequest: Equatable, Sendable {
    public let shotID: String
    public let releaseDigest: String
    public let action: NetworkReceiveAction
    public let reasons: String
}

struct PublicCatalogPage: Codable, Sendable {
    let schema: String
    let releases: [PublicRegistryRelease]
}

struct PublicRegistryServiceStatus: Codable, Sendable {
    struct Relayer: Codable, Sendable {
        let available: Bool
    }

    let schema: String
    let available: Bool
    let generation: String
    let relayer: Relayer
}

public struct RegistrySnapshot: Equatable, Sendable {
    public let builder: BuilderIdentityView
    public let network: RegistryNetworkStatus
    public let records: [LocalRegistryRecord]
    public let published: [PublicRegistryRelease]
    public let timeline: [PublicTimelineEvent]
    public let followedBuilderIDs: Set<String>
    public var privateUpdates: [PrivateUpdateItem]

    public init(
        builder: BuilderIdentityView,
        network: RegistryNetworkStatus,
        records: [LocalRegistryRecord],
        published: [PublicRegistryRelease] = [],
        timeline: [PublicTimelineEvent] = [],
        followedBuilderIDs: Set<String> = [],
        privateUpdates: [PrivateUpdateItem] = []
    ) {
        self.builder = builder
        self.network = network
        self.records = records.sorted {
            ($0.appName, $0.shotID) < ($1.appName, $1.shotID)
        }
        self.published = published
        self.timeline = timeline
        self.followedBuilderIDs = followedBuilderIDs
        self.privateUpdates = privateUpdates
    }

    public var acceptedVersionCount: UInt64 {
        records.reduce(0) { $0 + $1.localSequence }
    }
}

public struct CommandReceipt: Codable, Equatable, Sendable {
    public let schema: String
    public let commandID: String
    public let shotID: String
    public let executionID: String
    public let state: String
}

public struct PublicationPreparationView: Codable, Equatable, Sendable {
    public let schema: String
    public let jobID: String
    public let projectID: String
    public let shotID: String
    public let status: String

    enum CodingKeys: String, CodingKey {
        case schema, status
        case jobID = "job_id"
        case projectID = "project_id"
        case shotID = "shot_id"
    }
}

public enum NetworkReceiveAction: String, Codable, Sendable {
    case install
    case fork
}

public struct NetworkReceiveView: Codable, Equatable, Sendable {
    public let schema: String
    public let action: String
    public let shotID: String
    public let releaseDigest: String
    public let builderID: String
    public let sourcePath: String
    public let projectID: String
    public let candidateShotID: String?
    public let installationStatus: String

    enum CodingKeys: String, CodingKey {
        case schema, action
        case shotID = "shot_id"
        case releaseDigest = "release_digest"
        case builderID = "builder_id"
        case sourcePath = "source_path"
        case projectID = "project_id"
        case candidateShotID = "candidate_shot_id"
        case installationStatus = "installation_status"
    }
}

public struct NativeSessionCredential: Codable, Equatable, Sendable {
    public let schema: String
    public let token: String
    public let tokenType: String
    public let clientID: String
    public let instanceID: String
    public let origin: String
    public let scopes: [String]
    public let expiresAt: String
}

public struct CLIIntegrationStatus: Codable, Equatable, Sendable {
    public let schema: String
    public let installed: Bool
    public let enabled: Bool
    public let commandPath: String
    public let profilePath: String
    public let shell: String
    public let requiresNewTerminal: Bool
}

public struct ReadinessView: Codable, Equatable, Sendable {
    public let schema: String
    public let ready: Bool
    public let step: String
    public let headline: String
    public let detail: String
    public let primaryAction: String?
    public let primaryLabel: String?
    public let automaticallyObserved: Bool
    public let progress: Double?
    public let deviceName: String?
    public let deviceProductType: String?
    public let companionConnected: Bool
    public let companionInstallState: String?

    public init(
        schema: String,
        ready: Bool,
        step: String,
        headline: String,
        detail: String,
        primaryAction: String?,
        primaryLabel: String?,
        automaticallyObserved: Bool = false,
        progress: Double? = nil,
        deviceName: String? = nil,
        deviceProductType: String? = nil,
        companionConnected: Bool = false,
        companionInstallState: String? = nil
    ) {
        self.schema = schema
        self.ready = ready
        self.step = step
        self.headline = headline
        self.detail = detail
        self.primaryAction = primaryAction
        self.primaryLabel = primaryLabel
        self.automaticallyObserved = automaticallyObserved
        self.progress = progress
        self.deviceName = deviceName
        self.deviceProductType = deviceProductType
        self.companionConnected = companionConnected
        self.companionInstallState = companionInstallState
    }

    public var isWorking: Bool {
        [
            "building_companion", "installing_companion", "launching_companion",
            "pairing_companion",
        ].contains(step)
    }

    public var shouldMonitor: Bool {
        isWorking || automaticallyObserved
    }

    public var setupProgress: Double {
        if let progress { return progress }
        return switch step {
        case "welcome": 0.08
        case "connect_cable": 0.18
        case "trust_mac": 0.28
        case "install_xcode": 0.38
        case "developer_mode": 0.48
        case "add_apple_account": 0.58
        case "install_companion": 0.66
        case "building_companion": 0.70
        case "installing_companion": 0.80
        case "launching_companion": 0.90
        case "pairing_companion": 0.96
        default: ready ? 1 : 0.08
        }
    }

    public var setupStatus: String {
        switch step {
        case "welcome": "Ready to start"
        case "connect_cable": "Waiting for the cable"
        case "trust_mac": "Waiting for Trust"
        case "install_xcode": "Waiting for Xcode"
        case "developer_mode": "Waiting for Developer Mode"
        case "add_apple_account": "Checking Apple Account"
        case "install_companion" where companionInstallState == "failed": "Build stopped"
        case "install_companion": "Ready to build"
        case "building_companion": "Building and signing"
        case "installing_companion": "Installing on iPhone"
        case "launching_companion": "Opening on iPhone"
        case "pairing_companion": "Connecting privately"
        default: ready ? "Setup complete" : "Checking setup"
        }
    }

    public var setupStepNumber: Int {
        switch step {
        case "welcome": 1
        case "connect_cable": 2
        case "trust_mac": 3
        case "install_xcode": 4
        case "developer_mode": 5
        case "add_apple_account": 6
        case "install_companion", "building_companion": 7
        case "installing_companion", "launching_companion", "pairing_companion": 8
        default: ready ? 8 : 1
        }
    }

    public var setupCheckpoints: [ReadinessCheckpoint] {
        let labels = [
            "Start setup",
            "Connect your iPhone",
            "Trust this Mac",
            "Prepare Xcode",
            "Turn on Developer Mode",
            "Verify your Apple Account",
            "Build and sign Companion",
            "Install and connect Companion",
        ]
        return labels.enumerated().map { index, label in
            let number = index + 1
            let state: ReadinessCheckpoint.State = if ready || number < setupStepNumber {
                .complete
            } else if number > setupStepNumber {
                .waiting
            } else if companionInstallState == "failed" {
                .failed
            } else if isWorking {
                .working
            } else {
                .current
            }
            return ReadinessCheckpoint(number: number, label: label, state: state)
        }
    }
}

public struct ReadinessCheckpoint: Equatable, Identifiable, Sendable {
    public enum State: Equatable, Sendable {
        case complete
        case current
        case working
        case failed
        case waiting
    }

    public let number: Int
    public let label: String
    public let state: State
    public var id: Int { number }
}

public struct CustomHarnessDraft: Equatable, Sendable {
    public var id = ""
    public var label = ""
    public var executable = ""
    public var arguments = ""
    public var models = "default"
    public var preferred = false

    public init() {}
}

public struct LocalEndpointDraft: Equatable, Sendable {
    public var id = ""
    public var label = ""
    public var baseURL = "http://127.0.0.1:11434"
    public var models = ""
    public var credential = ""
    public var consentToSendSource = false
    public var privacyMode = "local"
    public var preferred = false

    public init() {}
}

public struct ExecutionReceipt: Codable, Equatable, Sendable {
    public let schema: String
    public let executionID: String
    public let appName: String
    public let versionOrdinal: UInt64
    public let phase: String
    public let intention: String?
    public let intentionSource: String
    public let intentionDigest: String
    public let referenceCount: Int
    public let harness: String
    public let harnessID: String
    public let model: String
    public let route: String
    public let routeBilling: String
    public let startedAt: String?
    public let endedAt: String?
    public let durationSeconds: UInt64?
    public let exitCode: Int32?
    public let totalTokens: UInt64?
    public let harnessAttempts: UInt32?
    public let additionalCostUSD: Double?
    public let outcome: String?
    public let landed: Bool?
    public let filesChanged: Int?
    public let diffSummary: String?
    public let refusals: [ExecutionRefusal]
    public let nextAction: String?
}

public struct ExecutionActivity: Codable, Equatable, Sendable {
    public let schema: String
    public let executionID: String
    public let complete: Bool
    public let totalTokens: UInt64?
    /// Optional for compatibility with a locally installed helper from before
    /// the native workspace exposed its bounded source-file projection.
    public let fileCount: Int?
    public let filesTruncated: Bool?
    public let files: [ExecutionActivityFile]?
    public let entries: [ExecutionActivityEntry]
}

public struct ExecutionActivityFile: Codable, Equatable, Identifiable, Sendable {
    public let status: String
    public let path: String
    public let additions: UInt64?
    public let deletions: UInt64?

    public var id: String { "\(status):\(path)" }
}

public struct ExecutionActivityEntry: Codable, Equatable, Identifiable, Sendable {
    public let sequence: UInt64
    public let timestamp: String
    public let phase: String
    public let message: String

    public var id: UInt64 { sequence }
}

public struct ExecutionRefusal: Codable, Equatable, Sendable {
    public let check: String
    public let status: String
    public let gate: String?
    public let evidence: String?
}

extension JSONDecoder {
    static var tohseno: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .custom { path in
            let source = path.last?.stringValue ?? ""
            return TohsenoCodingKey(stringValue: tohsenoPropertyName(source))!
        }
        return decoder
    }
}

private struct TohsenoCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
        self.stringValue = stringValue
        intValue = nil
    }

    init?(intValue: Int) {
        stringValue = String(intValue)
        self.intValue = intValue
    }
}

private func tohsenoPropertyName(_ source: String) -> String {
    let components = source.split(separator: "_", omittingEmptySubsequences: false)
    guard components.count > 1, components.allSatisfy({ !$0.isEmpty }) else { return source }
    return components.dropFirst().reduce(String(components[0])) { result, component in
        let word = String(component)
        let suffix = switch word {
        case "id": "ID"
        case "ids": "IDs"
        case "url": "URL"
        case "usd": "USD"
        default: word.prefix(1).uppercased() + word.dropFirst()
        }
        return result + suffix
    }
}

extension JSONEncoder {
    static var tohseno: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return encoder
    }
}
