import SwiftUI

struct ResultsView: View {
    let review: ChallengeReview
    let record: LocalProgressRecord
    let onPlayAgain: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
                Text("\(review.score)/\(review.total)")
                    .font(.system(size: 72, weight: .black, design: .rounded))
                Text("Decisions aligned with the run’s rule set.")
                    .foregroundStyle(.secondary)

                RankProgressView(record: record)
                ShareCardView(score: review.score, total: review.total, rank: RankProgression.rank(for: record))

                Button("Take another run", action: onPlayAgain)
                    .buttonStyle(.borderedProminent)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(DesignTokens.Spacing.large)
        }
    }
}
