import Foundation
import Testing
import TohsenoCompanionKit
@testable import TohsenoCompanionApp
#if canImport(AppKit)
import AppKit
import SwiftUI
#endif

@Suite("The phone and the Mac describe an app identically")
struct PresentationTests {
    #if canImport(AppKit)
    @MainActor
    @Test("The pocket workshop renders at an iPhone-sized fixture")
    func pocketWorkshopRenders() async throws {
        let backend = StubBackend(shots: [shot(version: 3)])
        let subject = await model(backend)
        let size = NSSize(width: 390, height: 844)
        let host = NSHostingView(
            rootView: YourAppsView(
                model: subject,
                openNetwork: {},
                openUpdates: {},
                openKeeper: {}
            )
            .frame(width: size.width, height: size.height)
            .preferredColorScheme(.dark)
        )
        host.frame = NSRect(origin: .zero, size: size)
        host.layoutSubtreeIfNeeded()
        let bitmap = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
        host.cacheDisplay(in: host.bounds, to: bitmap)
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        #expect(png.count > 12_000)
        if let output = ProcessInfo.processInfo.environment["TOHSENO_COMPANION_WORKSHOP_FIXTURE_PNG"] {
            try png.write(to: URL(fileURLWithPath: output), options: .atomic)
        }
    }
    #endif

    @Test("The Release app owns a modern full-screen launch configuration")
    func fullScreenLaunchConfiguration() throws {
        let package = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let infoData = try Data(contentsOf: package.appendingPathComponent("App/Info.plist"))
        let info = try #require(
            PropertyListSerialization.propertyList(from: infoData, format: nil) as? [String: Any]
        )
        #expect(info["UILaunchScreen"] != nil)
        #expect(info["CFBundleURLTypes"] != nil)
        #expect(info["NSMicrophoneUsageDescription"] != nil)
        #expect(info["NSSpeechRecognitionUsageDescription"] != nil)

