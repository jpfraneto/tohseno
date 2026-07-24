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
    @Published private(set) var persistenceError = false

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
        do {
            try FileManager.default.createDirectory(
                at: sessionsDirectory,
                withIntermediateDirectories: true,
                attributes: [.protectionKey: FileProtectionType.complete]
            )
            try excludeFromBackup(sessionsDirectory)
        } catch {
            persistenceError = true
        }
        let loadedSessions = loadSessions()
        let loadedDraft = loadDraft()
        sessions = loadedSessions
        if let loadedDraft,
           loadedSessions.contains(where: { $0.id == loadedDraft.id }) {
            // The text and sidecar became durable but process death (or a
            // removal error) left the old recovery checkpoint. The immutable
            // session wins; never resurrect it as a duplicate draft.
            draft = nil
            try? FileManager.default.removeItem(at: draftURL)
        } else {
            draft = loadedDraft
        }
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
        do {
            try writeProtected(encoder.encode(current), to: draftURL)
            persistenceError = false
        } catch {
            // Keep the newest text in memory and the last successful
            // checkpoint on disk. The UI makes the failure visible.
            persistenceError = true
        }
    }

    /// Ends the session: commits the text file and sidecar atomically,
    /// removes the draft, and prepends the session to the list.
    /// Empty sessions are discarded rather than kept.
    @discardableResult
    func finishDraft(now: Date = Date()) -> SessionRecord? {
        guard let current = draft else { return nil }
        guard !current.text.isEmpty else {
            do {
                if FileManager.default.fileExists(atPath: draftURL.path) {
                    try FileManager.default.removeItem(at: draftURL)
                }
                draft = nil
                persistenceError = false
            } catch {
                persistenceError = true
            }
            return nil
        }

        let record = SessionRecord(
            id: current.id,
            startedAt: current.startedAt,
            endedAt: now,
            characterCount: current.text.count
        )
        do {
            try writeProtected(Data(current.text.utf8), to: textURL(for: record.id))
            try writeProtected(encoder.encode(record), to: sidecarURL(for: record.id))
            if FileManager.default.fileExists(atPath: draftURL.path) {
                try FileManager.default.removeItem(at: draftURL)
            }
        } catch {
            // Draft memory and draft.json remain the recovery source until
            // both committed files and checkpoint cleanup have succeeded.
            persistenceError = true
            return nil
        }
        draft = nil
        persistenceError = false
        sessions.insert(record, at: 0)
        return record
    }

    func dismissPersistenceError() {
        persistenceError = false
    }

    /// Reads a kept session's text.
    func text(for record: SessionRecord) -> String {
        (try? String(contentsOf: textURL(for: record.id), encoding: .utf8)) ?? ""
    }

    /// The canonical bytes promised by the manifest. Passing these URLs to
    /// ShareLink is the explicit owner-selected disclosure boundary; no copy
    /// or upload happens before the system share sheet is opened.
    func exportURLs(for record: SessionRecord) -> [URL] {
        guard sessions.contains(record) else { return [] }
        let text = textURL(for: record.id)
        let sidecar = sidecarURL(for: record.id)
        guard isRegularFile(text),
              isRegularFile(sidecar),
              let textValue = try? String(contentsOf: text, encoding: .utf8),
              textValue.count == record.characterCount,
              let sidecarData = try? Data(contentsOf: sidecar),
              let storedRecord = try? decoder.decode(SessionRecord.self, from: sidecarData),
              storedRecord == record else {
            return []
        }
        return [text, sidecar]
    }

    // MARK: Files

    private func textURL(for id: UUID) -> URL {
        sessionsDirectory.appendingPathComponent("\(id.uuidString).txt")
    }

    private func sidecarURL(for id: UUID) -> URL {
        sessionsDirectory.appendingPathComponent("\(id.uuidString).json")
    }

    private func writeProtected(_ data: Data, to url: URL) throws {
        try data.write(to: url, options: [.atomic, .completeFileProtection])
        try excludeFromBackup(url)
    }

    private func excludeFromBackup(_ url: URL) throws {
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        var mutableURL = url
        try mutableURL.setResourceValues(values)
    }

    private func loadDraft() -> DraftRecord? {
        guard let data = try? Data(contentsOf: draftURL) else { return nil }
        return try? decoder.decode(DraftRecord.self, from: data)
    }

    private func isRegularFile(_ url: URL) -> Bool {
        guard let values = try? url.resourceValues(
            forKeys: [.isRegularFileKey, .isSymbolicLinkKey]
        ) else {
            return false
        }
        return values.isRegularFile == true && values.isSymbolicLink != true
    }

    private func loadSessions() -> [SessionRecord] {
        let contents = (try? FileManager.default.contentsOfDirectory(
            at: sessionsDirectory,
            includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey]
        )) ?? []
        return contents
            .filter { $0.pathExtension == "json" }
            .compactMap { url -> SessionRecord? in
                guard isRegularFile(url) else {
                    return nil
                }
                guard let data = try? Data(contentsOf: url) else { return nil }
                guard let record = try? decoder.decode(SessionRecord.self, from: data),
                      url.deletingPathExtension().lastPathComponent.lowercased()
                        == record.id.uuidString.lowercased(),
                      record.startedAt <= record.endedAt else {
                    return nil
                }
                let textURL = textURL(for: record.id)
                guard isRegularFile(textURL),
                      let text = try? String(contentsOf: textURL, encoding: .utf8),
                      text.count == record.characterCount else {
                    return nil
                }
                return record
            }
            .sorted { $0.endedAt > $1.endedAt }
    }
}
