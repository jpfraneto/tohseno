import SwiftUI

@main
struct HelloWorldApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "circle.hexagongrid.fill")
                .font(.system(size: 52))
                .foregroundStyle(.tint)
            Text("Hello from TOHSENO")
                .font(.title.bold())
        }
        .padding()
    }
}

