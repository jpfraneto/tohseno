import SwiftUI

/// Settings holds exactly two identity actions: reveal the recovery phrase
/// (behind a warning) and restore from a phrase (replaces local identity).
struct SettingsView: View {
    @EnvironmentObject private var identityManager: IdentityManager
    @State private var showingReveal = false
    @State private var revealConfirmed = false
    @State private var showingRestore = false
    @State private var restorePhrase = ""
    @State private var restoreError = false

    var body: some View {
        NavigationStack {
            List {
                Section("Identity") {
                    LabeledContent("User ID", value: identityManager.identity?.fingerprint ?? "—")
                    Button("Reveal recovery phrase") {
                        revealConfirmed = false
                        showingReveal = true
                    }
                    Button("Restore from phrase") {
                        restorePhrase = ""
                        restoreError = false
                        showingRestore = true
                    }
                }
                Section {
                    Text("Your identity is backed up automatically through iCloud Keychain, end-to-end encrypted. Your writing stays on this device. There is no account. Whoever holds the phrase holds the identity.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .sheet(isPresented: $showingReveal) { revealSheet }
            .sheet(isPresented: $showingRestore) { restoreSheet }
        }
    }

    private var revealSheet: some View {
        NavigationStack {
            Group {
                if revealConfirmed, let identity = identityManager.identity {
                    VStack(alignment: .leading, spacing: 16) {
                        Text(identity.mnemonic.joined(separator: " "))
                            .font(.system(.title3, design: .monospaced))
                            .textSelection(.enabled)
                        Text("Write it down somewhere private. Anyone who sees it can become you in this app.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                        Spacer()
                    }
                    .padding()
                } else {
                    VStack(spacing: 16) {
                        Text("These 12 words are your identity and your only recovery. Reveal them only where nobody else can see your screen.")
                            .multilineTextAlignment(.center)
                        Button("Reveal") { revealConfirmed = true }
                            .buttonStyle(.borderedProminent)
                    }
                    .padding()
                }
            }
            .navigationTitle("Recovery phrase")
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    private var restoreSheet: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("Enter a 12-word recovery phrase. It replaces the identity here and in your iCloud Keychain backup; the writing stored on this device stays.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                TextEditor(text: $restorePhrase)
                    .font(.system(.body, design: .monospaced))
                    .frame(minHeight: 120)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.quaternary))
                if restoreError {
                    Text("That is not a valid 12-word phrase.")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
                Button("Restore identity") {
                    do {
                        try identityManager.restore(phrase: restorePhrase)
                        showingRestore = false
                    } catch {
                        restoreError = true
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(BIP39.normalize(restorePhrase).count != 12)
                Spacer()
            }
            .padding()
            .navigationTitle("Restore")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}
