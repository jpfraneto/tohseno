import SwiftUI

struct Rank: Equatable {
    let title: String
    let level: Int
    let progress: Double
}

enum RankProgression {
    static func rank(for record: LocalProgressRecord) -> Rank {
        let level = max(1, record.completedRuns / 3 + 1)
        let names = ["Scout", "Operator", "Strategist", "Veteran"]
        let title = names[min(level - 1, names.count - 1)]
        return Rank(
            title: title,
            level: level,
            progress: Double(record.completedRuns % 3) / 3.0
        )
    }
}

struct RankProgressView: View {
    let record: LocalProgressRecord

    var body: some View {
        let rank = RankProgression.rank(for: record)
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
            Text("RANK")
                .font(.caption.monospaced().weight(.semibold))
                .foregroundStyle(.secondary)
            Text("\(rank.title) · \(rank.level)")
                .font(DesignTokens.Typography.metric)
            ProgressView(value: rank.progress)
            Text("\(record.completedRuns) completed run\(record.completedRuns == 1 ? "" : "s")")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(DesignTokens.Spacing.medium)
        .background(.quaternary, in: RoundedRectangle(cornerRadius: 16))
    }
}
