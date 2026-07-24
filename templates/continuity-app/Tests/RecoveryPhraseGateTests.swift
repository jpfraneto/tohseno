import XCTest
@testable import Writing

private struct FakeRecoveryPhraseAuthorizer: RecoveryPhraseAuthorizing {
    enum Failure: Error {
        case cancelled
    }

    let result: Result<Bool, Error>

    func authorize() async throws -> Bool {
        try result.get()
    }
}

private struct DelayedRecoveryPhraseAuthorizer: RecoveryPhraseAuthorizing {
    func authorize() async throws -> Bool {
        try await Task.sleep(nanoseconds: 50_000_000)
        return true
    }
}

@MainActor
final class RecoveryPhraseGateTests: XCTestCase {
    func testOnlySuccessfulDeviceOwnerAuthenticationRevealsPhrase() async {
        let gate = RecoveryPhraseGate(
            authorizer: FakeRecoveryPhraseAuthorizer(result: .success(true))
        )

        await gate.requestReveal()

        XCTAssertTrue(gate.isRevealed)
        XCTAssertFalse(gate.authorizationFailed)
        XCTAssertFalse(gate.isAuthorizing)
    }

    func testCancellationKeepsPhraseHidden() async {
        let gate = RecoveryPhraseGate(
            authorizer: FakeRecoveryPhraseAuthorizer(
                result: .failure(FakeRecoveryPhraseAuthorizer.Failure.cancelled)
            )
        )

        await gate.requestReveal()

        XCTAssertFalse(gate.isRevealed)
        XCTAssertTrue(gate.authorizationFailed)
    }

    func testLeavingRevealSurfaceRevokesSuccessfulAccess() async {
        let gate = RecoveryPhraseGate(
            authorizer: FakeRecoveryPhraseAuthorizer(result: .success(true))
        )
        await gate.requestReveal()
        XCTAssertTrue(gate.isRevealed)

        gate.hide()

        XCTAssertFalse(gate.isRevealed)
        XCTAssertFalse(gate.authorizationFailed)
    }

    func testLeavingDuringAuthenticationInvalidatesThePendingResult() async {
        let gate = RecoveryPhraseGate(
            authorizer: DelayedRecoveryPhraseAuthorizer()
        )
        let request = Task { await gate.requestReveal() }
        await Task.yield()

        gate.hide()
        await request.value

        XCTAssertFalse(gate.isRevealed)
        XCTAssertFalse(gate.authorizationFailed)
        XCTAssertFalse(gate.isAuthorizing)
    }
}
