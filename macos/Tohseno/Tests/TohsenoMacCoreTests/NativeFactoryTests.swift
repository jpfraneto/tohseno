import Foundation
import AppKit
import SwiftUI
import XCTest
@testable import TohsenoMacCore

final class NativeFactoryTests: XCTestCase {
    @MainActor
    func testNativeBuildWorkspaceRendersAtTheShippingWindowSize() async throws {
        let suite = "tohseno-render-fixture-\(UUID().uuidString)"
        let preferences = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { preferences.removePersistentDomain(forName: suite) }
        let model = TohsenoAppModel(client: UIFixtureFactoryClient(), preferences: preferences)
        model.route = .app(UIFixtureFactoryClient.appID)
        await model.reload()

        let size = NSSize(width: 862, height: 720)
        let host = NSHostingView(
            rootView: TohsenoBuildWorkspaceFixtureView(model: model)
                .frame(width: size.width, height: size.height)
                .environment(\.colorScheme, .dark)
        )
        host.frame = NSRect(origin: .zero, size: size)
        host.layoutSubtreeIfNeeded()
        let bitmap = try XCTUnwrap(host.bitmapImageRepForCachingDisplay(in: host.bounds))
        host.cacheDisplay(in: host.bounds, to: bitmap)
        let png = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
        XCTAssertGreaterThan(png.count, 20_000)
        if let output = ProcessInfo.processInfo.environment["TOHSENO_UI_FIXTURE_PNG"] {
            try png.write(to: URL(fileURLWithPath: output), options: .atomic)
        }
    }

    func testHumanPresentationHasExactlySixStates() {
        XCTAssertEqual(PresentedState.allCases.map(\.rawValue), [
            "waiting", "building", "ready_for_phone", "installing", "installed", "failed",
        ])
    }

    func testHumanPresentationMatchesRustFixture() throws {
        let repository = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent()
        let data = try Data(contentsOf: repository.appendingPathComponent("fixtures/presentation-v1.json"))
        let value = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let states = try XCTUnwrap(value["execution_states"] as? [String: String])
        XCTAssertEqual(Set(states.values), Set(PresentedState.allCases.map(\.rawValue)))
        XCTAssertEqual(states["waiting_for_device"], PresentedState.readyForPhone.rawValue)
        XCTAssertEqual(states["accepted"], PresentedState.installed.rawValue)
    }

    func testOnlyActiveReadinessStepsUseTheLoadingMark() {
        let view = { (step: String) in
            ReadinessView(
                schema: "tohseno.iphone-readiness-view/1", ready: false,
                step: step, headline: "Fixture", detail: "Fixture",
                primaryAction: nil, primaryLabel: nil
            )
        }
        XCTAssertTrue(view("building_readiness").isWorking)
        XCTAssertTrue(view("installing_readiness").isWorking)
        XCTAssertFalse(view("connect_iphone").isWorking)
        XCTAssertFalse(view("verify_installation").isWorking)
    }

    @MainActor
    func testActiveReadinessPollsUntilTheBackgroundCheckFinishes() async {
        let verify = ReadinessView(
            schema: "tohseno.iphone-readiness-view/1", ready: false,
            step: "verify_installation", headline: "Verify", detail: "Verify",
            primaryAction: "verify_installation", primaryLabel: "Verify iPhone"
        )
        let building = ReadinessView(
            schema: "tohseno.iphone-readiness-view/1", ready: false,
            step: "building_readiness", headline: "Building", detail: "Building",
            primaryAction: nil, primaryLabel: nil
        )
        let failed = ReadinessView(
            schema: "tohseno.iphone-readiness-view/1", ready: false,
            step: "verify_installation", headline: "Try again", detail: "Xcode failed",
            primaryAction: "verify_installation", primaryLabel: "Try Again"
        )
        let factory = FakeFactory(readinessResponses: [verify, building, failed])
        let model = TohsenoAppModel(client: factory)
        await model.reload()
        await model.performReadinessAction()
        XCTAssertEqual(model.readiness?.step, "building_readiness")
        try? await Task.sleep(for: .milliseconds(1_100))
        XCTAssertEqual(model.readiness?.step, "verify_installation")
        XCTAssertEqual(model.readiness?.detail, "Xcode failed")
    }

