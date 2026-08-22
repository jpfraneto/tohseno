import Foundation

public struct RelayMailboxEnvelope: Codable, Equatable, Sendable {
    public let cursor: UInt64
    public let envelope: OpaqueCompanionEnvelope

    public init(cursor: UInt64, envelope: OpaqueCompanionEnvelope) {
        self.cursor = cursor
        self.envelope = envelope
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["cursor", "envelope"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        cursor = try container.decode(UInt64.self, forKey: .cursor)
        envelope = try container.decode(OpaqueCompanionEnvelope.self, forKey: .envelope)
    }

    private enum CodingKeys: String, CodingKey { case cursor, envelope }
}

public struct RelayMailboxPage: Codable, Equatable, Sendable {
    public static let schemaV1 = "tohseno.companion-mailbox-page/1"
    public let schema: String
    public let envelopes: [RelayMailboxEnvelope]
    public let nextCursor: UInt64
    public let headCursor: UInt64
    public let hasMore: Bool

    enum CodingKeys: String, CodingKey {
        case schema, envelopes
        case nextCursor = "next_cursor"
        case headCursor = "head_cursor"
        case hasMore = "has_more"
    }

    public init(
        schema: String = schemaV1,
        envelopes: [RelayMailboxEnvelope],
        nextCursor: UInt64,
        headCursor: UInt64,
        hasMore: Bool
    ) {
        self.schema = schema
        self.envelopes = envelopes
        self.nextCursor = nextCursor
        self.headCursor = headCursor
        self.hasMore = hasMore
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "envelopes", "next_cursor", "head_cursor", "has_more",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        envelopes = try container.decode([RelayMailboxEnvelope].self, forKey: .envelopes)
        nextCursor = try container.decode(UInt64.self, forKey: .nextCursor)
        headCursor = try container.decode(UInt64.self, forKey: .headCursor)
        hasMore = try container.decode(Bool.self, forKey: .hasMore)
    }

    func validateRouting(mailboxID: String, afterCursor: UInt64) throws {
        guard schema == Self.schemaV1, envelopes.count <= 256,
              nextCursor >= afterCursor, headCursor >= nextCursor
        else { throw TohsenoCompanionError.invalidEncoding("mailbox page cursors") }
        var previous = afterCursor
        for item in envelopes {
            guard item.cursor > previous, item.cursor <= headCursor,
                  item.envelope.mailboxID == mailboxID
            else { throw TohsenoCompanionError.invalidEnvelope("misrouted mailbox page") }
            try item.envelope.validateShape()
            previous = item.cursor
        }
        if let last = envelopes.last {
            guard nextCursor == last.cursor else {
                throw TohsenoCompanionError.invalidEncoding("mailbox next cursor")
            }
        } else if nextCursor != afterCursor {
            throw TohsenoCompanionError.invalidEncoding("empty mailbox advanced cursor")
        }
        guard hasMore == (nextCursor < headCursor) else {
            throw TohsenoCompanionError.invalidEncoding("mailbox has_more differs")
        }
    }
}

private struct RelayMailboxReset: Codable {
    let schema: String
    let resetRequired: Bool
    let resetBeforeCursor: UInt64
    let headCursor: UInt64

    enum CodingKeys: String, CodingKey {
        case schema
        case resetRequired = "reset_required"
        case resetBeforeCursor = "reset_before_cursor"
        case headCursor = "head_cursor"
    }

    init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, [
            "schema", "reset_required", "reset_before_cursor", "head_cursor",
        ])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        resetRequired = try container.decode(Bool.self, forKey: .resetRequired)
        resetBeforeCursor = try container.decode(UInt64.self, forKey: .resetBeforeCursor)
        headCursor = try container.decode(UInt64.self, forKey: .headCursor)
    }
}

public struct RelayEnvelopeUploadReceipt: Codable, Equatable, Sendable {
    public let schema: String
    public let accepted: Bool
    public let duplicate: Bool
    public let cursor: UInt64

