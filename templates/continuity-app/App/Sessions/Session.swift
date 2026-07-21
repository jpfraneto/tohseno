import Foundation

/// One kept writing session: a plain text file plus this JSON sidecar.
/// The sidecar is small on purpose — a log entry, not a profile.
struct SessionRecord: Codable, Identifiable, Hashable {
    let id: UUID
    let startedAt: Date
    let endedAt: Date
    let characterCount: Int
}

/// The in-progress session. Persisted as a draft on every change so a
/// killed process never loses committed text.
struct DraftRecord: Codable, Equatable {
    let id: UUID
    let startedAt: Date
    var text: String
}
