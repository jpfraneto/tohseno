import XCTest
@testable import Shot

@MainActor
final class LocalProgressTests: XCTestCase {
    func testProgressSurvivesStoreRecreation() {
        let suite = "LocalProgressTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }

        let first = LocalProgressStore(defaults: defaults, key: "record")
        first.record(score: 4, total: 5)

        let restored = LocalProgressStore(defaults: defaults, key: "record")
        XCTAssertEqual(restored.record.completedRuns, 1)
        XCTAssertEqual(restored.record.bestScore, 4)
    }
}
