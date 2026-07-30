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
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "circle.hexagongrid.fill")
                .font(.system(size: 52))
                .foregroundStyle(.tint)
            Text("A TOHSENO expression")
                .font(.title.bold())
            Text("This fixture passes the real Apple materialization gates.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
        }
        .padding()
    }
}