    public init(schema: String, accepted: Bool, duplicate: Bool, cursor: UInt64) {
        self.schema = schema
        self.accepted = accepted
        self.duplicate = duplicate
        self.cursor = cursor
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "accepted", "duplicate", "cursor"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        accepted = try container.decode(Bool.self, forKey: .accepted)
        duplicate = try container.decode(Bool.self, forKey: .duplicate)
        cursor = try container.decode(UInt64.self, forKey: .cursor)
    }

    private enum CodingKeys: String, CodingKey { case schema, accepted, duplicate, cursor }
}

public struct RelayMailboxVerifiers: Codable, Equatable, Sendable {
    public let schema: String
    public let writeVerifier: String
    public let readVerifier: String
    public let acknowledgementVerifier: String
    public let revocationVerifier: String
    public let pushVerifier: String

    enum CodingKeys: String, CodingKey {
        case schema
        case writeVerifier = "write_verifier"
        case readVerifier = "read_verifier"
        case acknowledgementVerifier = "ack_verifier"
        case revocationVerifier = "revoke_verifier"
        case pushVerifier = "push_verifier"
    }

    public init(
        writeVerifier: String,
        readVerifier: String,
        acknowledgementVerifier: String,
        revocationVerifier: String,
        pushVerifier: String
    ) {
        schema = "tohseno.companion-mailbox-create/1"
        self.writeVerifier = writeVerifier
        self.readVerifier = readVerifier
        self.acknowledgementVerifier = acknowledgementVerifier
        self.revocationVerifier = revocationVerifier
        self.pushVerifier = pushVerifier
    }
}

public struct RelayCreatedMailbox: Codable, Equatable, Sendable {
    public let schema: String
    public let mailboxID: String
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case schema
        case mailboxID = "mailbox_id"
        case createdAt = "created_at"
    }

    public init(schema: String, mailboxID: String, createdAt: String) {
        self.schema = schema
        self.mailboxID = mailboxID
        self.createdAt = createdAt
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "mailbox_id", "created_at"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        mailboxID = try container.decode(String.self, forKey: .mailboxID)
        createdAt = try container.decode(String.self, forKey: .createdAt)
    }
}

/// A content-blind wake delivered by the relay's bounded SSE stream. Envelope
/// bytes are validated here, but authoritative state is still obtained through
/// ordinary cursor reconciliation before any workspace event is applied.
public enum RelayLiveEvent: Equatable, Sendable {
    case envelope(RelayMailboxEnvelope)
    case reconcile
    case revoked(cursor: UInt64?)
}

public protocol CompanionRelayTransport: Sendable {
    func createMailbox(
        endpoint: RelayEndpoint,
        verifiers: RelayMailboxVerifiers
    ) async throws -> RelayCreatedMailbox

    func submitPairingResponse(
        endpoint: RelayEndpoint,
        sessionID: String,
        opaqueResponse: Data
    ) async throws

    func uploadEnvelope(
        endpoint: RelayEndpoint,
        mailboxID: String,
        writeCapability: String,
        envelope: OpaqueCompanionEnvelope
    ) async throws -> RelayEnvelopeUploadReceipt

    func fetchEnvelopes(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) async throws -> RelayMailboxPage

    func acknowledge(
        endpoint: RelayEndpoint,
        mailboxID: String,
        acknowledgementCapability: String,
        cursor: UInt64
    ) async throws

    func liveEvents(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) async throws -> AsyncThrowingStream<RelayLiveEvent, Error>

    func registerPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String,
        token: Data
    ) async throws

    func unregisterPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String
    ) async throws
}

public final class URLSessionCompanionRelayTransport: CompanionRelayTransport, @unchecked Sendable {
    private let session: URLSession

    public init(session: URLSession? = nil) {
        if let session {
            self.session = session
        } else {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.httpCookieAcceptPolicy = .never
            configuration.httpShouldSetCookies = false
            configuration.urlCache = nil
            configuration.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            configuration.timeoutIntervalForRequest = 30
            configuration.timeoutIntervalForResource = 60
            self.session = URLSession(configuration: configuration)
        }
    }

