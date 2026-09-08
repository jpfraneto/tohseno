import Foundation
import Testing
@testable import TohsenoCompanionKit

@Test func executionCarriesBoundedLiveActivityWithoutChangingLegacyMessages() throws {
    let base = #"{"execution_id":"execution_1","shot_id":"shot_1","state":"materializing","updated_at":"2026-09-08T15:00:00Z"}"#
    let legacy = try JSONDecoder().decode(ExecutionSummary.self, from: Data(base.utf8))
    try legacy.validate()
    #expect(legacy.activity == nil)
    var object = try #require(JSONSerialization.jsonObject(with: Data(base.utf8)) as? [String: Any])
    let entry: [String: Any] = ["sequence": 1, "timestamp": "2026-09-08T15:00:00Z", "message": "Created the recording screen."]
    object["activity"] = ["entries": [entry], "files": ["Recorder.swift"], "file_count": 1, "total_tokens": 250]
    let live = try JSONDecoder().decode(ExecutionSummary.self, from: JSONSerialization.data(withJSONObject: object))
    try live.validate()
    #expect(live.activity?.entries.first?.message == "Created the recording screen.")
    #expect(live.activity?.files == ["Recorder.swift"])
    #expect(live.activity?.totalTokens == 250)
    object["activity"] = ["entries": Array(repeating: entry, count: 61), "files": [], "file_count": 0]
    let oversized = try JSONDecoder().decode(ExecutionSummary.self, from: JSONSerialization.data(withJSONObject: object))
    #expect(throws: (any Error).self) { try oversized.validate() }
}
