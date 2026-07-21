import XCTest
@testable import Writing

/// Invariants for local persistence: atomic commits, crash recovery, and
/// the reverse-chronological log. "Process death" is simulated by throwing
/// the store away and re-opening the same directory — exactly what a
/// killed app does on relaunch.
final class SessionStoreTests: XCTestCase {
    private var root: URL!

    override func setUp() {
        super.setUp()
        root = FileManager.default.temporaryDirectory
            .appendingPathComponent("session-store-tests-\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: root)
        super.tearDown()
    }

    func testDraftSurvivesProcessDeath() {
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        store.checkpoint(text: "written before the crash")
        // No finishDraft: the process dies here.

        let relaunched = SessionStore(root: root)
        let resumed = relaunched.beginOrResumeDraft()
        XCTAssertEqual(resumed.text, "written before the crash")
    }

    func testDraftKeepsItsStableIdentityAcrossRelaunch() {
        let store = SessionStore(root: root)
        let draft = store.beginOrResumeDraft()
        store.checkpoint(text: "x")

        let relaunched = SessionStore(root: root)
        XCTAssertEqual(relaunched.beginOrResumeDraft().id, draft.id)
    }

    func testFinishCommitsTextAndSidecarDurably() throws {
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        store.checkpoint(text: "kept words")
        let record = store.finishDraft()
        XCTAssertNotNil(record)
        XCTAssertEqual(record?.characterCount, 10)

        let relaunched = SessionStore(root: root)
        XCTAssertEqual(relaunched.sessions.map(\.id), [record!.id])
        XCTAssertEqual(relaunched.text(for: record!), "kept words")
        XCTAssertNil(relaunched.draft, "a finished draft must not resurrect")
    }

    func testEmptySessionsAreDiscardedNotKept() {
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        XCTAssertNil(store.finishDraft())
        XCTAssertEqual(store.sessions, [])
    }

    func testListIsReverseChronological() {
        let store = SessionStore(root: root)
        let base = Date(timeIntervalSince1970: 1_000_000)
        for offset in 0..<3 {
            _ = store.beginOrResumeDraft(now: base)
            store.checkpoint(text: "session \(offset)")
            _ = store.finishDraft(now: base.addingTimeInterval(Double(offset) * 60))
        }
        let relaunched = SessionStore(root: root)
        let ends = relaunched.sessions.map(\.endedAt)
        XCTAssertEqual(ends, ends.sorted(by: >))
        XCTAssertEqual(relaunched.sessions.count, 3)
    }

    func testUnicodeRoundTripsExactly() {
        let text = "día 🌊 — «writing» \u{1F600}\nsecond line"
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        store.checkpoint(text: text)
        let record = store.finishDraft()!

        let relaunched = SessionStore(root: root)
        XCTAssertEqual(relaunched.text(for: record), text)
    }

    func testTornSidecarIsIgnoredInsteadOfCrashing() throws {
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        store.checkpoint(text: "good session")
        _ = store.finishDraft()

        let sessions = root.appendingPathComponent("sessions", isDirectory: true)
        try Data("{torn".utf8).write(to: sessions.appendingPathComponent("\(UUID().uuidString).json"))

        let relaunched = SessionStore(root: root)
        XCTAssertEqual(relaunched.sessions.count, 1, "a torn file must not take the log down")
    }
}