        let project = try String(
            contentsOf: package.appendingPathComponent(
                "App/TohsenoCompanion.xcodeproj/project.pbxproj"
            ),
            encoding: .utf8
        )
        #expect(project.components(separatedBy: "GENERATE_INFOPLIST_FILE = NO;").count - 1 == 2)
        #expect(project.components(separatedBy: "INFOPLIST_FILE = Info.plist;").count - 1 == 2)
    }

    @Test("The paired Companion never restores the removed trial gate")
    func noTrialGateInTheProductView() throws {
        let package = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: package.appendingPathComponent(
                "Sources/TohsenoCompanionApp/CompanionRootView.swift"
            ),
            encoding: .utf8
        )
        #expect(!source.contains("CompanionEntitlementView"))
        #expect(!source.contains("Continue with TOHSENO Pro"))
        #expect(source.contains("case .entitlementDecision, .trialEnded, .apps, .create, .app:"))
    }

    @Test("The pocket workshop preserves every former top-level capability")
    func workshopCapabilityMigration() throws {
        let package = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: package.appendingPathComponent(
                "Sources/TohsenoCompanionApp/CompanionRootView.swift"
            ),
            encoding: .utf8
        )
        for label in ["Workshop", "Network", "Updates", "Keeper", "One Shot", "Take the Shot"] {
            #expect(source.contains("\"\(label)\""))
        }
        #expect(source.contains("KeeperInboxView"))
        #expect(source.contains("CompanionWorkshopProjection"))
        #expect(!source.contains(".tabItem { Label(\"Apps\""))
        #expect(!source.contains(".tabItem { Label(\"Registry\""))
        #expect(!source.contains(".tabItem { Label(\"Profile\""))
    }

    @Test("The pocket workshop chapter is only a projection of the six real app states")
    func workshopProjection() {
        let expected: [(TohsenoPresentedState, CompanionWorkshopChapter)] = [
            (.waiting, .waitingForMac), (.building, .building),
            (.readyForPhone, .readyForPhone), (.installing, .installing),
            (.installed, .installed), (.failed, .attention),
        ]
        for (state, chapter) in expected {
            let app = CompanionWorkshopApp(
                id: "fixture", name: "Fixture", state: state, headline: "Fixture state"
            )
            let projection = CompanionWorkshopProjection(
                apps: [app], macConnection: .connected, keeperAvailable: true,
                publicEvidenceObserved: nil, unreadUpdates: 0
            )
            #expect(projection.chapter == chapter)
            #expect(projection.apps.map(\.state) == [state])
        }
        let empty = CompanionWorkshopProjection(
            apps: [], macConnection: .disconnected, keeperAvailable: false,
            publicEvidenceObserved: false, unreadUpdates: 2
        )
        #expect(empty.chapter == .takeShot)
        #expect(empty.threshold == .unavailable)
        #expect(empty.unreadUpdates == 2)
    }

    @Test("The Companion consumes the same deterministic workshop scenario contract")
    func sharedWorkshopFixtureCatalog() throws {
        let repository = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: repository.appendingPathComponent("fixtures/workshop-scenes-v1.json"))
        let document = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let scenes = try #require(document["scenes"] as? [[String: String]])
        let companion = scenes.filter { ($0["surface"] ?? "").hasPrefix("companion_") }
        #expect(Set(companion.compactMap { $0["id"] }) == [
            "waiting_for_mac", "claim_available_inactive", "mac_offline_from_companion",
        ])
        #expect(companion.first { $0["id"] == "waiting_for_mac" }?["evidence"]
            == "presentation.state=waiting")
        #expect(companion.first { $0["id"] == "mac_offline_from_companion" }?["evidence"]
            == "companion.connection=disconnected")
    }

    @Test("First run brings the iPhone into the same workshop")
    func firstRunWorkshop() throws {
        let package = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: package.appendingPathComponent(
                "Sources/TohsenoCompanionApp/FirstRunView.swift"
            ),
            encoding: .utf8
        )
        #expect(source.contains("BRING YOUR IPHONE INTO THE WORKSHOP"))
        #expect(source.contains("Mac factory"))
        #expect(source.contains("This iPhone"))
        #expect(source.contains("workshop.first-run-connection"))
    }

    @Test("Every execution state the Mac can send is projected the same way here")
    func matchesTheSharedTable() throws {
        let table = try PresentationFixture.executionStates()
        for status in ExecutionStatus.allCases {
            let expected = try #require(
                table[status.rawValue],
                "the shared table is missing \(status.rawValue)"
            )
            #expect(TohsenoPresentedState.from(status).rawValue == expected)
        }
        // `cancelled` exists inside the factory but is never representable on
        // the wire: the Mac collapses it to `failed` before it reaches a phone.
        #expect(table["cancelled"] == "failed")
        #expect(ExecutionStatus(rawValue: "cancelled") == nil)
    }

    @Test("The table covers exactly the six human states")
    func coversSixStates() throws {
        let table = try PresentationFixture.executionStates()
        #expect(Set(table.values) == Set(TohsenoPresentedState.allCases.map(\.rawValue)))
    }

    @Test("An app with no execution reads from its accepted version")
    func settledApps() {
        #expect(TohsenoPresentation.of(shot(version: 3)).state == .installed)
        #expect(TohsenoPresentation.of(shot(version: 3)).headline == "anky updated ✓")
        #expect(TohsenoPresentation.of(shot(version: nil)).state == .waiting)
        let adopted = ShotSummary(
            shotID: "project_fixture",
            displayName: "Fixture",
            kind: .adoptedProject,
            sourceState: "state_fixture",
            iconRevision: 1,
            sortIndex: 0,
            supportedCompanionActions: [.workspaceRead, .shotEvolve]
        )
        #expect(TohsenoPresentation.of(adopted).state == .installed)
    }

    @Test("Waiting for the cable asks for the cable and claims nothing")
    func waitingForDevice() {
        let ready = TohsenoPresentation.of(shot(version: 2, execution: .waitingForDevice))
        #expect(ready.state == .readyForPhone)
        #expect(ready.headline == "anky is ready.")
        #expect(ready.detail == "Connect this iPhone to your Mac to install the update.")
        #expect(!ready.state.inFlight)
    }

    @Test("Nothing that reached only this phone is described as building")
    func waitingForMac() {
        let waiting = TohsenoPresentation.waitingForMac(appName: "anky")
        #expect(waiting.state == .waiting)
        #expect(waiting.headline == "Waiting for your Mac…")
        #expect(waiting.detail?.contains("close Tohseno") == true)
    }

    @Test("Internal phases collapse into one human sentence")
    func internalPhasesCollapse() {
        for phase in [
            ExecutionStatus.planning, .conception, .materializing,
            .building, .testing, .verifying, .repairing,
        ] {
            let presentation = TohsenoPresentation.of(shot(version: 1, execution: phase))
            #expect(presentation.state == .building)
            #expect(presentation.headline == "Building anky…")
        }
    }
}

func shot(
    name: String = "anky",
    version: UInt64?,
    execution: ExecutionStatus? = nil
) -> ShotSummary {
    ShotSummary(
        shotID: "shot_anky",
        displayName: name,
        kind: .factoryShot,
        iconRevision: 1,
        expressionID: version == nil ? nil : "expression_anky",
        latestVersionID: version == nil ? nil : "version_anky",
        latestVersionOrdinal: version,
        latestVersionCreatedAt: version == nil ? nil : "2026-08-18T00:00:00Z",
        execution: execution.map {
            ExecutionSummary(
                executionID: "execution_anky",
                shotID: "shot_anky",
                state: $0,
                updatedAt: "2026-08-18T00:00:00Z",
                failureCode: $0 == .failed ? "execution_failed" : nil
            )
        },
        sortIndex: 0,
        supportedCompanionActions: [.workspaceRead]
    )
}
