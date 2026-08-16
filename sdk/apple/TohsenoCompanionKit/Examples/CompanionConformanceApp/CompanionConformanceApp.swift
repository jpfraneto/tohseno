import SwiftUI
import TohsenoCompanionKit

@main
struct CompanionConformanceApp: App {
    @State private var model: ConformanceModel?

    init() {
        _model = State(initialValue: try? ConformanceModel())
    }

    var body: some Scene {
        WindowGroup {
            if let model {
                ConformanceView(model: model)
            } else {
                ContentUnavailableView(
                    "Conformance storage unavailable",
                    systemImage: "exclamationmark.shield"
                )
            }
        }
    }
}
