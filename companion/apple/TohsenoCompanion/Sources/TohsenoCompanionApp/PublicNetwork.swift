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
        public let publicCheckpointDigest: String
        public let display: Display
        public let permissions: Permissions

        enum CodingKeys: String, CodingKey {
            case shotID = "shot_id"
            case builderID = "builder_id"
            case checkpointSequence = "checkpoint_sequence"
            case publicCheckpointDigest = "public_checkpoint_digest"
            case display, permissions
        }
    }

    public let releaseDigest: String
    public let route: String
    public let release: Release
    public let sourceURL: String
    public let iconURL: String?

    public var id: String { releaseDigest }

    enum CodingKeys: String, CodingKey {
        case releaseDigest = "release_digest"
        case route, release
        case sourceURL = "source_url"
        case iconURL = "icon_url"
    }
}

public struct PublicClaimEdition: Codable, Equatable, Sendable {
    public struct Policy: Codable, Equatable, Sendable {
        public let kind: String
        public let maxClaims: String?
        public let closesAt: String?
        enum CodingKeys: String, CodingKey {
            case kind
            case maxClaims = "max_claims"
            case closesAt = "closes_at"
        }
    }
    public let schema: String
    public let shotID: String
    public let opened: Bool
    public let policy: Policy?
    public let totalClaims: String
    public let openedAt: String?
    public let closed: Bool
    enum CodingKeys: String, CodingKey {
        case schema, opened, policy, closed
        case shotID = "shot_id"
        case totalClaims = "total_claims"
        case openedAt = "opened_at"
    }
}

public struct AliasClaimReceipt: Codable, Equatable, Sendable {
    public let schema: String
    public let requestID: String
    public let alias: String
    public let status: String

    enum CodingKeys: String, CodingKey {
        case schema, alias, status
        case requestID = "request_id"
    }
}

public struct SoftwareClaimPreparation: Codable, Equatable, Sendable {
    public struct Edition: Codable, Equatable, Sendable {
        public let maxClaims: UInt64
        public let closesAt: UInt64
        public let totalClaims: UInt64
        enum CodingKeys: String, CodingKey {
            case maxClaims = "max_claims"
            case closesAt = "closes_at"
            case totalClaims = "total_claims"
        }
    }
    public let schema: String
    public let jobID: String
    public let jobToken: String
    public let chainID: UInt64
    public let claimsContract: String
    public let claimsActivationSigningDigest: String
    public let shotRegistry: String
    public let shotID: String
    public let builderID: String
    public let releaseDigest: String
    public let checkpointDigest: String
    public let checkpointSequence: UInt64
    public let claimant: String
    public let edition: Edition
    public let gestureCommitment: String
    public let nonce: UInt64
    public let deadline: UInt64
    enum CodingKeys: String, CodingKey {
        case schema, claimant, edition, nonce, deadline
        case jobID = "job_id"
        case jobToken = "job_token"
        case chainID = "chain_id"
        case claimsContract = "claims_contract"
        case claimsActivationSigningDigest = "claims_activation_signing_digest"
        case shotRegistry = "shot_registry"
        case shotID = "shot_id"
        case builderID = "builder_id"
        case releaseDigest = "release_digest"
        case checkpointDigest = "checkpoint_digest"
        case checkpointSequence = "checkpoint_sequence"
        case gestureCommitment = "gesture_commitment"
    }
}

public struct PublicSoftwareClaim: Codable, Equatable, Sendable {
    public let tokenID: String
    public let shotID: String
    public let claimNumber: String
    public let claimant: String
    public let releaseDigest: String
    public let checkpointDigest: String
    public let gestureCommitment: String
    public let transactionHash: String?
    enum CodingKeys: String, CodingKey {
        case claimant
        case tokenID = "token_id"
        case shotID = "shot_id"
        case claimNumber = "claim_number"
        case releaseDigest = "release_digest"
        case checkpointDigest = "checkpoint_digest"
        case gestureCommitment = "gesture_commitment"
        case transactionHash = "transaction_hash"
    }
}

public struct SoftwareClaimStatus: Codable, Equatable, Sendable {
    public let schema: String
    public let jobID: String
    public let status: String
    public let shotID: String
    public let releaseDigest: String
    public let gestureCommitment: String
    public let claim: PublicSoftwareClaim?
    public let failure: String?
    enum CodingKeys: String, CodingKey {
        case schema, status, claim, failure
        case jobID = "job_id"
        case shotID = "shot_id"
        case releaseDigest = "release_digest"
        case gestureCommitment = "gesture_commitment"
    }
}

private struct PublicSoftwareClaimState: Codable, Sendable {
    let schema: String
    let claimed: Bool
    let claim: PublicSoftwareClaim?
}

