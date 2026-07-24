import SwiftUI

struct ShareCardView: View {
    let score: Int
    let total: Int
    let rank: Rank

    private var shareText: String {
        "Today’s run: \(score)/\(total) · Rank \(rank.title) \(rank.level)"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.medium) {
            Text("TODAY’S RUN")
                .font(.caption.monospaced().weight(.semibold))
            Text("\(score)/\(total)")
                .font(.system(size: 42, weight: .black, design: .rounded))
            Text("\(rank.title) · Rank \(rank.level)")
                .foregroundStyle(.secondary)
            ShareLink(item: shareText) {
                Label("Share result", systemImage: "square.and.arrow.up")
            }
            .buttonStyle(.bordered)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(DesignTokens.Spacing.large)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 20))
    }
}
