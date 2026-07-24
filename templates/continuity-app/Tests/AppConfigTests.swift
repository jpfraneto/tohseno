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

    func testDevelopmentAndProductionEndpointsStaySeparate() throws {
        let fixtureURL = try XCTUnwrap(
            Bundle(for: Self.self).url(
                forResource: "production-endpoints",
                withExtension: "json"
            )
        )
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL))
                as? [String: [String]]
        )
        for value in object["acceptedProduction"] ?? [] {
            XCTAssertNotNil(
                AppConfig.validatedAPIBaseURL(value, production: true),
                "expected production endpoint to pass: \(value)"
            )
        }
        for value in object["rejectedProduction"] ?? [] {
            XCTAssertNil(
                AppConfig.validatedAPIBaseURL(value, production: true),
                "expected production endpoint to fail: \(value)"
            )
        }
        for value in object["acceptedDevelopment"] ?? [] {
            XCTAssertNotNil(
                AppConfig.validatedAPIBaseURL(value, production: false),
                "expected development endpoint to pass: \(value)"
            )
        }
        for value in object["rejectedDevelopment"] ?? [] {
            XCTAssertNil(
                AppConfig.validatedAPIBaseURL(value, production: false),
                "expected development endpoint to fail: \(value)"
            )
        }
    }

    func testShareCardSnippetStaysReadable() {
        XCTAssertEqual(ShareCard.snippet(of: "  short  "), "short")
        let long = String(repeating: "words and more words. ", count: 40)
        let snippet = ShareCard.snippet(of: long)
        XCTAssertTrue(snippet.hasSuffix("…"))
        XCTAssertLessThanOrEqual(snippet.count, 282)
    }
}
