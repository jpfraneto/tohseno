import Foundation
import CryptoKit
import XCTest
@testable import TohsenoCompanionKit

final class WorkshopRequestTests: XCTestCase {
    func testOldExecutionCannotCompleteNewRequestAndHistorySurvivesEncryption() throws {
        var request = try XCTUnwrap(WorkshopRequest(commandID: "command_new",
            payload: .shotCreateRequest(suggestedName: nil, intention: "Make a timer", references: []),
            createdAt: "2026-09-08T12:00:00Z"))
        let old = ExecutionSummary(executionID: "execution_old", shotID: "shot_timer", state: .accepted,
                                   updatedAt: "2026-09-08T12:01:00Z")
        request.apply(old)
        XCTAssertNil(request.execution)
        XCTAssertTrue(request.awaitingMac)
        request.apply(CommandReceipt(commandID: "command_other", state: .rejected), at: old.updatedAt)
        XCTAssertTrue(request.awaitingMac)
        request.apply(CommandReceipt(commandID: "command_new", state: .accepted,
                                     shotID: "shot_timer", executionID: "execution_new"), at: old.updatedAt)
        request.apply(old)
        XCTAssertNil(request.execution)
        XCTAssertFalse(request.awaitingMac)
        let working = ExecutionSummary(executionID: "execution_new", shotID: "shot_timer", state: .materializing,
                                       updatedAt: "2026-09-08T12:02:00Z")
        request.apply(working)
        request.apply(working)
        XCTAssertEqual(request.activity.count, 3, "Repeated reports must not duplicate the log")
        let failed = ExecutionSummary(executionID: "execution_new", shotID: "shot_timer", state: .failed,
                                      updatedAt: "2026-09-08T12:03:00Z", failureCode: "build_failed")
        request.apply(failed)
        request.apply(working)
        XCTAssertEqual(request.execution, failed)
        XCTAssertTrue(request.isFinished)
        var state = CompanionPersistentState()
        state.workshopRequests = [request]
        let key = SymmetricKey(size: .bits256)
        let sealed = try CompanionStateCodec.seal(state, key: key)
        XCTAssertFalse(String(decoding: sealed, as: UTF8.self).contains("Make a timer"))
        XCTAssertEqual(try CompanionStateCodec.open(sealed, key: key).workshopRequests, [request])
    }

    func testRejectionIsRetainedWithoutAShotAndDoesNotClaimWorkStarted() throws {
        var request = try XCTUnwrap(WorkshopRequest(commandID: "command_new",
            payload: .shotCreateRequest(suggestedName: nil, intention: "Make a timer", references: []),
            createdAt: "2026-09-08T12:00:00Z"))
        request.apply(CommandReceipt(commandID: "command_new", state: .rejected,
                                     rejectionCode: "command_rejected"), at: "2026-09-08T12:01:00Z")
        XCTAssertTrue(request.isFinished)
        XCTAssertNil(request.shotID)
        XCTAssertNil(request.execution)
        XCTAssertEqual(request.activity.count, 2)
        XCTAssertTrue(request.activity.last!.message.contains("command_rejected"))
    }

    func testIncrementalExecutionPreservesAdoptedSourceAndHistory() throws {
        let history = try JSONDecoder().decode([EvolutionHistorySummary].self, from: Data(#"[{"evolution_id":"evolution_previous","requested_at":"2026-09-08T12:00:00Z","request_summary":"Keep the writing visible","status":"completed"}]"#.utf8))
        let shot = ShotSummary(shotID: "project_anky", displayName: "Anky", kind: .adoptedProject,
            sourceState: "source_current", iconRevision: 0, recentEvolutions: history, sortIndex: 0,
            supportedCompanionActions: [.shotEvolve])
        let snapshot = WorkspaceSnapshot(workspaceID: "workspace_fixture", snapshotVersion: 1,
            generatedAt: "2026-09-08T12:00:00Z", serviceVersion: "1.2.1", shots: [shot], activeExecutions: [],
            deviceCapabilityState: DeviceCapabilityState(deviceID: "device_fixture", capabilityID: "capability_fixture",
                revocationEpoch: 0, allowedActions: [.workspaceRead, .shotEvolve], revoked: false), nextCursor: 1)
        let execution = ExecutionSummary(executionID: "evolution_next", shotID: shot.shotID, state: .building,
                                         updatedAt: "2026-09-08T12:01:00Z")
        let updated = snapshot.updatingExecution(execution)
        XCTAssertEqual(updated.shots.first?.sourceState, shot.sourceState)
        XCTAssertEqual(updated.shots.first?.recentEvolutions, history)
        XCTAssertEqual(updated.shots.first?.execution, execution)
        try updated.shots.first?.validate()
    }

    func testOlderEncryptedStateStillDecodes() throws {
        let legacy = Data(#"{"schema":"tohseno.companion-ios-state/1"}"#.utf8)
        let state = try JSONDecoder().decode(CompanionPersistentState.self, from: legacy)
        XCTAssertTrue(state.workshopRequests.isEmpty)
    }
}
