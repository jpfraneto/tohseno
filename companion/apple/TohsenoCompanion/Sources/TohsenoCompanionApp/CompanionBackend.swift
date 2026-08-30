import Foundation
import TohsenoCompanionKit

/// The narrow slice of `TohsenoCompanionKit` the product surface needs.
///
/// This is a seam, not a second client: the only shipping implementation is
/// the SDK actor below. It exists so the product's behaviour — one tap, no
/// confirmation, offline waiting, human failure copy — can be tested without
/// standing up relay, crypto, and Keychain fakes that the SDK already tests.
public protocol CompanionBackend: Sendable {
    func synchronizedWorkspace() async throws -> WorkspaceSnapshot
    func reconcile() async throws
    func iconBytes(for descriptor: IconDescriptor) async throws -> Data?
    func requestShotCreation(_ request: CreateShotRequest) async throws -> CommandReceipt
    func requestEvolution(_ request: EvolutionRequest) async throws -> CommandReceipt
    func requestProjectEvolution(_ request: ProjectEvolutionRequest) async throws -> CommandReceipt
    func announceBuilderDevice(_ builderDevice: BuilderDeviceAnnouncement, commandID: String) async throws -> CommandReceipt
    func approvePublication(
        jobID: String,
        catalog: BuilderDeviceSignature,
        registry: BuilderDeviceSignature,
        approvedAt: String,
        commandID: String
    ) async throws -> CommandReceipt
    func requestNetworkRelease(
        action: NetworkReleaseAction,
        shotID: String,
        releaseDigest: String,
        commandID: String
    ) async throws -> CommandReceipt
    /// Signed commands this phone has written that the Mac has not acknowledged.
    func unacknowledgedCommandCount() async throws -> Int
    func createIdentity() async throws -> RecoveryPhrase
    func pair(invitation: String, displayName: String) async throws
    func startSynchronization() async throws
    var connectionStates: AsyncStream<CompanionConnectionState> { get }
    var events: AsyncStream<WorkspaceEvent> { get }
}

extension TohsenoCompanionClient: CompanionBackend {
    public func synchronizedWorkspace() async throws -> WorkspaceSnapshot {
        try await currentWorkspace()
    }

    public func iconBytes(for descriptor: IconDescriptor) async throws -> Data? {
        try await iconBlob(for: descriptor)?.bytes
    }

    public func pair(invitation: String, displayName: String) async throws {
        try await pair(with: invitation, displayName: displayName)
    }

    public func startSynchronization() async throws {
        try await startForegroundSynchronization()
    }

    public nonisolated var connectionStates: AsyncStream<CompanionConnectionState> {
        connectionState
    }

    public nonisolated var events: AsyncStream<WorkspaceEvent> {
        workspaceEvents
    }
}

/// Build the shipping client against the owner's private relay allowlist.
///
/// Storage lives under Application Support and is encrypted with the
/// companion storage key; the identity itself stays in the Keychain.
public enum CompanionClientFactory {
    public static let productionRelay = "https://companion.tohseno.com"

    public static func make(relayOrigin: URL? = nil) throws -> TohsenoCompanionClient {
        let stateURL = URL.applicationSupportDirectory
            .appending(path: "TOHSENO", directoryHint: .isDirectory)
            .appending(path: "companion-state.bin")
        let stateStore = try FileCompanionStateStore(fileURL: stateURL)
        let payloadStore = try FileCompanionPayloadStore(
            directoryURL: stateURL.deletingLastPathComponent().appending(path: "outbox")
        )
        let selectedRelay = relayOrigin ?? URL(string: productionRelay)!
#if DEBUG
        // A physical development build may use the same content-blind relay
        // on its paired Mac. Release builds remain HTTPS-only.
        let allowLocalNetworkHTTP = relayOrigin?.scheme == "http"
#else
        let allowLocalNetworkHTTP = false
#endif
        let endpoint = try RelayEndpoint(
            id: "official-v1",
            baseURL: selectedRelay,
            allowLocalNetworkHTTP: allowLocalNetworkHTTP
        )
        return TohsenoCompanionClient(
            stateStore: stateStore,
            payloadStore: payloadStore,
            relayAllowlist: try RelayAllowlist([endpoint])
        )
    }
}
