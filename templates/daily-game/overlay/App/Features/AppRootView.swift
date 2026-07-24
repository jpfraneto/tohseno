import SwiftUI

struct AppRootView: View {
    var body: some View {
        NavigationStack {
            DailyChallengeView()
        }
    }
}

#Preview {
    AppRootView()
}
