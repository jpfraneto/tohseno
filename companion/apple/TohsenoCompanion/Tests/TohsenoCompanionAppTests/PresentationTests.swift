import Foundation
import Testing
import TohsenoCompanionKit
@testable import TohsenoCompanionApp

@Suite("The phone and the Mac describe an app identically")
struct PresentationTests {
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
