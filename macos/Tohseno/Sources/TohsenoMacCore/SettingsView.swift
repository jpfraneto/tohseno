import SwiftUI
import AppKit
import CoreImage.CIFilterBuiltins

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
                Section("Terminal") {
                    LabeledContent("tohseno command", value: model.cliIntegration?.enabled == true ? "Ready in new windows" : "Not activated")
                    if model.cliIntegration?.enabled != true {
                        Button(model.isEnablingCLI ? "Activating…" : "Activate CLI") {
                            Task { await model.enableCLIIntegration() }
                        }
                        .disabled(model.isEnablingCLI || model.cliIntegration?.installed != true)
                    }
                    Text(model.cliMessage ?? "Activation adds the verified ~/.tohseno/bin command to your shell profile without replacing unrelated settings.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Button("Check Again") { Task { await model.reload() } }
                Button("Restart Local Factory Safely") { Task { await model.restartService() } }
                Section("Companion devices") {
                    if model.pairedCompanionDevices.isEmpty {
                        Text("No iPhone Companion is paired yet.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(model.pairedCompanionDevices) { device in
                        PairedCompanionDeviceRow(model: model, device: device)
                    }
                    Button("Pair Another iPhone") {
                        Task { await model.beginCompanionPairing() }
                    }
                    if let session = model.companionPairingSession {
                        CompanionPairingCard(session: session)
                    }
                }
            }
            .padding(20)
            .tabItem { Label("Factory", systemImage: "gearshape.2") }

            Form {
                Section("Intelligence") {
                    Text("Tohseno uses intelligence already available on this Mac. Provider sign-in stays with the provider, and local work does not require Tohseno credits.")
                        .foregroundStyle(.secondary)
                    ForEach((model.defaults?.harnesses ?? []).filter {
                        $0.id != "tohseno-managed" && $0.installed
                    }) { option in
                        Label {
                            LabeledContent(
                                option.label,
                                value: option.authentication == .authenticated ? "Available" : "Needs sign-in"
                            )
                        } icon: {
                            Image(systemName: option.authentication == .authenticated
                                ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(option.authentication == .authenticated
                                    ? TohsenoTheme.amber : .secondary)
                        }
                        .accessibilityIdentifier("intelligence.provider.\(option.id)")
                    }
                    if !(model.defaults?.harnesses ?? []).contains(where: {
                        $0.id != "tohseno-managed" && $0.installed
                    }) {
                        Label("No supported local intelligence detected", systemImage: "circle")
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("intelligence.unavailable")
                    }
                    Label {
                        LabeledContent("Tohseno Intelligence", value: "Coming soon")
                    } icon: {
                        Image(systemName: "circle").foregroundStyle(.secondary)
                    }
                    .accessibilityIdentifier("intelligence.tohseno-coming-soon")
                }

                DisclosureGroup("Advanced", isExpanded: $model.advancedExpanded) {
                    Text("Custom executable")
                        .font(.headline)
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

                    Divider()
                    Text("Local OpenAI-compatible endpoint")
                        .font(.headline)
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
                    Text("Updates remain manual. Tohseno checks the fail-closed release metadata and opens the verified DMG route; it never downloads or replaces the app automatically.")
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

private struct PairedCompanionDeviceRow: View {
    let model: TohsenoAppModel
    let device: PairedCompanionDevice
    @State private var name: String

    init(model: TohsenoAppModel, device: PairedCompanionDevice) {
        self.model = model
        self.device = device
        _name = State(initialValue: device.displayName)
    }

    var body: some View {
        HStack {
            Image(systemName: device.revoked ? "iphone.slash" : "iphone.gen3")
            TextField("iPhone name", text: $name)
                .disabled(device.revoked)
            Text(device.revoked ? "Revoked" : "Paired")
                .font(.caption)
                .foregroundStyle(.secondary)
            if !device.revoked, name != device.displayName {
                Button("Rename") {
                    Task { await model.renameCompanionDevice(device, to: name) }
                }
            }
            if !device.revoked {
                Button("Revoke", role: .destructive) {
                    Task { await model.revokeCompanionDevice(device) }
                }
            }
        }
        .accessibilityIdentifier("settings.companion.\(device.id)")
    }
}

struct CompanionPairingCard: View {
    let session: CompanionPairingSession

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if session.state == "waiting", let image = pairingQRCode(session.pairingURI) {
                HStack(alignment: .top, spacing: 16) {
                    Image(nsImage: image)
                        .interpolation(.none)
                        .resizable()
                        .frame(width: 150, height: 150)
                        .accessibilityLabel("Companion pairing QR code")
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Open Tohseno Companion and scan this code.")
                        Text("This one-use invitation expires at \(session.expiresAt).")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Button("Copy Pairing Link") {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(session.pairingURI, forType: .string)
                        }
                    }
                }
            } else if session.state == "paired" {
                Label("\(session.deviceName ?? "iPhone") exchanged an authenticated snapshot with this Mac.", systemImage: "checkmark.shield.fill")
            } else {
                Text("Pairing \(session.state). Create a new one-use invitation to retry.")
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 6)
    }
}

private func pairingQRCode(_ value: String) -> NSImage? {
    let filter = CIFilter.qrCodeGenerator()
    filter.message = Data(value.utf8)
    filter.correctionLevel = "M"
    guard let output = filter.outputImage?.transformed(
        by: CGAffineTransform(scaleX: 8, y: 8)
    ) else { return nil }
    let representation = NSCIImageRep(ciImage: output)
    let image = NSImage(size: representation.size)
    image.addRepresentation(representation)
    return image
}
