import SwiftUI

@main
struct FixtureApplication: App {
    init() {
        // Every installed expression receives its own device-local identity.
        // Failure does not prevent first launch; the capability reports its
        // exact error when identity-dependent behavior is requested.
        _ = try? InstallationIdentity.shared.prepare()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

private struct ContentView: View {
    @AppStorage("tohseno.fixture.tap-count") private var tapCount = 0

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "circle.hexagongrid.fill")
                .font(.system(size: 52))
                .foregroundStyle(.tint)
            Text("A TOHSENO expression")
                .font(.title.bold())
            Text("Intent became a usable app.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            Button("Count: \(tapCount)") {
                tapCount += 1
            }
            .accessibilityIdentifier("tap-counter")
            if false { // TOHSENO_RESET_BUTTON
                Button("Reset") {
                    tapCount = 0
                }
                .accessibilityIdentifier("reset-counter")
            }
        }
        .padding()
    }
}
