import SwiftUI

/// The past-sessions list: reverse chronological, tap to read.
/// A log, not a dashboard.
struct SessionListView: View {
    @EnvironmentObject private var store: SessionStore

    var body: some View {
        NavigationStack {
            Group {
                if store.sessions.isEmpty {
                    Text("Nothing kept yet.")
                        .foregroundStyle(.secondary)
                } else {
                    List(store.sessions) { session in
                        NavigationLink(value: session) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(session.endedAt, style: .date)
                                Text("\(session.characterCount) characters")
                                    .font(.footnote)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .listStyle(.plain)
                }
            }
            .navigationTitle("Sessions")
            .navigationBarTitleDisplayMode(.inline)
            .navigationDestination(for: SessionRecord.self) { session in
                SessionDetailView(session: session)
            }
        }
    }
}

/// Reading one kept session, with the share card when the module is on.
struct SessionDetailView: View {
    @EnvironmentObject private var store: SessionStore
    let session: SessionRecord

    var body: some View {
        let sessionText = store.text(for: session)
        let exportURLs = store.exportURLs(for: session)

        return ScrollView {
            Text(sessionText)
                .font(.system(.body, design: .serif))
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
        }
        .navigationTitle(session.endedAt.formatted(date: .abbreviated, time: .shortened))
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Menu {
                    if exportURLs.count == 2 {
                        ShareLink(items: exportURLs) {
                            Label("Export session files", systemImage: "doc.on.doc")
                        }
                    }
                    if AppConfig.shareCardEnabled,
                       let image = ShareCard.render(text: sessionText, appName: appDisplayName()) {
                        ShareLink(
                            item: Image(uiImage: image),
                            preview: SharePreview("Share card", image: Image(uiImage: image))
                        ) {
                            Label("Share image", systemImage: "photo")
                        }
                    }
                } label: {
                    Image(systemName: "square.and.arrow.up")
                }
                .accessibilityLabel("Share or export session")
            }
        }
    }

    private func appDisplayName() -> String {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String) ?? "Writing"
    }
}
