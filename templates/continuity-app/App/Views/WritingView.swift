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
                .alert(
                    "Writing is not saved yet",
                    isPresented: Binding(
                        get: { store.persistenceError },
                        set: { if !$0 { store.dismissPersistenceError() } }
                    )
                ) {
                    Button("OK") { store.dismissPersistenceError() }
                } message: {
                    Text("Your text is still on screen. Free storage if needed, then edit or tap Done to retry.")
                }
        }
    }

    private func finish() {
        store.finishDraft()
        text = store.beginOrResumeDraft().text
        focused = true
    }
}
