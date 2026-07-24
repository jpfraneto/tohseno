import Foundation

struct LocalProgressRecord: Codable, Equatable {
    var completedRuns = 0
    var bestScore = 0
    var totalCorrect = 0
    var totalDecisions = 0
}

@MainActor
final class LocalProgressStore: ObservableObject {
    @Published private(set) var record: LocalProgressRecord

    private let defaults: UserDefaults
    private let key: String

    init(
        defaults: UserDefaults = .standard,
        key: String = "tohseno.local-progress.v1"
    ) {
        self.defaults = defaults
        self.key = key
        if let data = defaults.data(forKey: key),
           let decoded = try? JSONDecoder().decode(LocalProgressRecord.self, from: data) {
            record = decoded
        } else {
            record = LocalProgressRecord()
        }
    }

    func record(score: Int, total: Int) {
        record.completedRuns += 1
        record.bestScore = max(record.bestScore, score)
        record.totalCorrect += score
        record.totalDecisions += total
        if let data = try? JSONEncoder().encode(record) {
            defaults.set(data, forKey: key)
        }
    }
}