private struct SoftwareClaimPreparationRequest: Encodable {
    let releaseDigest: String
    let claimant: String
    let claimMark: String
    let builderDevice: BuilderDeviceAnnouncement
    enum CodingKeys: String, CodingKey {
        case claimant
        case releaseDigest = "release_digest"
        case claimMark = "claim_mark"
        case builderDevice = "builder_device"
    }
}

private struct PublicCatalogPage: Codable, Sendable {
    let schema: String
    let releases: [PublicAppRelease]
}

public struct PublicTimelineEvent: Codable, Equatable, Identifiable, Sendable {
    public let schema: String
    public let eventID: String
    public let kind: String
    public let shotID: String
    public let builderID: String
    public let releaseDigest: String
    public let checkpointSequence: UInt64
    public let occurredAt: String
    public var id: String { eventID }
    enum CodingKeys: String, CodingKey {
        case schema, kind
        case eventID = "event_id"
        case shotID = "shot_id"
        case builderID = "builder_id"
        case releaseDigest = "release_digest"
        case checkpointSequence = "checkpoint_sequence"
        case occurredAt = "occurred_at"
    }
}

private struct PublicTimelinePage: Codable, Sendable {
    let schema: String
    let events: [PublicTimelineEvent]
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

public struct ClaimsClientCoordinates: Equatable, Sendable {
    public let shotRegistry: String
    public let claimsContract: String?
    public let activationSigningDigest: String?

    public static let released = ClaimsClientCoordinates(
        shotRegistry: ClaimsClientActivation.shotRegistry,
        claimsContract: ClaimsClientActivation.claimsContract,
        activationSigningDigest: ClaimsClientActivation.activationSigningDigest
    )

    public var active: Bool {
        claimsContract != nil && activationSigningDigest != nil
    }

    public init(
        shotRegistry: String,
        claimsContract: String?,
        activationSigningDigest: String?
    ) {
        self.shotRegistry = shotRegistry
        self.claimsContract = claimsContract
        self.activationSigningDigest = activationSigningDigest
    }
}

public struct PublicNetworkClient: Sendable {
    public typealias Transport = @Sendable (URLRequest) async throws -> (Data, URLResponse)

    public static let production = PublicNetworkClient(origin: URL(string: "https://tohseno.com")!)
    public let origin: URL
    public let claims: ClaimsClientCoordinates
    private let transport: Transport

    public init(
        origin: URL,
        claims: ClaimsClientCoordinates = .released,
        transport: @escaping Transport = { try await URLSession.shared.data(for: $0) }
    ) {
        self.origin = origin
        self.claims = claims
        self.transport = transport
    }

    public func releases() async throws -> [PublicAppRelease] {
        try await releases(at: origin.appending(path: "api/registry/v1/shots"))
    }

