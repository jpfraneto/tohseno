import Foundation
import LocalAuthentication

protocol RecoveryPhraseAuthorizing {
    func authorize() async throws -> Bool
}

struct SystemRecoveryPhraseAuthorizer: RecoveryPhraseAuthorizing {
    enum AuthorizationError: Error {
        case unavailable
    }

    func authorize() async throws -> Bool {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            throw error ?? AuthorizationError.unavailable
        }
        return try await context.evaluatePolicy(
            .deviceOwnerAuthentication,
            localizedReason: "Access recovery phrase identity settings"
        )
    }
}

/// Owns the short-lived authorization state for recovery-phrase display.
/// Closing the sheet or leaving the active scene always clears access.
@MainActor
final class RecoveryPhraseGate: ObservableObject {
    @Published private(set) var isRevealed = false
    @Published private(set) var isAuthorizing = false
    @Published private(set) var authorizationFailed = false

    private let authorizer: RecoveryPhraseAuthorizing
    private var requestGeneration = 0

    init(authorizer: RecoveryPhraseAuthorizing = SystemRecoveryPhraseAuthorizer()) {
        self.authorizer = authorizer
    }

    func requestReveal() async {
        requestGeneration += 1
        let generation = requestGeneration
        isRevealed = false
        authorizationFailed = false
        isAuthorizing = true
        defer {
            if requestGeneration == generation {
                isAuthorizing = false
            }
        }
        do {
            let authorized = try await authorizer.authorize()
            guard requestGeneration == generation else { return }
            isRevealed = authorized
            authorizationFailed = !isRevealed
        } catch {
            guard requestGeneration == generation else { return }
            authorizationFailed = true
        }
    }

    func hide() {
        requestGeneration += 1
        isRevealed = false
        isAuthorizing = false
        authorizationFailed = false
    }
}
