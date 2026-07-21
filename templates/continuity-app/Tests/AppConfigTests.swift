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
    }

    func testRunsWithNoKeys() {
        // The slot exists; the base app never requires it to be filled.
        XCTAssertEqual(AppConfig.revenueCatPublicKey, "")
        XCTAssertTrue(NoopPaywall().isEntitled)
    }

    func testShareCardSnippetStaysReadable() {
        XCTAssertEqual(ShareCard.snippet(of: "  short  "), "short")
        let long = String(repeating: "words and more words. ", count: 40)
        let snippet = ShareCard.snippet(of: long)
        XCTAssertTrue(snippet.hasSuffix("…"))
        XCTAssertLessThanOrEqual(snippet.count, 282)
    }
}