    public func timeline() async throws -> [PublicTimelineEvent] {
        let url = origin.appending(path: "api/registry/v1/timeline")
        let (data, http) = try await request(url: url)
        guard http.statusCode == 200, data.count <= 2 * 1024 * 1024 else {
            throw URLError(.badServerResponse)
        }
        let page = try JSONDecoder().decode(PublicTimelinePage.self, from: data)
        guard page.schema == "tohseno.registry-timeline-page/1", page.events.count <= 100 else {
            throw URLError(.cannotParseResponse)
        }
        return page.events
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

    public func requestAlias(_ envelope: SignedAliasClaim) async throws -> AliasClaimReceipt {
        let url = origin.appending(path: "api/registry/v1/aliases/claims")
        let body = try JSONEncoder().encode(EnvelopeRequest(envelope: envelope))
        let (data, response) = try await request(url: url, method: "POST", body: body)
        guard response.statusCode == 202, data.count <= 512 * 1024 else {
            throw URLError(.badServerResponse)
        }
        let receipt = try JSONDecoder().decode(AliasClaimReceipt.self, from: data)
        guard receipt.schema == "tohseno.alias-claim-receipt/1",
              isDigest(receipt.requestID),
              receipt.alias == envelope.claim.alias,
              receipt.status == "pending_policy_review"
        else { throw URLError(.cannotParseResponse) }
        return receipt
    }

    public func claimEdition(shotID: String) async throws -> PublicClaimEdition {
        guard claims.active else {
            throw URLError(.userAuthenticationRequired)
        }
        guard isDigest(shotID) else { throw URLError(.badURL) }
        let url = origin.appending(path: "api/registry/v1/shots")
            .appending(path: shotID).appending(path: "claim-edition")
        let (data, response) = try await request(url: url)
        guard response.statusCode == 200, data.count <= 128 * 1024 else {
            throw URLError(.badServerResponse)
        }
        let value = try JSONDecoder().decode(PublicClaimEdition.self, from: data)
        guard value.schema == "tohseno.claim-edition/1", value.shotID == shotID,
              isCanonicalDecimal(value.totalClaims, allowZero: true),
              validEditionPolicy(value)
        else { throw URLError(.cannotParseResponse) }
        return value
    }

    public func softwareClaim(shotID: String, claimant: String) async throws -> PublicSoftwareClaim? {
        guard claims.active else {
            throw URLError(.userAuthenticationRequired)
        }
        guard isDigest(shotID), claimant.range(of: #"^0x[0-9a-f]{40}$"#, options: .regularExpression) != nil
        else { throw URLError(.badURL) }
        let url = origin.appending(path: "api/registry/v1/shots").appending(path: shotID)
            .appending(path: "claims").appending(path: claimant)
        let (data, response) = try await request(url: url)
        guard response.statusCode == 200, data.count <= 256 * 1024 else { throw URLError(.badServerResponse) }
        let value = try JSONDecoder().decode(PublicSoftwareClaimState.self, from: data)
        guard value.schema == "tohseno.software-claim-state/1",
              value.claimed == (value.claim != nil)
        else { throw URLError(.cannotParseResponse) }
        if let claim = value.claim {
            guard validClaim(claim, shotID: shotID, claimant: claimant) else {
                throw URLError(.cannotParseResponse)
            }
        }
        return value.claim
    }

    public func prepareSoftwareClaim(
        app: PublicAppRelease,
        claimant: String,
        mark: ClaimMark,
        builderDevice: BuilderDeviceAnnouncement
    ) async throws -> SoftwareClaimPreparation {
        guard let trustedContract = claims.claimsContract,
              let trustedActivation = claims.activationSigningDigest else {
            throw URLError(.userAuthenticationRequired)
        }
        let url = origin.appending(path: "api/registry/v1/shots")
            .appending(path: app.release.shotID).appending(path: "claims").appending(path: "prepare")
        let body = try JSONEncoder().encode(SoftwareClaimPreparationRequest(
            releaseDigest: app.releaseDigest,
            claimant: claimant,
            claimMark: mark.canonicalBytes.prefixedHex,
            builderDevice: builderDevice
        ))
        let (data, response) = try await request(url: url, method: "POST", body: body)
        guard response.statusCode == 201, data.count <= 256 * 1024 else {
            throw URLError(.badServerResponse)
        }
        let value = try JSONDecoder().decode(SoftwareClaimPreparation.self, from: data)
        guard value.schema == "tohseno.software-claim-preparation/1",
              value.chainID == ClaimsActionEncoding.activeChainID,
              value.claimsContract == trustedContract,
              value.claimsActivationSigningDigest == trustedActivation,
              value.shotRegistry == claims.shotRegistry,
              value.shotID == app.release.shotID,
              value.builderID == app.release.builderID,
              value.releaseDigest == app.releaseDigest,
              value.checkpointDigest == app.release.publicCheckpointDigest,
              value.checkpointSequence == app.release.checkpointSequence,
              value.claimant == claimant,
              value.gestureCommitment == mark.gestureCommitment.prefixedHex,
              value.jobID.range(of: #"^[0-9a-f]{32}$"#, options: .regularExpression) != nil,
              value.jobToken.range(of: #"^[0-9a-f]{64}$"#, options: .regularExpression) != nil,
              value.deadline > UInt64(Date().timeIntervalSince1970.rounded(.down)),
              value.deadline <= ClaimsActionEncoding.maximumSafeInteger,
              value.nonce <= ClaimsActionEncoding.maximumSafeInteger,
              value.edition.maxClaims <= ClaimsActionEncoding.maximumSafeInteger,
              value.edition.closesAt <= ClaimsActionEncoding.maximumSafeInteger,
              value.edition.totalClaims <= ClaimsActionEncoding.maximumSafeInteger,
              value.edition.maxClaims == 0 || value.edition.totalClaims < value.edition.maxClaims
        else { throw URLError(.cannotParseResponse) }
        return value
    }

    public func submitSoftwareClaim(
        _ authorization: SoftwareClaimAuthorization,
        preparation: SoftwareClaimPreparation
    ) async throws -> SoftwareClaimStatus {
        guard claims.active else {
            throw URLError(.userAuthenticationRequired)
        }
        let url = origin.appending(path: "api/registry/v1/claims").appending(path: "jobs")
            .appending(path: preparation.jobID).appending(path: "submit")
        let body = try JSONEncoder().encode(authorization)
        let (data, response) = try await request(
            url: url, method: "POST", body: body, bearer: preparation.jobToken
        )
        guard response.statusCode == 202, data.count <= 256 * 1024 else {
            throw URLError(.badServerResponse)
        }
        let value = try JSONDecoder().decode(SoftwareClaimStatus.self, from: data)
        guard validStatus(value, preparation: preparation) else {
            throw URLError(.cannotParseResponse)
        }
        return value
    }

    public func softwareClaimStatus(_ preparation: SoftwareClaimPreparation) async throws -> SoftwareClaimStatus {
        guard claims.active else {
            throw URLError(.userAuthenticationRequired)
        }
        let url = origin.appending(path: "api/registry/v1/claims").appending(path: "jobs")
            .appending(path: preparation.jobID)
        let (data, response) = try await request(url: url, bearer: preparation.jobToken)
        guard [200, 202, 422].contains(response.statusCode), data.count <= 256 * 1024 else {
            throw URLError(.badServerResponse)
        }
        let value = try JSONDecoder().decode(SoftwareClaimStatus.self, from: data)
        guard validStatus(value, preparation: preparation) else {
            throw URLError(.cannotParseResponse)
        }
        return value
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

    private func isAddress(_ value: String) -> Bool {
        value.range(of: #"^0x[0-9a-f]{40}$"#, options: .regularExpression) != nil
    }

    private func isCanonicalDecimal(_ value: String, allowZero: Bool) -> Bool {
        let pattern = allowZero ? #"^(?:0|[1-9][0-9]*)$"# : #"^[1-9][0-9]*$"#
        guard value.range(of: pattern, options: .regularExpression) != nil,
              let parsed = UInt64(value)
        else { return false }
        return parsed <= ClaimsActionEncoding.maximumSafeInteger
    }

    private func validEditionPolicy(_ edition: PublicClaimEdition) -> Bool {
        guard edition.opened == (edition.policy != nil) else { return false }
        guard let policy = edition.policy else {
            return edition.openedAt == nil && edition.totalClaims == "0" && !edition.closed
        }
        guard edition.openedAt.map({ isCanonicalDecimal($0, allowZero: false) }) == true else {
            return false
        }
        switch policy.kind {
        case "open": return policy.maxClaims == nil && policy.closesAt == nil
        case "limited":
            return policy.maxClaims.map { isCanonicalDecimal($0, allowZero: false) } == true
                && policy.closesAt == nil
        case "timed":
            return policy.maxClaims == nil
                && policy.closesAt.map { isCanonicalDecimal($0, allowZero: false) } == true
        case "limited_timed":
            return policy.maxClaims.map { isCanonicalDecimal($0, allowZero: false) } == true
                && policy.closesAt.map { isCanonicalDecimal($0, allowZero: false) } == true
        default: return false
        }
    }

    private func validClaim(
        _ claim: PublicSoftwareClaim,
        shotID: String,
        claimant: String
    ) -> Bool {
        isCanonicalDecimal(claim.tokenID, allowZero: false)
            && isCanonicalDecimal(claim.claimNumber, allowZero: false)
            && claim.shotID == shotID
            && claim.claimant == claimant
            && isAddress(claim.claimant)
            && isDigest(claim.releaseDigest)
            && isDigest(claim.checkpointDigest)
            && isDigest(claim.gestureCommitment)
            && (claim.transactionHash == nil || isDigest(claim.transactionHash!))
    }

    private func validStatus(
        _ status: SoftwareClaimStatus,
        preparation: SoftwareClaimPreparation
    ) -> Bool {
        guard status.schema == "tohseno.software-claim-status/1",
              status.jobID == preparation.jobID,
              status.shotID == preparation.shotID,
              status.releaseDigest == preparation.releaseDigest,
              status.gestureCommitment == preparation.gestureCommitment,
              ["prepared", "authorized", "account_pending", "claim_submitted", "complete", "failed"]
                .contains(status.status)
        else { return false }
        switch status.status {
        case "complete":
            guard let claim = status.claim,
                  validClaim(claim, shotID: preparation.shotID, claimant: preparation.claimant)
            else { return false }
            return claim.releaseDigest == preparation.releaseDigest
                && claim.checkpointDigest == preparation.checkpointDigest
                && claim.gestureCommitment == preparation.gestureCommitment
                && status.failure == nil
        case "failed": return status.claim == nil && status.failure?.isEmpty == false
        default: return status.claim == nil && status.failure == nil
        }
    }

    private func request(
        url: URL,
        method: String = "GET",
        body: Data? = nil,
        bearer: String? = nil
    ) async throws -> (Data, HTTPURLResponse) {
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = 15
        request.cachePolicy = .reloadRevalidatingCacheData
        if let body {
            request.httpBody = body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        if let bearer { request.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization") }
        let (data, response) = try await transport(request)
        guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        return (data, http)
    }
}

private extension Data {
    var prefixedHex: String { "0x" + map { String(format: "%02x", $0) }.joined() }
}
