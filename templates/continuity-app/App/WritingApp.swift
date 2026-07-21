import SwiftUI

/// The app opens directly to the writing surface. No onboarding, no splash
/// ceremony, no permission requests. Identity is created silently on first
/// launch; the person just writes.
@main
struct WritingApp: App {
    @StateObject private var store = SessionStore()
    @StateObject private var identityManager = IdentityManager(store: KeychainSecretStore())

    var body: some Scene {
        WindowGroup {
            WritingView()
                .environmentObject(store)
                .environmentObject(identityManager)
                .task { identityManager.loadOrCreate() }
        }
    }

    /// The paywall seam. Swap `NoopPaywall` for a real provider when
    /// `AppConfig.paywallEnabled` is flipped — see Modules/Paywall.swift.
    static func makePaywall() -> PaywallProviding {
        NoopPaywall()
    }
}
