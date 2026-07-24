import XCTest
@testable import Shot

final class RankProgressionTests: XCTestCase {
    func testRankAdvancesEveryThreeRuns() {
        var record = LocalProgressRecord()
        XCTAssertEqual(RankProgression.rank(for: record).level, 1)
        record.completedRuns = 3
        XCTAssertEqual(RankProgression.rank(for: record).level, 2)
    }
}
