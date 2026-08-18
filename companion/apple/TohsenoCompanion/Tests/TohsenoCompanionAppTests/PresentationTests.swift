import Foundation
import Testing
import TohsenoCompanionKit
@testable import TohsenoCompanionApp

@Suite("The phone and the Mac describe an app identically")
struct PresentationTests {
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
        #expect(waiting.detail?.contains("close TOHSENO") == true)
    }

    @Test("Internal phases collapse into one human sentence")
    func internalPhasesCollapse() {
        for phase in [
            ExecutionStatus.planning, .conception, .materializing,
            .building, .testing, .verifying, .repairing,
        ] {
            let presentation = TohsenoPresentation.of(shot(version: 1, execution: phase))
            #expect(presentation.state == .building)
            #expect(presentation.headline == "Evolving anky…")
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
