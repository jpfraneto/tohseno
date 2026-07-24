import SwiftUI

struct DailyChallengeView: View {
    @StateObject private var progress = LocalProgressStore()
    @State private var decisionIndex = 0
    @State private var decisions: [Int] = []
    @State private var review: ChallengeReview?

    private let challenge = DailyChallengeEngine.sample

    var body: some View {
        Group {
            if let review {
                ResultsView(
                    review: review,
                    record: progress.record,
                    onPlayAgain: reset
                )
            } else {
                decision
            }
        }
        .navigationTitle(review == nil ? "Today’s Run" : "Result")
    }

    private var decision: some View {
        let current = challenge[decisionIndex]
        return VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
            Text("DECISION \(decisionIndex + 1) OF \(challenge.count)")
                .font(.caption.monospaced().weight(.semibold))
                .foregroundStyle(.secondary)
            Text(current.prompt)
                .font(DesignTokens.Typography.title)
            ForEach(Array(current.choices.enumerated()), id: \.offset) { index, choice in
                Button {
                    choose(index)
                } label: {
                    Text(choice)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(DesignTokens.Spacing.medium)
                }
                .buttonStyle(.bordered)
            }
            Spacer()
        }
        .padding(DesignTokens.Spacing.large)
    }

    private func choose(_ choice: Int) {
        decisions.append(choice)
        if decisionIndex + 1 < challenge.count {
            decisionIndex += 1
            return
        }
        let result = DailyChallengeEngine.review(
            decisions: decisions,
            challenge: challenge
        )
        progress.record(score: result.score, total: result.total)
        review = result
    }

    private func reset() {
        decisionIndex = 0
        decisions = []
        review = nil
    }
}