    public func createMailbox(
        endpoint: RelayEndpoint,
        verifiers: RelayMailboxVerifiers
    ) async throws -> RelayCreatedMailbox {
        var request = try makeRequest(endpoint, path: "v1/companion/mailboxes", method: "POST")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try StrictJSON.encode(verifiers)
        let data = try await execute(request, expected: [201], maximumBytes: 8 * 1024)
        let mailbox = try StrictJSON.decode(RelayCreatedMailbox.self, from: data, maximumBytes: 8 * 1024)
        guard mailbox.schema == "tohseno.companion-mailbox-created/1" else {
            throw TohsenoCompanionError.invalidEncoding("relay mailbox creation schema")
        }
        try requireIdentifier(mailbox.mailboxID, field: "mailbox_id")
        _ = try CompanionTimestamp.parse(mailbox.createdAt)
        return mailbox
    }

    public func submitPairingResponse(
        endpoint: RelayEndpoint,
        sessionID: String,
        opaqueResponse: Data
    ) async throws {
        try requireIdentifier(sessionID, field: "session_id")
        guard opaqueResponse.count <= 256 * 1024 else { throw TohsenoCompanionError.responseTooLarge }
        var request = try makeRequest(
            endpoint,
            path: "v1/companion/pairing-sessions/\(sessionID)/respond",
            method: "POST"
        )
        request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        request.httpBody = opaqueResponse
        _ = try await execute(request, expected: [200, 201], maximumBytes: 8 * 1024)
    }

    public func uploadEnvelope(
        endpoint: RelayEndpoint,
        mailboxID: String,
        writeCapability: String,
        envelope: OpaqueCompanionEnvelope
    ) async throws -> RelayEnvelopeUploadReceipt {
        let data = try StrictJSON.encode(envelope)
        var request = try makeRequest(
            endpoint,
            path: "v1/companion/mailboxes/\(mailboxID)/envelopes",
            method: "POST",
            bearer: writeCapability
        )
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = data
        let response = try await execute(request, expected: [200, 201], maximumBytes: 8 * 1024)
        let receipt = try StrictJSON.decode(
            RelayEnvelopeUploadReceipt.self,
            from: response,
            maximumBytes: 8 * 1024
        )
        guard receipt.schema == "tohseno.companion-envelope-accepted/1",
              receipt.accepted, receipt.cursor > 0
        else { throw TohsenoCompanionError.invalidEncoding("relay envelope receipt") }
        return receipt
    }

    public func fetchEnvelopes(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) async throws -> RelayMailboxPage {
        var components = URLComponents(
            url: try endpointURL(endpoint, path: "v1/companion/mailboxes/\(mailboxID)/envelopes"),
            resolvingAgainstBaseURL: false
        )!
        // A page is one bounded opaque envelope. This prevents a valid series
        // of large encrypted blobs from causing an unbounded aggregate HTTP
        // response while cursor reconciliation still drains every record.
        components.queryItems = [
            URLQueryItem(name: "cursor", value: String(cursor)),
            URLQueryItem(name: "limit", value: "1"),
        ]
        guard let url = components.url else { throw TohsenoCompanionError.transportUnavailable }
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        applyPrivateHeaders(&request, bearer: readCapability)
        let data = try await execute(request, expected: [200, 409], maximumBytes: CompanionLimits.maximumRelayResponseBytes)
        if let reset = try? StrictJSON.decode(
            RelayMailboxReset.self,
            from: data,
            maximumBytes: 8 * 1024
        ), reset.schema == "tohseno.companion-mailbox-reset-required/1", reset.resetRequired,
           reset.resetBeforeCursor <= reset.headCursor {
            throw TohsenoCompanionError.cursorResetRequired(
                resetBefore: reset.resetBeforeCursor,
                head: reset.headCursor
            )
        }
        let page = try StrictJSON.decode(
            RelayMailboxPage.self,
            from: data,
            maximumBytes: CompanionLimits.maximumRelayResponseBytes
        )
        guard page.schema == RelayMailboxPage.schemaV1 else {
            throw TohsenoCompanionError.invalidEncoding("relay mailbox page schema")
        }
        return page
    }

