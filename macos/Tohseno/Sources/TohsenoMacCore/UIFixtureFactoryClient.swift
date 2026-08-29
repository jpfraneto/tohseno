#if DEBUG
import Foundation

/// A deliberately local, compile-time-only workspace used to inspect native
/// layout without touching the owner's service, Keychain, apps, or iPhone.
public actor UIFixtureFactoryClient: FactoryServing {
    public static let appID = "ui_fixture"

    public init() {}

    public func workspace() async throws -> WorkspaceSnapshot {
        let execution = ExecutionSummary(
            executionID: "execution_fixture",
            shotID: Self.appID,
            state: "building",
            versionOrdinal: 1,
            startedAt: "2026-08-28T22:00:00Z",
            elapsedSeconds: 42,
            updatedAt: "2026-08-28T22:00:42Z"
        )
        let app = AppSummary(
            shotID: Self.appID,
            displayName: "Campanita",
            bundleIdentifier: "org.tohseno.campanita.app",
            icon: IconDescriptor(
                revision: "fixture",
                blobID: "fixture",
                mediaType: "image/png",
                byteLength: 0,
                placeholder: true
            ),
            expressionID: nil,
            latestVersionID: nil,
            latestVersionOrdinal: nil,
            latestVersionCreatedAt: nil,
            execution: execution,
            presentation: Presentation(
                state: .building,
                headline: "Making your app…",
                detail: "Writing the interface and checking it in Simulator."
            ),
            archived: false,
            retired: false,
            sortIndex: 0
        )
        return WorkspaceSnapshot(
            schema: "tohseno.workspace-snapshot/1",
            workspaceID: "ui_fixture",
            snapshotVersion: 1,
            generatedAt: "2026-08-28T22:00:42Z",
            serviceVersion: "ui-fixture",
            shots: [app],
            activeExecutions: [execution]
        )
    }

    public func factoryDefaults() async throws -> FactoryDefaults {
        FactoryDefaults(
            schema: "tohseno.factory-defaults/1",
            ready: true,
            harnessID: "fixture",
            harnessLabel: "Fixture",
            modelID: "fixture",
            modelLabel: "Fixture",
            routeID: "local",
            routeLabel: "Local",
            harnesses: []
        )
    }

    public func readiness() async throws -> ReadinessView {
        ReadinessView(
            schema: "tohseno.iphone-readiness-view/1",
            ready: true,
            step: "ready",
            headline: "Ready",
            detail: "Ready",
            primaryAction: nil,
            primaryLabel: nil
        )
    }

    public func activity(for appID: String) async throws -> ExecutionActivity? {
        ExecutionActivity(
            schema: "tohseno.execution-activity/1",
            executionID: "execution_fixture",
            complete: false,
            totalTokens: 8_412,
            fileCount: 4,
            filesTruncated: false,
            files: [
                ExecutionActivityFile(status: "A", path: "CampanitaApp.swift", additions: 38, deletions: 0),
                ExecutionActivityFile(status: "A", path: "Views/BellView.swift", additions: 96, deletions: 0),
                ExecutionActivityFile(status: "M", path: "project.yml", additions: 7, deletions: 2),
                ExecutionActivityFile(status: "A", path: "Tests/CampanitaTests.swift", additions: 41, deletions: 0),
            ],
            entries: [
                ExecutionActivityEntry(sequence: 1, timestamp: "2026-08-28T22:00:01Z", phase: "prepared", message: "Your intention is preserved."),
                ExecutionActivityEntry(sequence: 2, timestamp: "2026-08-28T22:00:08Z", phase: "building", message: "Writing the app source."),
                ExecutionActivityEntry(sequence: 3, timestamp: "2026-08-28T22:00:38Z", phase: "building", message: "Checking the iPhone build."),
            ]
        )
    }

    public func receipt(for appID: String) async throws -> ExecutionReceipt? { nil }
    public func icon(for appID: String) async throws -> Data? { nil }
    public func preview(for appID: String) async throws -> Data? { nil }
    public func managedStatus() async throws -> ManagedStatus { throw FactoryClientError.transport("UI fixture") }
    public func managedBalance() async throws -> ManagedBalance { throw FactoryClientError.transport("UI fixture") }
    public func managedCatalog() async throws -> ManagedCatalog { throw FactoryClientError.transport("UI fixture") }
    public func managedEstimate(model: String, privacy: String, intentionBytes: UInt64, referenceBytes: UInt64, appID: String?) async throws -> ManagedEstimate { throw FactoryClientError.transport("UI fixture") }
    public func managedCheckout(packID: String) async throws -> ManagedCheckout { throw FactoryClientError.transport("UI fixture") }
    public func registrySnapshot(appNames: [String]) async throws -> RegistrySnapshot { throw FactoryClientError.transport("UI fixture") }
    public func performReadinessAction(_ action: String) async throws -> ReadinessView { try await readiness() }
    public func create(_ draft: CreationDraft, commandID: String) async throws -> CommandReceipt { throw FactoryClientError.transport("UI fixture") }
    public func evolve(_ app: AppSummary, draft: EvolutionDraft, commandID: String) async throws -> CommandReceipt { throw FactoryClientError.transport("UI fixture") }
    public func openOnPhone(for appID: String) async throws {}
    public func openSource(for appID: String) async throws {}
    public func retire(appID: String) async throws {}
    public func restore(appID: String) async throws {}
    public func restartService() async throws {}
    public func openLegacyStudio() async throws {}
    public func configureCustomHarness(_ draft: CustomHarnessDraft) async throws {}
    public func configureLocalEndpoint(_ draft: LocalEndpointDraft) async throws {}
    public func events() async -> AsyncThrowingStream<Void, Error> {
        AsyncThrowingStream { continuation in
            let heartbeat = Task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(3_600))
                    guard !Task.isCancelled else { return }
                    continuation.yield(())
                }
            }
            continuation.onTermination = { _ in heartbeat.cancel() }
        }
    }
}
#endif
