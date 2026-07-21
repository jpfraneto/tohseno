import SwiftUI

/// ShareCard module — ON by default (`AppConfig.shareCardEnabled`).
///
/// Renders a styled snippet of a kept session to an image, entirely on
/// device, and hands it to the system share sheet. Distribution is part of
/// the app, not an afterthought — and it never touches the network.
enum ShareCard {
    /// The largest snippet that stays readable on a card.
    private static let snippetLimit = 280

    /// Renders the share image for a session. Runs on the main actor
    /// because `ImageRenderer` requires it.
    @MainActor
    static func render(text: String, appName: String) -> UIImage? {
        let renderer = ImageRenderer(content: ShareCardView(snippet: snippet(of: text), appName: appName))
        renderer.scale = 3
        return renderer.uiImage
    }

    static func snippet(of text: String) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count > snippetLimit else { return trimmed }
        return String(trimmed.prefix(snippetLimit)).trimmingCharacters(in: .whitespacesAndNewlines) + "…"
    }
}

/// The card itself: the words carry it; the chrome stays out of the way.
struct ShareCardView: View {
    let snippet: String
    let appName: String

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            Text(snippet)
                .font(.system(.title3, design: .serif))
                .foregroundStyle(.black)
                .frame(maxWidth: .infinity, alignment: .leading)
            Spacer(minLength: 0)
            Text(appName.uppercased())
                .font(.system(.caption, design: .monospaced))
                .kerning(2)
                .foregroundStyle(.black.opacity(0.45))
        }
        .padding(40)
        .frame(width: 400, height: 500, alignment: .topLeading)
        .background(Color(red: 1, green: 0.99, blue: 0.96))
    }
}
