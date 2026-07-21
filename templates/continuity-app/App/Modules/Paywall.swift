import Foundation

/// Paywall module — flag-gated, OFF by default (`AppConfig.paywallEnabled`).
///
/// The seam is this protocol. The base app ships `NoopPaywall`, which
/// compiles and runs with no SDK and no key, so the paywall's presence
/// costs nothing until a builder wants it.
///
/// To enable RevenueCat:
///  1. Set `AppConfig.paywallEnabled = true`.
///  2. Add the RevenueCat SPM package (https://github.com/RevenueCat/purchases-ios).
///  3. Implement `RevenueCatPaywall: PaywallProviding`, configuring the SDK
///     with `AppConfig.revenueCatPublicKey` (filled via `bun run setup` or
///     the `REVENUECAT_PUBLIC_KEY` slot in Config/App.xcconfig).
///  4. Swap the provider in `WritingApp.makePaywall()`.
///
/// A paywall is a tool, not a sin — but it must never gate what the person
/// already wrote: sessions on disk stay readable and exportable regardless
/// of entitlement state.
protocol PaywallProviding {
    var isEntitled: Bool { get }
    func presentPaywallIfNeeded() async -> Bool
}

/// The default provider: everything is available, nothing is presented.
struct NoopPaywall: PaywallProviding {
    var isEntitled: Bool { true }

    func presentPaywallIfNeeded() async -> Bool { true }
}
