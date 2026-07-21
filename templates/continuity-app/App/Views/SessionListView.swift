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
        ScrollView {
            Text(store.text(for: session))
                .font(.system(.body, design: .serif))
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
        }
        .navigationTitle(session.endedAt.formatted(date: .abbreviated, time: .shortened))
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if AppConfig.shareCardEnabled,
               let image = ShareCard.render(text: store.text(for: session), appName: appDisplayName()) {
                ToolbarItem(placement: .primaryAction) {
                    ShareLink(
                        item: Image(uiImage: image),
                        preview: SharePreview("Share card", image: Image(uiImage: image))
                    )
                }
            }
        }
    }

    private func appDisplayName() -> String {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String) ?? "Writing"
    }
}
