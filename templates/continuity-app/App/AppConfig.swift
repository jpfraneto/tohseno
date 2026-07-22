import Foundation

/// The single configuration seam of the app: feature flags plus key slots.
///
/// Flipping a flag here is the only integration step for a module. Every
/// module's code is present and compiles cleanly whether its flag is on or
/// off; the flag decides whether it appears at runtime.
///
/// Key slots hold public identifiers only. The isolated development-secret
/// exception below reads only gitignored local configuration and is forced
/// empty outside an owner-controlled Debug device. Secrets never live in
/// source, and the app must build and run with every slot empty.
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

    /// RESERVED — not implemented. TokenMint is the named service pattern for
    /// exchanging a non-secret client request for a short-lived provider
    /// credential without receiving user content. The flag exists so agents
    /// can cite the seam; flipping it does nothing in this release. See
    /// Modules/TokenMint.swift.
    static let tokenMintEnabled = false

    // MARK: Key slots

    /// RevenueCat *public* API key, injected from the xcconfig via Info.plist.
    /// Empty by default; the app never requires it to build or run.
    static var revenueCatPublicKey: String {
        (Bundle.main.object(forInfoDictionaryKey: "RevenueCatPublicKey") as? String) ?? ""
    }

    // MARK: Prototype-only development exception

    /// Prototype exception for a provider secret on an owner-controlled
    /// development device. `App.xcconfig` exposes this Info.plist value only
    /// to Debug builds for iphoneos; simulator and Release builds receive an
    /// empty string. Replace direct provider credentials with short-lived
    /// credentials from a TokenMint service before distributing the app.
    #if DEBUG
    static var developmentSecret: String {
        (Bundle.main.object(forInfoDictionaryKey: "DevelopmentSecret") as? String) ?? ""
    }
    #else
    static let developmentSecret = ""
    #endif
}
