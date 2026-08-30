import SwiftUI
import AppKit
import TohsenoMacCore

@main
struct TohsenoMacApp: App {
    @State private var model: TohsenoAppModel

    init() {
        #if DEBUG
        if ProcessInfo.processInfo.environment["TOHSENO_UI_FIXTURE"] == "1" {
            let fixture = TohsenoAppModel(
                client: UIFixtureFactoryClient(),
                preferences: UserDefaults(suiteName: "tohseno-ui-fixture") ?? .standard
            )
            fixture.route = .app(UIFixtureFactoryClient.appID)
            _model = State(initialValue: fixture)
            return
        }
        #endif
        _model = State(initialValue: TohsenoAppModel(client: LoopbackFactoryClient()))
    }

    var body: some Scene {
        WindowGroup("Tohseno", id: "factory") {
            TohsenoRootView(model: model)
                .frame(minWidth: 860, minHeight: 620)
        }
        .defaultSize(width: 1120, height: 760)
        .windowResizability(.contentMinSize)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Create App") { model.route = .create }
                    .keyboardShortcut("n", modifiers: .command)
            }
            CommandGroup(after: .help) {
                Link("Tohseno Help", destination: URL(string: "https://tohseno.com/docs")!)
                Link("Check for Updates…", destination: URL(string: "https://tohseno.com/download/macos")!)
            }
        }

        MenuBarExtra {
            TohsenoMenuBarView(model: model)
        } label: {
            Image(nsImage: menuBarIcon())
                .accessibilityLabel("Tohseno")
        }

        Settings {
            TohsenoSettingsView(model: model)
        }
    }
}

private struct TohsenoMenuBarView: View {
    @Bindable var model: TohsenoAppModel
    @Environment(\.openWindow) private var openWindow

    private var status: (String, String) {
        if model.isLoading { return ("Opening your connected projects…", "circle.dotted") }
        if model.errorMessage != nil { return ("Needs attention", "exclamationmark.triangle.fill") }
        if model.readiness?.ready != true { return ("Finish setup", "iphone.badge.exclamationmark") }
        return ("Factory ready", "checkmark.circle.fill")
    }

    var body: some View {
        Label(status.0, systemImage: status.1)
            .disabled(true)
        if let device = model.connectedDeviceDescription {
            Label(device, systemImage: "iphone.gen3")
                .disabled(true)
        }
        Divider()
        Button("Open Tohseno") {
            openWindow(id: "factory")
            NSApplication.shared.activate(ignoringOtherApps: true)
        }
        SettingsLink { Text("Settings…") }
        Divider()
        Button("Quit Tohseno") { NSApplication.shared.terminate(nil) }
            .keyboardShortcut("q")
    }
}

@MainActor
private func menuBarIcon() -> NSImage {
    let bundled = Bundle.main.url(forResource: "TohsenoLogo", withExtension: "svg")
        .flatMap(NSImage.init(contentsOf:))
    let image = bundled
        ?? NSImage(systemSymbolName: "circle.hexagongrid.fill", accessibilityDescription: "Tohseno")
        ?? NSImage(size: NSSize(width: 18, height: 18))
    image.size = NSSize(width: 18, height: 18)
    image.isTemplate = true
    return image
}
