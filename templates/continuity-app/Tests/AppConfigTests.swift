import XCTest
@testable import Writing

/// The base app must run with zero keys and its default flags: paywall and
/// notifications off, share card on, SessionLink permanently reserved.
final class AppConfigTests: XCTestCase {
    func testDefaultFlagsMatchTheManifest() {
        XCTAssertFalse(AppConfig.paywallEnabled)
        XCTAssertTrue(AppConfig.shareCardEnabled)
        XCTAssertFalse(AppConfig.notificationsEnabled)
        XCTAssertFalse(AppConfig.sessionLinkEnabled, "SessionLink is reserved; enabling it is unsupported")
        XCTAssertFalse(AppConfig.tokenMintEnabled, "TokenMint is reserved; enabling it is unsupported")
    }

    func testRunsWithNoKeys() {
        // The slot exists; the base app never requires it to be filled.
        XCTAssertEqual(AppConfig.revenueCatPublicKey, "")
        XCTAssertEqual(AppConfig.developmentSecret, "", "Simulator builds must never expose DEV_SECRET")
        XCTAssertTrue(NoopPaywall().isEntitled)
    }

    func testDevelopmentAndProductionEndpointsStaySeparate() {
        XCTAssertNotNil(AppConfig.validatedAPIBaseURL("http://127.0.0.1:43123", production: false))
        XCTAssertNotNil(AppConfig.validatedAPIBaseURL("https://random-name.trycloudflare.com", production: false))
        XCTAssertNil(AppConfig.validatedAPIBaseURL("http://api.example.com", production: false))

        XCTAssertNotNil(AppConfig.validatedAPIBaseURL("https://api.example.com", production: true))
        XCTAssertNil(AppConfig.validatedAPIBaseURL("http://localhost:43123", production: true))
        XCTAssertNil(AppConfig.validatedAPIBaseURL("https://random-name.trycloudflare.com", production: true))
        XCTAssertNil(AppConfig.validatedAPIBaseURL("https://api.example.com/private", production: true))
    }

    func testShareCardSnippetStaysReadable() {
        XCTAssertEqual(ShareCard.snippet(of: "  short  "), "short")
        let long = String(repeating: "words and more words. ", count: 40)
        let snippet = ShareCard.snippet(of: long)
        XCTAssertTrue(snippet.hasSuffix("…"))
        XCTAssertLessThanOrEqual(snippet.count, 282)
    }
}
