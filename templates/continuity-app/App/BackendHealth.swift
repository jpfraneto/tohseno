import Foundation

enum BackendHealthState: Equatable {
    case notConfigured
    case checking
    case available
    case unavailable

    var label: String {
        switch self {
        case .notConfigured: return "Not configured"
        case .checking: return "Checking…"
        case .available: return "Available"
        case .unavailable: return "Unavailable"
        }
    }
}

enum BackendHealth {
    static func check(baseURL: URL?) async -> BackendHealthState {
        guard let baseURL else { return .notConfigured }
        let healthURL = baseURL.appending(path: "health")
        var request = URLRequest(url: healthURL)
        request.timeoutInterval = 4
        request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse,
                  http.statusCode == 200,
                  let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  object["status"] as? String == "ok",
                  object["service"] as? String == "shot-api" else {
                return .unavailable
            }
            return .available
        } catch {
            return .unavailable
        }
    }
}