    public func acknowledge(
        endpoint: RelayEndpoint,
        mailboxID: String,
        acknowledgementCapability: String,
        cursor: UInt64
    ) async throws {
        var request = try makeRequest(
            endpoint,
            path: "v1/companion/mailboxes/\(mailboxID)/ack",
            method: "POST",
            bearer: acknowledgementCapability
        )
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try StrictJSON.encode(RelayAcknowledgement(cursor: cursor))
        _ = try await execute(request, expected: [200], maximumBytes: 8 * 1024)
    }

    public func liveEvents(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) async throws -> AsyncThrowingStream<RelayLiveEvent, Error> {
        try requireIdentifier(mailboxID, field: "mailbox_id")
        var components = URLComponents(
            url: try endpointURL(endpoint, path: "v1/companion/mailboxes/\(mailboxID)/live"),
            resolvingAgainstBaseURL: false
        )!
        components.queryItems = [URLQueryItem(name: "cursor", value: String(cursor))]
        guard let url = components.url else { throw TohsenoCompanionError.transportUnavailable }
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        applyPrivateHeaders(&request, bearer: readCapability)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        let liveRequest = request

        return AsyncThrowingStream { continuation in
            let task = Task { [session] in
                do {
                    let (bytes, response) = try await session.bytes(
                        for: liveRequest,
                        delegate: RedirectRejectingTaskDelegate.shared
                    )
                    guard let HTTP = response as? HTTPURLResponse else {
                        throw TohsenoCompanionError.transportUnavailable
                    }
                    guard HTTP.statusCode == 200 else {
                        if HTTP.statusCode == 401 || HTTP.statusCode == 403 || HTTP.statusCode == 410 {
                            throw TohsenoCompanionError.capabilityRevoked
                        }
                        throw TohsenoCompanionError.relayFailure(HTTP.statusCode)
                    }
                    guard HTTP.value(forHTTPHeaderField: "Content-Type")?
                        .lowercased().hasPrefix("text/event-stream") == true else {
                        throw TohsenoCompanionError.invalidEncoding("relay live content type")
                    }
                    var parser = RelayServerSentEventParser(expectedMailboxID: mailboxID)
                    for try await byte in bytes {
                        try Task.checkCancellation()
                        if let event = try parser.append(byte) { continuation.yield(event) }
                    }
                    try parser.finish()
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch let error as TohsenoCompanionError {
                    continuation.finish(throwing: error)
                } catch {
                    continuation.finish(throwing: TohsenoCompanionError.transportUnavailable)
                }
            }
            continuation.onTermination = { @Sendable _ in task.cancel() }
        }
    }

    public func registerPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String,
        token: Data
    ) async throws {
        guard (1 ... 256).contains(token.count) else {
            throw TohsenoCompanionError.invalidEncoding("APNs token size")
        }
        var request = try makeRequest(
            endpoint,
            path: "v1/companion/push/register",
            method: "POST",
            bearer: pushCapability
        )
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try StrictJSON.encode(RelayPushRegistration(
            mailboxID: mailboxID,
            deviceID: deviceID,
            APNSToken: token.map { String(format: "%02x", $0) }.joined()
        ))
        _ = try await execute(request, expected: [200, 201], maximumBytes: 8 * 1024)
    }

