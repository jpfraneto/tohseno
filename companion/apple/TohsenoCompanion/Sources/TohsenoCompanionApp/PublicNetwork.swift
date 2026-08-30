import Foundation
import TohsenoCompanionKit

public struct PublicAppRelease: Codable, Equatable, Identifiable, Sendable {
    public struct Release: Codable, Equatable, Sendable {
        public struct Display: Codable, Equatable, Sendable {
            public let name: String
            public let description: String
            public let builderHandle: String?
            public let appSlug: String?

            enum CodingKeys: String, CodingKey {
                case name, description
                case builderHandle = "builder_handle"
                case appSlug = "app_slug"
            }
        }

        public struct Permissions: Codable, Equatable, Sendable {
            public let installAllowed: Bool
            public let forkAllowed: Bool

            enum CodingKeys: String, CodingKey {
                case installAllowed = "install_allowed"
                case forkAllowed = "fork_allowed"
            }
        }

        public let shotID: String
        public let builderID: String
        public let checkpointSequence: UInt64
        public let display: Display
        public let permissions: Permissions

        enum CodingKeys: String, CodingKey {
            case shotID = "shot_id"
            case builderID = "builder_id"
            case checkpointSequence = "checkpoint_sequence"
            case display, permissions
        }
    }

    public let releaseDigest: String
    public let route: String
    public let release: Release
    public let sourceURL: String

    public var id: String { releaseDigest }

    enum CodingKeys: String, CodingKey {
        case releaseDigest = "release_digest"
        case route, release
        case sourceURL = "source_url"
    }
}

private struct PublicCatalogPage: Codable, Sendable {
    let schema: String
    let releases: [PublicAppRelease]
}

private struct PublicBuilderPage: Codable, Sendable {
    let schema: String
    let builderID: String
    let profile: BuilderProfile?

    enum CodingKeys: String, CodingKey {
        case schema, profile
        case builderID = "builder_id"
    }
}

private struct EnvelopeRequest<Value: Encodable>: Encodable {
    let envelope: Value
}

public struct PublicNetworkClient: Sendable {
    public static let production = PublicNetworkClient(origin: URL(string: "https://tohseno.com")!)
    public let origin: URL

    public func releases() async throws -> [PublicAppRelease] {
        try await releases(at: origin.appending(path: "api/registry/v1/shots"))
    }

    public func release(shotID: String, releaseDigest: String) async throws -> PublicAppRelease {
        guard isDigest(shotID), isDigest(releaseDigest) else {
            throw URLError(.badURL)
        }
        let url = origin
            .appending(path: "api/registry/v1/shots")
            .appending(path: shotID)
            .appending(path: "releases")
        let values = try await releases(at: url)
        guard let value = values.first(where: { $0.releaseDigest == releaseDigest }) else {
            throw URLError(.resourceUnavailable)
        }
        return value
    }

    public func builderProfile(builderID: String) async throws -> BuilderProfile? {
        guard isBuilderID(builderID) else { throw URLError(.badURL) }
        let url = origin.appending(path: "api/registry/v1/builders").appending(path: builderID)
        let (data, response) = try await request(url: url)
        guard response.statusCode == 200, data.count <= 512 * 1024 else {
            if response.statusCode == 404 { return nil }
            throw URLError(.badServerResponse)
        }
        let value = try JSONDecoder().decode(PublicBuilderPage.self, from: data)
        guard value.schema == "tohseno.builder-page/1", value.builderID == builderID else {
            throw URLError(.cannotParseResponse)
        }
        return value.profile
    }

    public func updateProfile(_ envelope: SignedBuilderProfile) async throws {
        guard isBuilderID(envelope.profile.builderID) else { throw URLError(.badURL) }
        let url = origin.appending(path: "api/registry/v1/builders")
            .appending(path: envelope.profile.builderID).appending(path: "profile")
        let body = try JSONEncoder().encode(EnvelopeRequest(envelope: envelope))
        let (data, response) = try await request(url: url, method: "PUT", body: body)
        guard response.statusCode == 200, data.count <= 512 * 1024 else {
            throw URLError(.badServerResponse)
        }
    }

    public func requestAlias(_ envelope: SignedAliasClaim) async throws {
        let url = origin.appending(path: "api/registry/v1/aliases/claims")
        let body = try JSONEncoder().encode(EnvelopeRequest(envelope: envelope))
        let (data, response) = try await request(url: url, method: "POST", body: body)
        guard response.statusCode == 202, data.count <= 512 * 1024 else {
            throw URLError(.badServerResponse)
        }
    }

    private func releases(at url: URL) async throws -> [PublicAppRelease] {
        let (data, http) = try await request(url: url)
        guard http.statusCode == 200,
              data.count <= 2 * 1024 * 1024
        else { throw URLError(.badServerResponse) }
        let value = try JSONDecoder().decode(PublicCatalogPage.self, from: data)
        guard value.schema == "tohseno.catalog-page/1", value.releases.count <= 100 else {
            throw URLError(.cannotParseResponse)
        }
        return value.releases
    }

    private func isDigest(_ value: String) -> Bool {
        value.utf8.count == 66 && value.hasPrefix("0x")
            && value.dropFirst(2).utf8.allSatisfy { (48 ... 57).contains($0) || (97 ... 102).contains($0) }
            && value.dropFirst(2).contains { $0 != "0" }
    }

    private func isBuilderID(_ value: String) -> Bool {
        value.range(of: #"^eip155:4663:0x[0-9a-f]{40}$"#, options: .regularExpression) != nil
    }

    private func request(
        url: URL,
        method: String = "GET",
        body: Data? = nil
    ) async throws -> (Data, HTTPURLResponse) {
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = 15
        request.cachePolicy = .reloadRevalidatingCacheData
        if let body {
            request.httpBody = body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        return (data, http)
    }
}
