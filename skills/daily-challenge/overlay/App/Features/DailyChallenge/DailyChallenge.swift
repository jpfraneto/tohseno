import Foundation

struct ChallengeDecision: Identifiable, Equatable {
    let id: Int
    let prompt: String
    let choices: [String]
    let preferredChoice: Int
}

struct ChallengeReview: Equatable {
    let score: Int
    let total: Int
    let decisions: [Int]
}

enum DailyChallengeEngine {
    static let sample = [
        ChallengeDecision(
            id: 1,
            prompt: "The signal is loud and the evidence is thin. What comes first?",
            choices: ["Commit immediately", "Define the invalidation", "Ask the crowd"],
            preferredChoice: 1
        ),
        ChallengeDecision(
            id: 2,
            prompt: "Your position moves against you. What do you protect?",
            choices: ["The original thesis", "The loss limit", "The screenshot"],
            preferredChoice: 1
        ),
        ChallengeDecision(
            id: 3,
            prompt: "A fast win creates urgency. What is the next move?",
            choices: ["Double exposure", "Pause and review", "Broadcast it"],
            preferredChoice: 1
        ),
        ChallengeDecision(
            id: 4,
            prompt: "Two signals disagree. What deserves weight?",
            choices: ["The newest signal", "The loudest voice", "The predeclared rule"],
            preferredChoice: 2
        ),
        ChallengeDecision(
            id: 5,
            prompt: "The setup no longer matches the plan. What closes the run?",
            choices: ["Hope", "The exit rule", "Another indicator"],
            preferredChoice: 1
        )
    ]

    static func review(
        decisions: [Int],
        challenge: [ChallengeDecision] = sample
    ) -> ChallengeReview {
        let score = zip(decisions, challenge).reduce(into: 0) { result, pair in
            if pair.0 == pair.1.preferredChoice {
                result += 1
            }
        }
        return ChallengeReview(
            score: score,
            total: challenge.count,
            decisions: decisions
        )
    }
}
