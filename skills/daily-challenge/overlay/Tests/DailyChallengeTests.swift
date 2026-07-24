import XCTest
@testable import Shot

final class DailyChallengeTests: XCTestCase {
    func testReviewIsDeterministic() {
        let decisions = DailyChallengeEngine.sample.map(\.preferredChoice)
        let review = DailyChallengeEngine.review(decisions: decisions)
        XCTAssertEqual(review.score, 5)
        XCTAssertEqual(review.total, 5)
    }
}
