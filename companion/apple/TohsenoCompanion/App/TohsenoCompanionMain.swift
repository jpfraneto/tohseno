import SwiftUI
import TohsenoCompanionApp
import UIKit

/// The iPhone app shell.
///
/// Everything the person sees lives in `TohsenoCompanionApp` so it can be
/// tested without a Simulator. This file only creates the SDK client and hands
/// it to the product surface.
@main
struct TohsenoCompanion: App {
    @State private var model: CompanionModel?
    @Environment(\.scenePhase) private var scenePhase

    init() {
        _model = State(initialValue: Self.makeModel())
    }

    var body: some Scene {
        WindowGroup {
            if let model {
                CompanionRootView(model: model)
            } else {
                ContentUnavailableView(
                    "TOHSENO can’t use this iPhone’s private storage",
                    systemImage: "exclamationmark.shield"
                )
            }
        }
        .onChange(of: scenePhase) { _, phase in
            // Reconciling on foreground is what makes a request sent while the
            // Mac was away deliver itself without another tap.
            guard phase == .active, let model else { return }
            Task { await model.refresh() }
        }
    }

    @MainActor
    private static func makeModel() -> CompanionModel? {
#if DEBUG
        let configured = Bundle.main.object(
            forInfoDictionaryKey: "TOHSENOVerificationRelayOrigin"
        ) as? String
        let relayOrigin = configured.flatMap(URL.init(string:))
#else
        let relayOrigin: URL? = nil
#endif
        guard let client = try? CompanionClientFactory.make(relayOrigin: relayOrigin) else {
            return nil
        }
        return CompanionModel(backend: client, deviceName: UIDevice.current.name)
    }
}
