import Foundation

public struct ApplicationUpdate: Equatable, Sendable {
    public let version: String
    public let buildNumber: Int
    public let channel: String
    public let downloadURL: URL

    public init(version: String, buildNumber: Int, channel: String, downloadURL: URL) {
        self.version = version
        self.buildNumber = buildNumber
        self.channel = channel
        self.downloadURL = downloadURL
    }
}

public protocol ApplicationUpdateChecking: Sendable {
    func availableUpdate() async -> ApplicationUpdate?
}

public struct WebsiteApplicationUpdateChecker: ApplicationUpdateChecking {
    private let endpoint: URL
    private let downloadURL: URL
    private let currentBuildNumber: Int
    private let urlSession: URLSession

    public init(
        endpoint: URL = URL(string: "https://tohseno.com/api/distribution/v1/macos")!,
        downloadURL: URL = URL(string: "https://tohseno.com/download/macos")!,
        currentBuildNumber: Int? = nil,
        urlSession: URLSession? = nil
    ) {
        self.endpoint = endpoint
        self.downloadURL = downloadURL
        self.currentBuildNumber = currentBuildNumber
            ?? Int(Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "")
            ?? 0
        if let urlSession {
            self.urlSession = urlSession
        } else {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
            configuration.timeoutIntervalForRequest = 6
            self.urlSession = URLSession(configuration: configuration)
        }
    }

    public func availableUpdate() async -> ApplicationUpdate? {
        var request = URLRequest(url: endpoint)
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.timeoutInterval = 6
        do {
            let (data, response) = try await urlSession.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else { return nil }
            return Self.availableUpdate(
                from: data,
                currentBuildNumber: currentBuildNumber,
                downloadURL: downloadURL
            )
        } catch {
            return nil
        }
    }

    static func availableUpdate(
        from data: Data,
        currentBuildNumber: Int,
        downloadURL: URL
    ) -> ApplicationUpdate? {
        guard let projection = try? JSONDecoder().decode(DistributionProjection.self, from: data),
              projection.schema == "tohseno.macos-distribution/1",
              projection.available,
              ["release-candidate", "stable"].contains(projection.channel),
              projection.buildNumber > currentBuildNumber,
              projection.version.range(
                of: #"^\d+\.\d+\.\d+(?:-rc\.\d+)?$"#,
                options: .regularExpression
              ) != nil,
              projection.sha256.range(of: #"^[a-f0-9]{64}$"#, options: .regularExpression) != nil,
              let artifactURL = URL(string: projection.url),
              let components = URLComponents(url: artifactURL, resolvingAgainstBaseURL: false),
              components.scheme == "https",
              components.host != nil,
              components.user == nil,
              components.password == nil,
              components.fragment == nil else { return nil }
        return ApplicationUpdate(
            version: projection.version,
            buildNumber: projection.buildNumber,
            channel: projection.channel,
            downloadURL: downloadURL
        )
    }
}

private struct DistributionProjection: Decodable, Sendable {
    let schema: String
    let available: Bool
    let channel: String
    let version: String
    let buildNumber: Int
    let url: String
    let sha256: String

    enum CodingKeys: String, CodingKey {
        case schema
        case available
        case channel
        case version
        case buildNumber = "build_number"
        case url
        case sha256
    }
}
