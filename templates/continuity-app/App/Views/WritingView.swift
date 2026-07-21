import SwiftUI

/// The writing surface. Cursor ready on open; a Done action ends the
/// session. Everything else stays out of the way.
struct WritingView: View {
    @EnvironmentObject private var store: SessionStore
    @FocusState private var focused: Bool
    @State private var text = ""
    @State private var showingSessions = false
    @State private var showingSettings = false

    var body: some View {
        NavigationStack {
            TextEditor(text: $text)
                .font(.system(.body, design: .serif))
                .scrollContentBackground(.hidden)
                .padding(.horizontal, 12)
                .focused($focused)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Button("Sessions") { showingSessions = true }
                    }
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Settings") { showingSettings = true }
                    }
                    ToolbarItem(placement: .primaryAction) {
                        Button("Done") { finish() }
                            .fontWeight(.semibold)
                            .disabled(text.isEmpty)
                    }
                }
                .sheet(isPresented: $showingSessions) {
                    SessionListView()
                }
                .sheet(isPresented: $showingSettings) {
                    SettingsView()
                }
                .onAppear {
                    text = store.beginOrResumeDraft().text
                    focused = true
                }
                .onChange(of: text) {
                    store.checkpoint(text: text)
                }
        }
    }

    private func finish() {
        store.finishDraft()
        text = store.beginOrResumeDraft().text
        focused = true
    }
}
