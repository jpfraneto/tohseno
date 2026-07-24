import SwiftUI

/// The neutral kernel surface. An app template may deliberately replace this
/// file while preserving the app entry point and design-token seam.
struct AppRootView: View {
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
                Text("Ready for your app")
                    .font(DesignTokens.Typography.title)
                Text("The native shell is running. Shape the first useful experience from SHOT.md.")
                    .font(DesignTokens.Typography.body)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(DesignTokens.Spacing.large)
            .navigationTitle("New App")
        }
    }
}

#Preview {
    AppRootView()
}
