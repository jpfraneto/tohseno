import Foundation
import XCTest
@testable import TohsenoCompanionKit

final class RelayTransportTests: XCTestCase {
    override func tearDown() {
        RelayTestURLProtocol.setHandler(nil)
        super.tearDown()
    }

    func testTransportRejectsRedirectWithoutContactingDestination() async throws {
        let destinationWasContacted = LockedFlag()
        RelayTestURLProtocol.setHandler { request, protocolInstance in
            if request.url?.host == "malicious.invalid" {
                destinationWasContacted.set()
                protocolInstance.fail(URLError(.cannotConnectToHost))
                return
            }
            let destination = URL(string: "https://malicious.invalid/stolen")!
            let redirect = HTTPURLResponse(
                url: request.url!,
                statusCode: 307,
                httpVersion: "HTTP/1.1",
                headerFields: ["Location": destination.absoluteString]
            )!
            protocolInstance.redirect(to: URLRequest(url: destination), response: redirect)
        }

        let transport = makeTransport()
        do {
            _ = try await transport.createMailbox(
                endpoint: endpoint(),
                verifiers: mailboxVerifiers()
            )
            XCTFail("a relay redirect must not be followed")
        } catch let error as TohsenoCompanionError {
            XCTAssertTrue(
                error == .relayFailure(307) || error == .transportUnavailable,
                "redirect refusal must surface as a closed transport, got \(error)"
            )
        }
        XCTAssertFalse(destinationWasContacted.value())
    }

    func testTransportRejectsOversizedBodyWhileStreaming() async throws {
        let delivered = LockedCounter()
        RelayTestURLProtocol.setHandler { request, protocolInstance in
            protocolInstance.respond(
                status: 201,
                headers: ["Content-Type": "application/json"]
            )
            for _ in 0 ..< 10 {
                let chunk = Data(repeating: 0x61, count: 1024)
                delivered.add(chunk.count)
                protocolInstance.deliver(chunk)
            }
            protocolInstance.finish()
        }

        let transport = makeTransport()
        do {
            _ = try await transport.createMailbox(
                endpoint: endpoint(),
                verifiers: mailboxVerifiers()
            )
            XCTFail("an oversized relay response must be rejected")
        } catch let error as TohsenoCompanionError {
            XCTAssertEqual(error, .responseTooLarge)
        }
        XCTAssertGreaterThan(delivered.value(), 8 * 1024)
    }

    func testLiveTransportParsesOneBoundedEnvelopeEvent() async throws {
        let mailboxID = String(repeating: "a", count: 32)
        let envelope = OpaqueCompanionEnvelope(
            envelopeID: "11111111-1111-4111-8111-111111111111",
            mailboxID: mailboxID,
            senderDeviceID: "studio_fixture",
            recipientDeviceID: "phone_fixture",
            senderSequence: 1,
            createdAt: "2026-08-15T12:01:00Z",
            expiresAt: "2026-08-16T12:01:00Z",
            ephemeralPublicKey: Base64URL.encode(Data(repeating: 1, count: 32)),
            nonce: Base64URL.encode(Data(repeating: 2, count: 12)),
            ciphertext: Base64URL.encode(Data(repeating: 3, count: 16)),
            signature: Base64URL.encode(Data(repeating: 4, count: 64))
        )
        let JSON = try XCTUnwrap(String(data: StrictJSON.encode(envelope), encoding: .utf8))
        RelayTestURLProtocol.setHandler { _, protocolInstance in
            protocolInstance.respond(
                status: 200,
                headers: ["Content-Type": "text/event-stream; charset=utf-8"]
            )
            protocolInstance.deliver(Data(
                "id: 1\nevent: envelope\ndata: \(JSON)\n\n".utf8
            ))
            protocolInstance.finish()
        }

        let stream = try await makeTransport().liveEvents(
            endpoint: endpoint(),
            mailboxID: mailboxID,
            readCapability: Base64URL.encode(Data(repeating: 9, count: 32)),
            after: 0
        )
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        XCTAssertEqual(event, .envelope(RelayMailboxEnvelope(cursor: 1, envelope: envelope)))
        let end = try await iterator.next()
        XCTAssertNil(end)
    }

    private func makeTransport() -> URLSessionCompanionRelayTransport {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [RelayTestURLProtocol.self]
        return URLSessionCompanionRelayTransport(session: URLSession(configuration: configuration))
    }

    private func endpoint() -> RelayEndpoint {
        try! RelayEndpoint(
            id: "official-v1",
            baseURL: URL(string: "https://relay.example")!
        )
    }

    private func mailboxVerifiers() -> RelayMailboxVerifiers {
        RelayMailboxVerifiers(
            writeVerifier: String(repeating: "1", count: 64),
            readVerifier: String(repeating: "2", count: 64),
            acknowledgementVerifier: String(repeating: "3", count: 64),
            revocationVerifier: String(repeating: "4", count: 64),
            pushVerifier: String(repeating: "5", count: 64)
        )
    }
}

private final class RelayTestURLProtocol: URLProtocol, @unchecked Sendable {
    typealias Handler = @Sendable (URLRequest, RelayTestURLProtocol) -> Void
    private static let lock = NSLock()
    nonisolated(unsafe) private static var handler: Handler?

    static func setHandler(_ value: Handler?) {
        lock.lock()
        handler = value
        lock.unlock()
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lock.lock()
        let handler = Self.handler
        Self.lock.unlock()
        guard let handler else {
            fail(URLError(.resourceUnavailable))
            return
        }
        handler(request, self)
    }

    override func stopLoading() {}

    func respond(status: Int, headers: [String: String]) {
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: status,
            httpVersion: "HTTP/1.1",
            headerFields: headers
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    }

    func deliver(_ bytes: Data) { client?.urlProtocol(self, didLoad: bytes) }
    func finish() { client?.urlProtocolDidFinishLoading(self) }
    func fail(_ error: Error) { client?.urlProtocol(self, didFailWithError: error) }

    func redirect(to request: URLRequest, response: HTTPURLResponse) {
        client?.urlProtocol(self, wasRedirectedTo: request, redirectResponse: response)
        client?.urlProtocolDidFinishLoading(self)
    }
}

private final class LockedFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var setValue = false

    func set() {
        lock.lock()
        setValue = true
        lock.unlock()
    }

    func value() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return setValue
    }
}

private final class LockedCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    func add(_ value: Int) {
        lock.lock()
        count += value
        lock.unlock()
    }

    func value() -> Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }
}
