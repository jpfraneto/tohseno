import Foundation

/// The single configuration seam of the app: feature flags plus key slots.
///
/// Flipping a flag here is the only integration step for a module. Every
/// module's code is present and compiles cleanly whether its flag is on or
/// off; the flag decides whether it appears at runtime.
///
/// Key slots hold public identifiers only. Secrets never live in source,
/// and the app must build and run with every slot empty.
enum AppConfig {
    // MARK: Feature flags

    /// Paywall via RevenueCat. OFF by default. To enable: set this to true,
    /// add the RevenueCat SDK package, implement `RevenueCatPaywall` behind
    /// `PaywallProviding` (see Modules/Paywall.swift), and put your public
    /// API key in the `REVENUECAT_PUBLIC_KEY` slot of Config/App.xcconfig
    /// (or run `bun run setup`).
    static let paywallEnabled = false

    /// Local share card rendered to the system share sheet. ON by default.
    /// Pure local rendering, no network.
    static let shareCardEnabled = true

    /// Local notifications. OFF by default. When enabled the app asks for
    /// permission from Settings (never at launch) and schedules a daily
    /// reminder.
    static let notificationsEnabled = false

    /// RESERVED — not implemented. SessionLink is the named future primitive
    /// for QR browser pairing: scan a QR on a web page, confirm in the app,
    /// and a signed session connects the browser to the app identity.
    /// The flag exists so the pattern has a stable name; flipping it does
    /// nothing in this release. See Modules/SessionLink.swift.
    static let sessionLinkEnabled = false

    // MARK: Key slots

    /// RevenueCat *public* API key, injected from the xcconfig via Info.plist.
    /// Empty by default; the app never requires it to build or run.
    static var revenueCatPublicKey: String {
        (Bundle.main.object(forInfoDictionaryKey: "RevenueCatPublicKey") as? String) ?? ""
    }
}
