import SwiftUI
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
        WindowGroup("TOHSENO", id: "factory") {
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
                Link("TOHSENO Help", destination: URL(string: "https://tohseno.com/docs")!)
                Link("Check for Updates…", destination: URL(string: "https://tohseno.com/download/macos")!)
            }
        }

        Settings {
            TohsenoSettingsView(model: model)
        }
    }
}