    public func unregisterPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String
    ) async throws {
        var request = try makeRequest(
            endpoint,
            path: "v1/companion/push/register/\(deviceID)",
            method: "DELETE",
            bearer: pushCapability
        )
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try StrictJSON.encode(RelayPushRemoval(mailboxID: mailboxID))
        _ = try await execute(request, expected: [204], maximumBytes: 1024)
    }

    private func makeRequest(
        _ endpoint: RelayEndpoint,
        path: String,
        method: String,
        bearer: String? = nil
    ) throws -> URLRequest {
        var request = URLRequest(url: try endpointURL(endpoint, path: path))
        request.httpMethod = method
        applyPrivateHeaders(&request, bearer: bearer)
        return request
    }

    private func endpointURL(_ endpoint: RelayEndpoint, path: String) throws -> URL {
        guard !path.contains(".."), !path.contains("?"), !path.contains("#"),
              let url = URL(string: path, relativeTo: endpoint.baseURL)?.absoluteURL,
              url.scheme == endpoint.baseURL.scheme, url.host == endpoint.baseURL.host,
              url.port == endpoint.baseURL.port
        else { throw TohsenoCompanionError.relayNotAllowed }
        return url
    }

    private func applyPrivateHeaders(_ request: inout URLRequest, bearer: String?) {
        request.setValue("no-store", forHTTPHeaderField: "Cache-Control")
        request.setValue("no-cache", forHTTPHeaderField: "Pragma")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let bearer { request.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization") }
    }

    private func execute(
        _ request: URLRequest,
        expected: Set<Int>,
        maximumBytes: Int
    ) async throws -> Data {
        do {
            let (bytes, response) = try await session.bytes(
                for: request,
                delegate: RedirectRejectingTaskDelegate.shared
            )
            guard let HTTP = response as? HTTPURLResponse else {
                throw TohsenoCompanionError.transportUnavailable
            }
            guard expected.contains(HTTP.statusCode) else {
                if HTTP.statusCode == 401 || HTTP.statusCode == 403 || HTTP.statusCode == 410 {
                    throw TohsenoCompanionError.capabilityRevoked
                }
                throw TohsenoCompanionError.relayFailure(HTTP.statusCode)
            }
            if response.expectedContentLength > Int64(maximumBytes) {
                throw TohsenoCompanionError.responseTooLarge
            }
            var data = Data()
            data.reserveCapacity(min(maximumBytes, 64 * 1024))
            for try await byte in bytes {
                guard data.count < maximumBytes else {
                    throw TohsenoCompanionError.responseTooLarge
                }
                data.append(byte)
            }
            return data
        } catch let error as TohsenoCompanionError {
            throw error
        } catch {
#if DEBUG
            let diagnostic = error as NSError
            NSLog(
                "TOHSENO Companion relay transport failed: domain=%@ code=%ld",
                diagnostic.domain,
                diagnostic.code
            )
#endif
            throw TohsenoCompanionError.transportUnavailable
        }
    }
}

private final class RedirectRejectingTaskDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    static let shared = RedirectRejectingTaskDelegate()

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}

private struct RelayServerSentEventParser {
    private let expectedMailboxID: String
    private var line = Data()
    private var eventName: String?
    private var eventID: String?
    private var eventData = Data()
    private var frameBytes = 0

    init(expectedMailboxID: String) { self.expectedMailboxID = expectedMailboxID }

    mutating func append(_ byte: UInt8) throws -> RelayLiveEvent? {
        frameBytes += 1
        guard frameBytes <= CompanionLimits.maximumRelayResponseBytes else {
            throw TohsenoCompanionError.responseTooLarge
        }
        if byte != 0x0a {
            guard line.count < CompanionLimits.maximumRelayResponseBytes else {
                throw TohsenoCompanionError.responseTooLarge
            }
            line.append(byte)
            return nil
        }
        if line.last == 0x0d { line.removeLast() }
        defer { line.removeAll(keepingCapacity: true) }
        guard !line.isEmpty else { return try completeFrame() }
        if line.first == 0x3a { return nil }
        guard let separator = line.firstIndex(of: 0x3a) else {
            throw TohsenoCompanionError.invalidEncoding("relay live SSE field")
        }
        guard let field = String(data: line[..<separator], encoding: .utf8) else {
            throw TohsenoCompanionError.invalidEncoding("relay live SSE field encoding")
        }
        var valueStart = line.index(after: separator)
        if valueStart < line.endIndex, line[valueStart] == 0x20 { valueStart = line.index(after: valueStart) }
        let value = line[valueStart...]
        switch field {
        case "event":
            guard let decoded = String(data: value, encoding: .utf8), !decoded.isEmpty else {
                throw TohsenoCompanionError.invalidEncoding("relay live event name")
            }
            eventName = decoded
        case "id":
            guard let decoded = String(data: value, encoding: .utf8), !decoded.isEmpty else {
                throw TohsenoCompanionError.invalidEncoding("relay live event ID")
            }
            eventID = decoded
        case "data":
            if !eventData.isEmpty { eventData.append(0x0a) }
            eventData.append(value)
        default:
            throw TohsenoCompanionError.invalidEncoding("relay live SSE field")
        }
        return nil
    }