    @MainActor
    func testRouteRestoresAndRepairsAgainstAdoptedWorkspace() async {
        let suite = "tohseno-native-route-\(UUID().uuidString)"
        let preferences = try! XCTUnwrap(UserDefaults(suiteName: suite))
        defer { preferences.removePersistentDomain(forName: suite) }
        preferences.set("shot_fixture", forKey: "tohseno.selected-app-id")
        let factory = FakeFactory()
        let model = TohsenoAppModel(client: factory, preferences: preferences)
        XCTAssertEqual(model.route, .app("shot_fixture"))
        await model.reload()
        XCTAssertEqual(model.route, .app("shot_fixture"))
        preferences.set("missing", forKey: "tohseno.selected-app-id")
        let repaired = TohsenoAppModel(client: factory, preferences: preferences)
        await repaired.reload()
        XCTAssertEqual(repaired.route, .library)
    }

    @MainActor
    func testConcurrentCreateGestureSubmitsExactlyOnce() async {
        let factory = FakeFactory(createDelay: .milliseconds(80))
        let preferences = try! XCTUnwrap(UserDefaults(suiteName: "tohseno-submit-\(UUID().uuidString)"))
        let model = TohsenoAppModel(client: factory, preferences: preferences)
        model.creation.intention = "Make a tiny water reminder."
        async let first: Void = model.submitCreation()
        async let second: Void = model.submitCreation()
        _ = await (first, second)
        let calls = await factory.createCallCount()
        XCTAssertEqual(calls, 1)
        XCTAssertEqual(model.route, .app("shot_fixture"))
    }

    @MainActor
    func testEvolutionRestoresLastDurableAdvancedSelectionWithoutManagedConsent() async {
        let factory = FakeFactory(receipt: fixtureReceipt)
        let model = TohsenoAppModel(client: factory)
        await model.reload()
        let app = try! XCTUnwrap(model.apps.first)
        await model.prepareEvolution(for: app)
        let draft = try! XCTUnwrap(model.evolutions[app.id])
        XCTAssertEqual(draft.harness, "tohseno-managed")
        XCTAssertEqual(draft.model, "qwen3-coder")
        XCTAssertEqual(draft.managedPrivacy, "zdr")
        XCTAssertNil(draft.managedMaximumMicrousd)
        XCTAssertFalse(draft.managedConsent)
    }

    func testManagedDraftRequiresExplicitMaximumConsentAtClientBoundary() async throws {
        let factory = LoopbackFactoryClient(helperOverride: URL(fileURLWithPath: "/missing"))
        var draft = CreationDraft(intention: "Build it", harness: "tohseno-managed", model: "fixture")
        do {
            _ = try await factory.create(draft, commandID: "native_fixture")
            XCTFail("managed request without consent must fail before transport")
        } catch let error as FactoryClientError {
            guard case .invalidConfiguration = error else { return XCTFail("unexpected error: \(error)") }
        }
        draft.managedMaximumMicrousd = 10_000
        draft.managedConsent = true
    }

