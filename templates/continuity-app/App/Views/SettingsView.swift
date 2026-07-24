import SwiftUI

/// Settings holds exactly two identity actions: reveal the recovery phrase
/// (behind a warning) and restore from a phrase (replaces local identity).
struct SettingsView: View {
    @EnvironmentObject private var identityManager: IdentityManager
    @Environment(\.scenePhase) private var scenePhase
    @StateObject private var recoveryGate = RecoveryPhraseGate()
    @State private var showingReveal = false
    @State private var showingRestore = false
    @State private var restorePhrase = ""
    @State private var restoreError = false
    @State private var backendHealth: BackendHealthState = .checking

    var body: some View {
        NavigationStack {
            List {
                Section("Identity") {
                    LabeledContent("User ID", value: identityManager.identity?.fingerprint ?? "—")
                    if identityManager.accessState == .unavailable {
                        Text("The saved identity is unavailable. No replacement was created.")
                            .font(.footnote)
                            .foregroundStyle(.red)
                        Button("Retry identity access") {
                            identityManager.loadOrCreate()
                        }
                    }
                    Button("Reveal recovery phrase") {
                        recoveryGate.hide()
                        showingReveal = true
                    }
                    .disabled(identityManager.identity == nil)
                    Button("Restore from phrase") {
                        recoveryGate.hide()
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
                Section("App API") {
                    LabeledContent("Environment", value: AppConfig.apiEnvironment.capitalized)
                    LabeledContent("Endpoint", value: AppConfig.apiBaseURL?.host() ?? "Not configured")
                    LabeledContent("Health", value: backendHealth.label)
                    if backendHealth == .unavailable {
                        Text("The configured backend is unavailable. Your writing remains local and safe.")
                            .font(.footnote)
                            .foregroundStyle(.red)
                    }
                    Button("Check again") {
                        Task { await refreshBackendHealth() }
                    }
                    .disabled(AppConfig.apiBaseURL == nil)
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .sheet(isPresented: $showingReveal) { revealSheet }
            .sheet(isPresented: $showingRestore) { restoreSheet }
            .task { await refreshBackendHealth() }
            .onChange(of: scenePhase) {
                if scenePhase != .active {
                    recoveryGate.hide()
                    showingReveal = false
                    restorePhrase = ""
                    showingRestore = false
                }
            }
        }
    }

    @MainActor
    private func refreshBackendHealth() async {
        backendHealth = AppConfig.apiBaseURL == nil ? .notConfigured : .checking
        backendHealth = await BackendHealth.check(baseURL: AppConfig.apiBaseURL)
    }

    private var revealSheet: some View {
        NavigationStack {
            Group {
                if recoveryGate.isRevealed, let identity = identityManager.identity {
                    VStack(alignment: .leading, spacing: 16) {
                        Text(identity.mnemonic.joined(separator: " "))
                            .font(.system(.title3, design: .monospaced))
                            .textSelection(.enabled)
                            .privacySensitive()
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
                        Button(recoveryGate.isAuthorizing ? "Authenticating…" : "Authenticate and reveal") {
                            Task { await recoveryGate.requestReveal() }
                        }
                            .buttonStyle(.borderedProminent)
                            .disabled(recoveryGate.isAuthorizing)
                        if recoveryGate.authorizationFailed {
                            Text("Authentication did not complete. The phrase is still hidden.")
                                .font(.footnote)
                                .foregroundStyle(.red)
                        }
                    }
                    .padding()
                }
            }
            .navigationTitle("Recovery phrase")
            .navigationBarTitleDisplayMode(.inline)
            .onDisappear { recoveryGate.hide() }
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
                    .privacySensitive()
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.quaternary))
                if restoreError {
                    Text("That is not a valid 12-word phrase.")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
                Button("Restore identity") {
                    Task {
                        recoveryGate.hide()
                        await recoveryGate.requestReveal()
                        guard recoveryGate.isRevealed else { return }
                        do {
                            try identityManager.restore(phrase: restorePhrase)
                            restorePhrase = ""
                            recoveryGate.hide()
                            showingRestore = false
                        } catch {
                            recoveryGate.hide()
                            restoreError = true
                        }
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(
                    BIP39.normalize(restorePhrase).count != 12 ||
                    recoveryGate.isAuthorizing
                )
                if recoveryGate.isAuthorizing {
                    Text("Authenticating before replacing this identity…")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                } else if recoveryGate.authorizationFailed {
                    Text("Authentication did not complete. The identity was not changed.")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
                Spacer()
            }
            .padding()
            .navigationTitle("Restore")
            .navigationBarTitleDisplayMode(.inline)
            .onDisappear {
                recoveryGate.hide()
                restorePhrase = ""
                restoreError = false
            }
        }
    }
}
