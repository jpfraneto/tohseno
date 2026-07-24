# Local Progress

This skill owns aggregate challenge progress for the first shot. It persists a
small codable record in `UserDefaults`, has no network path, and requires no
identity. Extend the record deliberately rather than creating a parallel
progress store.