    func testConcurrentInitialRequestsLaunchOneNativeSessionHelper() async throws {
        let fixture = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-native-session-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: fixture, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: fixture) }
        let counter = fixture.appendingPathComponent("launches")
        let helper = fixture.appendingPathComponent("native-helper")
        let token = String(repeating: "a", count: 43)
        let credential = """
        {"schema":"tohseno.native-session/1","token":"\(token)","token_type":"TohsenoNative","client_id":"com.tohseno.mac","instance_id":"fixture","origin":"http://127.0.0.1:1","scopes":["factory.read","factory.mutate"],"expires_at":"2099-01-01T00:00:00Z"}
        """
        let script = """
        #!/bin/sh
        printf '1\\n' >> '\(counter.path)'
        sleep 0.2
        printf '%s\\n' '\(credential)'
        """
        try script.write(to: helper, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: helper.path)

        let factory = LoopbackFactoryClient(helperOverride: helper)
        async let workspace: WorkspaceSnapshot? = try? await factory.workspace()
        async let defaults: FactoryDefaults? = try? await factory.factoryDefaults()
        async let readiness: ReadinessView? = try? await factory.readiness()
        _ = await (workspace, defaults, readiness)

        let launches = try String(contentsOf: counter, encoding: .utf8)
            .split(separator: "\n")
        XCTAssertEqual(launches.count, 1)
    }

    func testConcurrentRejectedRequestsLaunchOneReplacementNativeSession() async throws {
        let fixture = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-native-rejection-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: fixture, withIntermediateDirectories: false)
        defer {
            NativeSessionTestURLProtocol.setHandler(nil)
            try? FileManager.default.removeItem(at: fixture)
        }
        let counter = fixture.appendingPathComponent("launches")
        let lock = fixture.appendingPathComponent("launch-lock")
        let helper = fixture.appendingPathComponent("native-helper")
        let rejectedToken = String(repeating: "a", count: 43)
        let replacementToken = String(repeating: "b", count: 43)
        let script = """
        #!/bin/sh
        while ! mkdir '\(lock.path)' 2>/dev/null; do sleep 1; done
        count=0
        if [ -f '\(counter.path)' ]; then count=$(sed -n '1p' '\(counter.path)'); fi
        count=$((count + 1))
        printf '%s\n' "$count" > '\(counter.path)'
        rmdir '\(lock.path)'
        if [ "$count" -eq 1 ]; then token='\(rejectedToken)'; else token='\(replacementToken)'; fi
        printf '{"schema":"tohseno.native-session/1","token":"%s","token_type":"TohsenoNative","client_id":"com.tohseno.mac","instance_id":"fixture","origin":"http://127.0.0.1:1","scopes":["factory.read","factory.mutate","events.read"],"expires_at":"2099-01-01T00:00:00Z"}\n' "$token"
        """
        try script.write(to: helper, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: helper.path)

        let rejection = NativeSessionRejectionBarrier(expected: 3, token: rejectedToken)
        NativeSessionTestURLProtocol.setHandler { request, protocolInstance in
            rejection.respond(to: request, with: protocolInstance)
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [NativeSessionTestURLProtocol.self]
        let factory = LoopbackFactoryClient(
            helperOverride: helper,
            urlSession: URLSession(configuration: configuration)
        )
        async let workspace: WorkspaceSnapshot? = try? await factory.workspace()
        async let defaults: FactoryDefaults? = try? await factory.factoryDefaults()
        async let readiness: ReadinessView? = try? await factory.readiness()
        _ = await (workspace, defaults, readiness)

        let launches = try String(contentsOf: counter, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        XCTAssertEqual(launches, "2")
    }

    func testPublishedSnakeCasePreservesSwiftAcronymProperties() throws {
        struct AcronymFixture: Decodable {
            let clientID: String
            let checkoutURL: String
            let additionalCostUSD: Double
            let managedMaximumMicrousd: UInt64
        }
        let data = Data(#"{"client_id":"com.tohseno.mac","checkout_url":"https://example.com","additional_cost_usd":1.25,"managed_maximum_microusd":5000000}"#.utf8)
        let decoded = try JSONDecoder.tohseno.decode(AcronymFixture.self, from: data)
        XCTAssertEqual(decoded.clientID, "com.tohseno.mac")
        XCTAssertEqual(decoded.checkoutURL, "https://example.com")
        XCTAssertEqual(decoded.additionalCostUSD, 1.25)
        XCTAssertEqual(decoded.managedMaximumMicrousd, 5_000_000)
    }

    func testRegistryHelperViewsDecodeWithoutClaimingPublication() throws {
        let identity = try JSONDecoder.tohseno.decode(
            BuilderIdentityView.self,
            from: Data(#"{"builder_id":"eip155:4663:0x1111111111111111111111111111111111111111","chain_id":4663,"account_address":"0x1111111111111111111111111111111111111111","identity_generation":"legacy_v0.7","scope":"local_only","authority_status":"test_only_non_authoritative","deployment_status":"legacy_offline_prediction","device_key_id":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","security_level":"software_test","test_only":true}"#.utf8)
        )
        let network = try JSONDecoder.tohseno.decode(
            RegistryNetworkStatus.self,
            from: Data(#"{"schema":"tohseno.network-status/2","product_version":"1.0.2","active_generation":"0.8.0","ready":false,"rpc_checked":false,"public_authority_available":true,"reason":"registry RPC is not implemented"}"#.utf8)
        )
        let record = try JSONDecoder.tohseno.decode(
            LocalRegistryRecord.self,
            from: Data(#"{"schema":"tohseno.registry-view/2","app_name":"fixture","shot_id":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","local_head":"0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","local_sequence":2,"local_state":"private","local_verified":true,"active_generation":"0.8.0","public_checked":false,"public_authority_available":true,"reason":"registry RPC is not implemented"}"#.utf8)
        )
        let snapshot = RegistrySnapshot(builder: identity, network: network, records: [record])
        XCTAssertEqual(snapshot.acceptedVersionCount, 2)
        XCTAssertEqual(identity.scope, "local_only")
        XCTAssertFalse(network.rpcChecked)
        XCTAssertFalse(record.publicChecked)
        XCTAssertTrue(record.localVerified)
    }

    func testExecutionActivityDecodesLiveFilesAndRemainsCompatibleWithOlderHelper() throws {
        let current = try JSONDecoder.tohseno.decode(
            ExecutionActivity.self,
            from: Data(#"{"schema":"tohseno.execution-activity/1","execution_id":"execution_fixture","complete":false,"file_count":1,"files_truncated":false,"files":[{"status":"A","path":"Fixture/App.swift","additions":12,"deletions":0}],"entries":[{"sequence":1,"timestamp":"2026-08-28T00:00:00Z","phase":"building","message":"Writing the interface."}]}"#.utf8)
        )
        XCTAssertEqual(current.fileCount, 1)
        XCTAssertEqual(current.files?.first?.path, "Fixture/App.swift")
        XCTAssertEqual(current.entries.first?.message, "Writing the interface.")

        let older = try JSONDecoder.tohseno.decode(
            ExecutionActivity.self,
            from: Data(#"{"schema":"tohseno.execution-activity/1","execution_id":"execution_fixture","complete":true,"entries":[]}"#.utf8)
        )
        XCTAssertNil(older.files)
        XCTAssertNil(older.fileCount)
    }

    func testRegistryHelperDrainsBoundedOutputWhileTheProcessRuns() async throws {
        let fixture = FileManager.default.temporaryDirectory
            .appendingPathComponent("tohseno-registry-helper-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: fixture, withIntermediateDirectories: false)
        defer { try? FileManager.default.removeItem(at: fixture) }
        let helper = fixture.appendingPathComponent("tohseno")
        let script = #"""
        #!/bin/sh
        case "$*" in
          *identity*)
            printf '%s' '{"builder_id":"eip155:4663:0x1111111111111111111111111111111111111111","chain_id":4663,"account_address":"0x1111111111111111111111111111111111111111","identity_generation":"legacy_v0.7","scope":"local_only","authority_status":"test_only_non_authoritative","deployment_status":"legacy_offline_prediction","device_key_id":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","security_level":"software_test","test_only":true,"padding":"'
            ;;
          *network*)
            printf '%s' '{"schema":"tohseno.network-status/2","product_version":"1.0.2","active_generation":"0.8.0","ready":false,"rpc_checked":false,"public_authority_available":true,"reason":"registry RPC is not implemented","padding":"'
            ;;
          *registry*)
            printf '%s' '{"schema":"tohseno.registry-view/2","app_name":"fixture","shot_id":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","local_head":"0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","local_sequence":2,"local_state":"private","local_verified":true,"active_generation":"0.8.0","public_checked":false,"public_authority_available":true,"reason":"registry RPC is not implemented","padding":"'
            ;;
          *) exit 2 ;;
        esac
        dd if=/dev/zero bs=20000 count=1 2>/dev/null | tr '\000' x
        printf '"}\n'
        """#
        try script.write(to: helper, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: helper.path)

        let snapshot = try await LoopbackFactoryClient(helperOverride: helper)
            .registrySnapshot(appNames: ["fixture"])
        XCTAssertEqual(snapshot.records.map(\.appName), ["fixture"])
        XCTAssertEqual(snapshot.acceptedVersionCount, 2)
        XCTAssertFalse(snapshot.network.rpcChecked)
    }

    @MainActor
    func testRegistryRouteRestoresAsAFirstClassMacDestination() async {
        let suite = "tohseno-native-registry-route-\(UUID().uuidString)"
        let preferences = try! XCTUnwrap(UserDefaults(suiteName: suite))
        defer { preferences.removePersistentDomain(forName: suite) }
        let factory = FakeFactory()
        let model = TohsenoAppModel(client: factory, preferences: preferences)
        model.route = .registry
        let restored = TohsenoAppModel(client: factory, preferences: preferences)
        XCTAssertEqual(restored.route, .registry)
    }

    func testComposersAdvertiseAndImplementReturnToSend() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Sources/TohsenoMacCore/RootView.swift")
        let source = try String(contentsOf: root, encoding: .utf8)
        XCTAssertTrue(source.contains("Return sends · Shift–Return adds a line"))
        XCTAssertGreaterThanOrEqual(source.components(separatedBy: ".shotSubmitOnReturn(").count - 1, 3)
        XCTAssertGreaterThanOrEqual(source.components(separatedBy: ".keyboardShortcut(.return, modifiers: [])").count - 1, 3)
        XCTAssertTrue(source.contains("press.modifiers.contains(.shift)"))
    }

    func testRequiredAccessibilityIdentifiersRemainPresent() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Sources/TohsenoMacCore/RootView.swift")
        let source = try String(contentsOf: root, encoding: .utf8)
        for identifier in [
            "readiness.primary", "create-app.sidebar", "creation.intention",
            "creation.submit", "evolution.intention", "evolution.submit",
            "advanced.harness", "advanced.managed.consent", "app.open-on-iphone",
            "app.workspace-tabs", "app.change", "app.files", "app.build-log",
            "app.preview", "app.iphone-handoff", "app.open-source",
            "registry.sidebar", "registry.quick.intention", "registry.quick.submit",
            "registry.builder", "registry.public-status",
        ] {
            XCTAssertTrue(source.contains("accessibilityIdentifier(\"\(identifier)\")"), identifier)
        }
    }
}

private actor FakeFactory: FactoryServing {
    private(set) var createCalls = 0
    let createDelay: Duration
    let receiptValue: ExecutionReceipt?
    var readinessResponses: [ReadinessView]

    init(
        createDelay: Duration = .zero,
        receipt: ExecutionReceipt? = nil,
        readinessResponses: [ReadinessView] = []
    ) {
        self.createDelay = createDelay
        self.receiptValue = receipt
        self.readinessResponses = readinessResponses
    }
    func createCallCount() -> Int { createCalls }

    func workspace() async throws -> WorkspaceSnapshot {
        WorkspaceSnapshot(
            schema: "tohseno.workspace-snapshot/1", workspaceID: "workspace_fixture",
            snapshotVersion: 1, generatedAt: "2026-08-27T00:00:00Z", serviceVersion: "1.0.2",
            shots: [fixtureApp], activeExecutions: []
        )
    }
    func factoryDefaults() async throws -> FactoryDefaults {
        FactoryDefaults(schema: "tohseno.factory-defaults/1", ready: true, harnessID: "fixture", harnessLabel: "Fixture", modelID: "default", modelLabel: "Default", routeID: "local", routeLabel: "Local", harnesses: [])
    }
    func readiness() async throws -> ReadinessView {
        if !readinessResponses.isEmpty {
            return readinessResponses.removeFirst()
        }
        return ReadinessView(schema: "tohseno.readiness/1", ready: true, step: "ready", headline: "Ready", detail: "Ready", primaryAction: nil, primaryLabel: nil)
    }
    func managedStatus() async throws -> ManagedStatus { throw FactoryClientError.transport("offline") }
    func managedBalance() async throws -> ManagedBalance { throw FactoryClientError.transport("offline") }
    func managedCatalog() async throws -> ManagedCatalog { throw FactoryClientError.transport("offline") }
    func managedEstimate(model: String, privacy: String, intentionBytes: UInt64, referenceBytes: UInt64, appID: String?) async throws -> ManagedEstimate { throw FactoryClientError.transport("offline") }
    func managedCheckout(packID: String) async throws -> ManagedCheckout { throw FactoryClientError.transport("offline") }
    func registrySnapshot(appNames: [String]) async throws -> RegistrySnapshot {
        let builder = BuilderIdentityView(
            builderID: "eip155:4663:0x1111111111111111111111111111111111111111",
            chainID: 4663,
            accountAddress: "0x1111111111111111111111111111111111111111",
            identityGeneration: "legacy_v0.7",
            scope: "local_only",
            authorityStatus: "test_only_non_authoritative",
            deploymentStatus: "legacy_offline_prediction",
            deviceKeyID: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            securityLevel: "software_test",
            testOnly: true
        )
        let network = RegistryNetworkStatus(
            schema: "tohseno.network-status/2",
            productVersion: "1.0.2",
            activeGeneration: "0.8.0",
            ready: false,
            rpcChecked: false,
            publicAuthorityAvailable: true,
            reason: "registry RPC is not implemented"
        )
        return RegistrySnapshot(builder: builder, network: network, records: [])
    }
    func performReadinessAction(_ action: String) async throws -> ReadinessView { try await readiness() }
    func create(_ draft: CreationDraft, commandID: String) async throws -> CommandReceipt {
        createCalls += 1
        try await Task.sleep(for: createDelay)
        return CommandReceipt(schema: "tohseno.create-shot-receipt/1", commandID: commandID, shotID: "shot_fixture", executionID: "execution_fixture", state: "accepted")
    }
    func evolve(_ app: AppSummary, draft: EvolutionDraft, commandID: String) async throws -> CommandReceipt { try await create(CreationDraft(intention: draft.intention), commandID: commandID) }
    func receipt(for appID: String) async throws -> ExecutionReceipt? { receiptValue }
    func activity(for appID: String) async throws -> ExecutionActivity? { nil }
    func icon(for appID: String) async throws -> Data? { nil }
    func preview(for appID: String) async throws -> Data? { nil }
    func openOnPhone(for appID: String) async throws {}
    func openSource(for appID: String) async throws {}
    func retire(appID: String) async throws {}
    func restore(appID: String) async throws {}
    func restartService() async throws {}
    func openLegacyStudio() async throws {}
    func configureCustomHarness(_ draft: CustomHarnessDraft) async throws {}
    func configureLocalEndpoint(_ draft: LocalEndpointDraft) async throws {}
    func events() async -> AsyncThrowingStream<Void, Error> { AsyncThrowingStream { $0.finish() } }

    private var fixtureApp: AppSummary {
        AppSummary(
            shotID: "shot_fixture", displayName: "Fixture", bundleIdentifier: "org.tohseno.genesis.fixture.app",
            icon: IconDescriptor(revision: "fixture", blobID: "fixture", mediaType: "image/png", byteLength: 1, placeholder: true),
            expressionID: "expression_fixture", latestVersionID: "version_fixture", latestVersionOrdinal: 1,
            latestVersionCreatedAt: "2026-08-27T00:00:00Z", execution: nil,
            presentation: Presentation(state: .installed, headline: "Fixture is on your iPhone ✓"),
            archived: false, retired: false, sortIndex: 0
        )
    }
}

private let fixtureReceipt = ExecutionReceipt(
    schema: "tohseno.execution-receipt/1", executionID: "execution_fixture",
    appName: "Fixture", versionOrdinal: 1, phase: "accepted",
    intention: "Build it", intentionSource: "exact", intentionDigest: "digest",
    referenceCount: 0, harness: "TOHSENO managed", harnessID: "tohseno-managed",
    model: "qwen3-coder", route: "managed-zdr", routeBilling: "managed_balance",
    startedAt: nil, endedAt: nil, durationSeconds: nil, exitCode: nil,
    totalTokens: nil, harnessAttempts: nil, additionalCostUSD: nil,
    outcome: nil, landed: true, filesChanged: nil, diffSummary: nil,
    refusals: [], nextAction: nil
)

private final class NativeSessionTestURLProtocol: URLProtocol, @unchecked Sendable {
    typealias Handler = @Sendable (URLRequest, NativeSessionTestURLProtocol) -> Void
    private static let lock = NSLock()
    nonisolated(unsafe) private static var handler: Handler?

    static func setHandler(_ value: Handler?) {
        lock.lock()
        handler = value
        lock.unlock()
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lock.lock()
        let handler = Self.handler
        Self.lock.unlock()
        guard let handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.resourceUnavailable))
            return
        }
        handler(request, self)
    }

    override func stopLoading() {}

    func respond(status: Int, body: Data) {
        let response = HTTPURLResponse(
            url: request.url!, statusCode: status, httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }
}

private final class NativeSessionRejectionBarrier: @unchecked Sendable {
    private let lock = NSLock()
    private let expected: Int
    private let token: String
    private var rejected: [NativeSessionTestURLProtocol] = []

    init(expected: Int, token: String) {
        self.expected = expected
        self.token = token
    }

    func respond(to request: URLRequest, with protocolInstance: NativeSessionTestURLProtocol) {
        let authorization = request.value(forHTTPHeaderField: "Authorization") ?? ""
        guard authorization == "TohsenoNative \(token)" else {
            protocolInstance.respond(status: 200, body: Data("{}".utf8))
            return
        }
        lock.lock()
        rejected.append(protocolInstance)
        let ready = rejected.count == expected ? rejected : []
        if !ready.isEmpty { rejected.removeAll() }
        lock.unlock()
        guard !ready.isEmpty else { return }
        let body = Data(#"{"code":"native_session_rejected","message":"expired"}"#.utf8)
        for pending in ready {
            pending.respond(status: 403, body: body)
        }
    }
}
