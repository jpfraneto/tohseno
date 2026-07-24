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

    func testOwnerExportReturnsExactTextAndSidecarBytes() throws {
        let text = "literal bytes: día 🌊\n"
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft(now: Date(timeIntervalSince1970: 1_000))
        store.checkpoint(text: text)
        let record = try XCTUnwrap(store.finishDraft(now: Date(timeIntervalSince1970: 2_000)))

        let exports = store.exportURLs(for: record)
        XCTAssertEqual(exports.map(\.pathExtension), ["txt", "json"])
        XCTAssertEqual(try Data(contentsOf: exports[0]), Data(text.utf8))

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        XCTAssertEqual(
            try decoder.decode(SessionRecord.self, from: Data(contentsOf: exports[1])),
            record
        )
    }

    func testOwnerExportFailsClosedAfterFileSubstitution() throws {
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        store.checkpoint(text: "canonical")
        let record = try XCTUnwrap(store.finishDraft())
        let text = root
            .appendingPathComponent("sessions", isDirectory: true)
            .appendingPathComponent("\(record.id.uuidString).txt")
        try FileManager.default.removeItem(at: text)
        try FileManager.default.createSymbolicLink(
            at: text,
            withDestinationURL: root.appendingPathComponent("draft.json")
        )

        XCTAssertTrue(store.exportURLs(for: record).isEmpty)
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

    func testSidecarFailureKeepsTheRecoveryDraft() throws {
        let store = SessionStore(root: root)
        let draft = store.beginOrResumeDraft()
        store.checkpoint(text: "must survive a failed commit")
        let sidecar = root
            .appendingPathComponent("sessions", isDirectory: true)
            .appendingPathComponent("\(draft.id.uuidString).json")
        try FileManager.default.createDirectory(at: sidecar, withIntermediateDirectories: false)

        XCTAssertNil(store.finishDraft())
        XCTAssertTrue(store.persistenceError)
        XCTAssertEqual(store.draft?.text, "must survive a failed commit")

        let relaunched = SessionStore(root: root)
        XCTAssertEqual(
            relaunched.beginOrResumeDraft().text,
            "must survive a failed commit",
            "a failed finalization must leave the durable checkpoint resumable"
        )
        XCTAssertTrue(relaunched.sessions.isEmpty)
    }

    func testTextFailureKeepsTheRecoveryDraft() throws {
        let store = SessionStore(root: root)
        let draft = store.beginOrResumeDraft()
        store.checkpoint(text: "still here")
        let text = root
            .appendingPathComponent("sessions", isDirectory: true)
            .appendingPathComponent("\(draft.id.uuidString).txt")
        try FileManager.default.createDirectory(at: text, withIntermediateDirectories: false)

        XCTAssertNil(store.finishDraft())
        XCTAssertEqual(store.draft?.text, "still here")
        XCTAssertEqual(SessionStore(root: root).beginOrResumeDraft().text, "still here")
    }

    func testCommittedSessionWinsOverAStaleDraftCheckpoint() throws {
        let store = SessionStore(root: root)
        let draft = store.beginOrResumeDraft()
        store.checkpoint(text: "committed once")
        XCTAssertNotNil(store.finishDraft())

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        try encoder.encode(draft).write(
            to: root.appendingPathComponent("draft.json"),
            options: .atomic
        )

        let relaunched = SessionStore(root: root)
        XCTAssertEqual(relaunched.sessions.map(\.id), [draft.id])
        XCTAssertNil(relaunched.draft, "a committed record must not resurrect as a duplicate")
        XCTAssertFalse(
            FileManager.default.fileExists(
                atPath: root.appendingPathComponent("draft.json").path
            )
        )
    }

    func testMismatchedSidecarIsNotAcceptedAsACommittedSession() throws {
        let store = SessionStore(root: root)
        _ = store.beginOrResumeDraft()
        store.checkpoint(text: "canonical")
        let record = store.finishDraft()!
        let sidecar = root
            .appendingPathComponent("sessions", isDirectory: true)
            .appendingPathComponent("\(record.id.uuidString).json")
        let source = try String(contentsOf: sidecar, encoding: .utf8)
        try source.replacingOccurrences(
            of: "\"characterCount\":9",
            with: "\"characterCount\":999"
        ).write(to: sidecar, atomically: true, encoding: .utf8)

        XCTAssertTrue(SessionStore(root: root).sessions.isEmpty)
    }
}
