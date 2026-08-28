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

public struct AppSummary: Codable, Equatable, Identifiable, Sendable {
    public let shotID: String
    public let displayName: String
    public let bundleIdentifier: String?
    public let icon: IconDescriptor
    public let expressionID: String?
    public let latestVersionID: String?
    public let latestVersionOrdinal: UInt64?
    public let latestVersionCreatedAt: String?
    public let execution: ExecutionSummary?
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

public struct RegistrySnapshot: Equatable, Sendable {
    public let builder: BuilderIdentityView
    public let network: RegistryNetworkStatus
    public let records: [LocalRegistryRecord]

    public init(
        builder: BuilderIdentityView,
        network: RegistryNetworkStatus,
        records: [LocalRegistryRecord]
    ) {
        self.builder = builder
        self.network = network
        self.records = records.sorted {
            ($0.appName, $0.shotID) < ($1.appName, $1.shotID)
        }
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

public struct ReadinessView: Codable, Equatable, Sendable {
    public let schema: String
    public let ready: Bool
    public let step: String
    public let headline: String
    public let detail: String
    public let primaryAction: String?
    public let primaryLabel: String?

    public var isWorking: Bool {
        step == "building_readiness" || step == "installing_readiness"
    }
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