    mutating func finish() throws {
        guard line.isEmpty, eventName == nil, eventID == nil, eventData.isEmpty else {
            throw TohsenoCompanionError.invalidEncoding("truncated relay live event")
        }
    }

    private mutating func completeFrame() throws -> RelayLiveEvent? {
        defer {
            eventName = nil
            eventID = nil
            eventData.removeAll(keepingCapacity: true)
            frameBytes = 0
        }
        guard let eventName else {
            guard eventID == nil, eventData.isEmpty else {
                throw TohsenoCompanionError.invalidEncoding("relay live unnamed event")
            }
            return nil
        }
        switch eventName {
        case "envelope":
            guard let eventID, let cursor = UInt64(eventID), cursor > 0 else {
                throw TohsenoCompanionError.invalidEncoding("relay live envelope cursor")
            }
            let envelope = try StrictJSON.decode(
                OpaqueCompanionEnvelope.self,
                from: eventData,
                maximumBytes: CompanionLimits.maximumRelayResponseBytes
            )
            guard envelope.mailboxID == expectedMailboxID else {
                throw TohsenoCompanionError.invalidEnvelope("misrouted relay live envelope")
            }
            try envelope.validateShape()
            return .envelope(RelayMailboxEnvelope(cursor: cursor, envelope: envelope))
        case "reconcile":
            guard eventID == nil,
                  try StrictJSON.decode(
                      RelayLiveReconcile.self,
                      from: eventData,
                      maximumBytes: 1024
                  ).snapshotRequired else {
                throw TohsenoCompanionError.invalidEncoding("relay live reconcile event")
            }
            return .reconcile
        case "revoked":
            let revoked = try StrictJSON.decode(RelayLiveRevocation.self, from: eventData, maximumBytes: 1024)
            guard revoked.revoked else {
                throw TohsenoCompanionError.invalidEncoding("relay live revocation event")
            }
            if let eventID {
                guard let cursor = UInt64(eventID) else {
                    throw TohsenoCompanionError.invalidEncoding("relay live revocation cursor")
                }
                return .revoked(cursor: cursor)
            }
            return .revoked(cursor: nil)
        default:
            throw TohsenoCompanionError.invalidEncoding("unsupported relay live event")
        }
    }
}

private struct RelayLiveReconcile: Codable {
    let snapshotRequired: Bool

    enum CodingKeys: String, CodingKey { case snapshotRequired = "snapshot_required" }

    init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["snapshot_required"])
        snapshotRequired = try decoder.container(keyedBy: CodingKeys.self)
            .decode(Bool.self, forKey: .snapshotRequired)
    }
}

private struct RelayLiveRevocation: Codable {
    let revoked: Bool

    private enum CodingKeys: String, CodingKey { case revoked }

    init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["revoked"])
        revoked = try decoder.container(keyedBy: CodingKeys.self).decode(Bool.self, forKey: .revoked)
    }
}

private struct RelayAcknowledgement: Codable {
    let schema: String
    let cursor: UInt64

    init(cursor: UInt64) {
        schema = "tohseno.companion-mailbox-ack/1"
        self.cursor = cursor
    }
}

private struct RelayPushRegistration: Codable {
    let schema = "tohseno.companion-push-register/1"
    let mailboxID: String
    let deviceID: String
    let APNSToken: String

    enum CodingKeys: String, CodingKey {
        case schema
        case mailboxID = "mailbox_id"
        case deviceID = "device_id"
        case APNSToken = "apns_token"
    }
}

private struct RelayPushRemoval: Codable {
    let schema = "tohseno.companion-push-unregister/1"
    let mailboxID: String

    enum CodingKeys: String, CodingKey {
        case schema
        case mailboxID = "mailbox_id"
    }
}
