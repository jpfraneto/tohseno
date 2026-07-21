import Foundation

/// Local, file-based session storage.
///
/// Layout under the store's root directory (Documents by default):
///
///     sessions/<uuid>.txt    the session text, exactly as written
///     sessions/<uuid>.json   sidecar: startedAt, endedAt, characterCount
///     draft.json             the in-progress session, if any
///
/// Every write is atomic (`.atomic` writes to a temporary file and renames),
/// so a killed process never leaves a torn file and never loses committed
/// text: whatever the last completed write held is what comes back.
final class SessionStore: ObservableObject {
    @Published private(set) var sessions: [SessionRecord] = []
    @Published var draft: DraftRecord?

    private let root: URL
    private var sessionsDirectory: URL { root.appendingPathComponent("sessions", isDirectory: true) }
    private var draftURL: URL { root.appendingPathComponent("draft.json") }

    private let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        return encoder
    }()
    private let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }()

    /// The production store rooted in the app's Documents directory.
    convenience init() {
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        self.init(root: documents)
    }

    /// Rooted anywhere — tests use a temporary directory.
    init(root: URL) {
        self.root = root
        try? FileManager.default.createDirectory(at: sessionsDirectory, withIntermediateDirectories: true)
        sessions = loadSessions()
        draft = loadDraft()
    }

    // MARK: The core loop

    /// Returns the in-progress draft, creating one if none survives from a
    /// previous run. Called when the writing surface appears.
    func beginOrResumeDraft(now: Date = Date()) -> DraftRecord {
        if let draft { return draft }
        let fresh = DraftRecord(id: UUID(), startedAt: now, text: "")
        draft = fresh
        return fresh
    }

    /// Persists the in-progress text atomically. Called on every change.
    func checkpoint(text: String) {
        guard var current = draft else { return }
        current.text = text
        draft = current
        if let data = try? encoder.encode(current) {
            try? data.write(to: draftURL, options: .atomic)
        }
    }

    /// Ends the session: commits the text file and sidecar atomically,
    /// removes the draft, and prepends the session to the list.
    /// Empty sessions are discarded rather than kept.
    @discardableResult
    func finishDraft(now: Date = Date()) -> SessionRecord? {
        guard let current = draft else { return nil }
        draft = nil
        try? FileManager.default.removeItem(at: draftURL)
        guard !current.text.isEmpty else { return nil }

        let record = SessionRecord(
            id: current.id,
            startedAt: current.startedAt,
            endedAt: now,
            characterCount: current.text.count
        )
        do {
            try Data(current.text.utf8).write(to: textURL(for: record.id), options: .atomic)
            try encoder.encode(record).write(to: sidecarURL(for: record.id), options: .atomic)
        } catch {
            return nil
        }
        sessions.insert(record, at: 0)
        return record
    }

    /// Reads a kept session's text.
    func text(for record: SessionRecord) -> String {
        (try? String(contentsOf: textURL(for: record.id), encoding: .utf8)) ?? ""
    }

    // MARK: Files

    private func textURL(for id: UUID) -> URL {
        sessionsDirectory.appendingPathComponent("\(id.uuidString).txt")
    }

    private func sidecarURL(for id: UUID) -> URL {
        sessionsDirectory.appendingPathComponent("\(id.uuidString).json")
    }

    private func loadDraft() -> DraftRecord? {
        guard let data = try? Data(contentsOf: draftURL) else { return nil }
        return try? decoder.decode(DraftRecord.self, from: data)
    }

    private func loadSessions() -> [SessionRecord] {
        let contents = (try? FileManager.default.contentsOfDirectory(
            at: sessionsDirectory,
            includingPropertiesForKeys: nil
        )) ?? []
        return contents
            .filter { $0.pathExtension == "json" }
            .compactMap { url -> SessionRecord? in
                guard let data = try? Data(contentsOf: url) else { return nil }
                return try? decoder.decode(SessionRecord.self, from: data)
            }
            .sorted { $0.endedAt > $1.endedAt }
    }
}
