import SwiftUI

public struct TohsenoSettingsView: View {
    @Bindable private var model: TohsenoAppModel
    @State private var choosingExecutable = false

    public init(model: TohsenoAppModel) { self.model = model }

    public var body: some View {
        TabView {
            Form {
                LabeledContent("iPhone readiness", value: model.readiness?.ready == true ? "Ready" : "Needs attention")
                LabeledContent("Local factory", value: model.workspace == nil ? "Unavailable" : "Running")
                LabeledContent("App storage", value: "~/Desktop/Tohseno")
                Button("Check Again") { Task { await model.reload() } }
                Button("Restart Local Factory Safely") { Task { await model.restartService() } }
            }
            .padding(20)
            .tabItem { Label("Factory", systemImage: "gearshape.2") }

            Form {
                Section {
                    Text("Local and bring-your-own routes use the provider configuration you selected. Managed work sends only necessary context through TOHSENO and Bankr to the chosen upstream provider. Your generated source remains on this Mac.")
                        .foregroundStyle(.secondary)
                    ForEach(model.defaults?.harnesses ?? []) { option in
                        LabeledContent(option.label, value: option.installed ? (option.authentication == .notDetected ? "Needs sign-in" : "Available") : "Not installed")
                    }
                }
                Section("Custom executable harness") {
                    TextField("Identifier", text: $model.customHarness.id)
                    TextField("Display name", text: $model.customHarness.label)
                    HStack {
                        TextField("Executable", text: $model.customHarness.executable)
                            .disabled(true)
                        Button("Choose…") { choosingExecutable = true }
                    }
                    TextField("Models, separated by commas", text: $model.customHarness.models)
                    TextField("Fixed arguments, one per line", text: $model.customHarness.arguments, axis: .vertical)
                        .lineLimit(2...4)
                    Toggle("Prefer this route automatically", isOn: $model.customHarness.preferred)
                    Button("Save Custom Harness") { Task { await model.saveCustomHarness() } }
                        .disabled(model.isSubmitting)
                }
                Section("Local OpenAI-compatible endpoint") {
                    TextField("Identifier", text: $model.localEndpoint.id)
                    TextField("Display name", text: $model.localEndpoint.label)
                    TextField("Loopback base URL", text: $model.localEndpoint.baseURL)
                    TextField("Advertised models, separated by commas", text: $model.localEndpoint.models)
                    SecureField("Optional bearer credential", text: $model.localEndpoint.credential)
                    Picker("Privacy mode", selection: $model.localEndpoint.privacyMode) {
                        Text("Local").tag("local")
                        Text("Standard").tag("standard")
                        Text("Zero data retention").tag("zdr")
                        Text("Private").tag("private")
                    }
                    Toggle("I consent to send app source to this endpoint", isOn: $model.localEndpoint.consentToSendSource)
                    Toggle("Prefer this route automatically", isOn: $model.localEndpoint.preferred)
                    Button("Check and Save Endpoint") { Task { await model.saveLocalEndpoint() } }
                        .disabled(model.isSubmitting || !model.localEndpoint.consentToSendSource)
                }
            }
            .padding(20)
            .tabItem { Label("Intelligence", systemImage: "sparkles") }

            Form {
                Text("Managed balance is not required for local or bring-your-own intelligence.")
                    .foregroundStyle(.secondary)
                if let balance = model.managedBalance {
                    Section("Creation balance") {
                        LabeledContent("Spendable", value: model.signedCurrency(balance.spendableMicrousd))
                        LabeledContent("Paid", value: model.signedCurrency(balance.paidMicrousd))
                        LabeledContent("Promotional", value: model.signedCurrency(balance.promotionalMicrousd))
                        if balance.reservedMicrousd > 0 {
                            LabeledContent("Reserved for work", value: model.signedCurrency(balance.reservedMicrousd))
                        }
                        HStack {
                            Button("Add $10") { Task { await model.buyBalance(packID: "usd_10") } }
                            Button("Add $25") { Task { await model.buyBalance(packID: "usd_25") } }
                            Button("Add $50") { Task { await model.buyBalance(packID: "usd_50") } }
                        }
                        if model.managedStatus?.welcomeContactURL != nil {
                            Button("Message JP for Welcome Compute") { model.requestWelcomeCompute() }
                        }
                        Button("Refresh Balance") { Task { await model.refreshManaged() } }
                    }
                    Section("Recent transactions") {
                        if balance.transactions.isEmpty {
                            Text("No balance transactions yet.").foregroundStyle(.secondary)
                        }
                        ForEach(balance.transactions.prefix(20)) { entry in
                            LabeledContent(entry.description, value: model.signedCurrency(entry.amountMicrousd))
                                .help("\(entry.bucket) · \(entry.createdAt)")
                        }
                    }
                } else {
                    Text(model.managedMessage ?? "Managed balance is not configured for this release.")
                        .foregroundStyle(.secondary)
                    Button("Check Managed Service") { Task { await model.refreshManaged() } }
                }
            }
            .padding(20)
            .tabItem { Label("Balance", systemImage: "creditcard") }

            Form {
                Button("Open Legacy Browser Studio") { Task { await model.openLegacyStudio() } }
                Button("Export Support Report…") { model.exportSupportReport() }
                Section("Retired apps") {
                    if model.archivedApps.isEmpty {
                        Text("No retired apps.").foregroundStyle(.secondary)
                    }
                    ForEach(model.archivedApps) { app in
                        HStack {
                            VStack(alignment: .leading) {
                                Text(app.displayName)
                                Text("Source and accepted history remain on this Mac.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("Restore") { Task { await model.restore(app) } }
                                .disabled(model.isSubmitting)
                                .accessibilityIdentifier("archive.restore.\(app.id)")
                        }
                    }
                }
                Section("Privacy and updates") {
                    Link("Read Privacy Explanation", destination: URL(string: "https://tohseno.com/privacy")!)
                    Link("Check for Updates", destination: URL(string: "https://tohseno.com/download/macos")!)
                    Text("Updates are manual in this release. TOHSENO verifies the bundled factory before selecting it; no automatic update feed is active.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Text("Browser Studio is retained for support and diagnostics; it is not the normal product surface.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(20)
            .tabItem { Label("Diagnostics", systemImage: "stethoscope") }
        }
        .frame(width: 700, height: 520)
        .accessibilityIdentifier("settings.root")
        .fileImporter(isPresented: $choosingExecutable, allowedContentTypes: [.executable], allowsMultipleSelection: false) { result in
            do {
                guard let url = try result.get().first else { return }
                let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
                guard values.isRegularFile == true, values.isSymbolicLink != true,
                      FileManager.default.isExecutableFile(atPath: url.path) else {
                    throw FactoryClientError.invalidConfiguration("Choose a regular, non-symlink executable.")
                }
                model.customHarness.executable = url.path
            } catch {
                model.report(error)
            }
        }
    }
}
