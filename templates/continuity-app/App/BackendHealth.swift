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
    private static let maximumResponseBytes = 65_536

    static func check(baseURL: URL?) async -> BackendHealthState {
        guard let baseURL else { return .notConfigured }
        let healthURL = baseURL.appending(path: "health")
        var request = URLRequest(url: healthURL)
        request.timeoutInterval = 4
        request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        do {
            let (bytes, response) = try await URLSession.shared.bytes(for: request)
            guard let http = response as? HTTPURLResponse,
                  http.statusCode == 200 else {
                return .unavailable
            }
            if let expected = http.value(forHTTPHeaderField: "Content-Length"),
               let length = Int(expected),
               length > maximumResponseBytes {
                return .unavailable
            }
            var data = Data()
            data.reserveCapacity(min(http.expectedContentLength > 0
                ? Int(http.expectedContentLength)
                : 0, maximumResponseBytes))
            for try await byte in bytes {
                guard data.count < maximumResponseBytes else {
                    return .unavailable
                }
                data.append(byte)
            }
            guard
                  let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  object["status"] as? String == "ok",
                  object["ready"] as? Bool == true,
                  object["service"] as? String == "shot-api" else {
                return .unavailable
            }
            return .available
        } catch {
            return .unavailable
        }
    }
}
