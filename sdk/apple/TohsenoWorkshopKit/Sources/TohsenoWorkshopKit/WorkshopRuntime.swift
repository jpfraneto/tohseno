import Foundation
import Network
import Observation

public protocol WorkshopSession: AnyObject, Sendable {
    var id: WorkshopSessionID? { get async }
    var devices: [WorkshopDevice] { get async }
    var events: AsyncStream<WorkshopEnvelope> { get }
    func send(_ event: WorkshopEvent) async throws
}

/// The intentionally tiny Shot-facing boundary. Standalone Shot enrollment is
/// not implicit: a host product or an explicitly authorized Shot must install
/// a real Session before `join()` succeeds.
public actor TohsenoWorkshop {
    public static let current = TohsenoWorkshop()

    private weak var activeSession: (any WorkshopSession)?

    public init() {}

    public var devices: [WorkshopDevice] {
        get async { await activeSession?.devices ?? [] }
    }

    public func join() throws -> any WorkshopSession {
        guard let activeSession else { throw WorkshopRuntimeError.transportUnavailable }
        return activeSession
    }

    public func use(session: any WorkshopSession) {
        activeSession = session
    }
}

@MainActor
@Observable
public final class WorkshopHostRuntime: WorkshopSession, @unchecked Sendable {
    public static let serviceType = "_tohseno-ws._tcp"

    public private(set) var connectionState: WorkshopConnectionState = .unavailable
    public private(set) var peerName: String?
    public private(set) var lastEventAt: Date?
    public private(set) var lastRoundTrip: TimeInterval?
    public private(set) var rejectionReason: String?
    public private(set) var currentSessionID: WorkshopSessionID?

    public nonisolated let events: AsyncStream<WorkshopEnvelope>

    private let authorizer: any WorkshopHostAuthorizing
    private let localDeviceName: String
    private var intelligenceReady: Bool
    private let queue = DispatchQueue(label: "com.tohseno.workshop.host")
    private let eventContinuation: AsyncStream<WorkshopEnvelope>.Continuation
    private var listener: NWListener?
    private var authorization: WorkshopHostAuthorization?
    private var channel: WorkshopWireChannel?
    private var activePeer: WorkshopTrustedPeer?
    private var remoteDevice: WorkshopDevice?
    private var sendSequence: UInt64 = 0
    private var receiveSequence: UInt64 = 0
    private var pendingPulses: [String: Date] = [:]

    public init(
        authorizer: any WorkshopHostAuthorizing,
        localDeviceName: String,
        intelligenceReady: Bool = false
    ) {
        self.authorizer = authorizer
        self.localDeviceName = localDeviceName
        self.intelligenceReady = intelligenceReady
        let stream = AsyncStream<WorkshopEnvelope>.makeStream(bufferingPolicy: .bufferingNewest(64))
        events = stream.stream
        eventContinuation = stream.continuation
    }

    deinit {
        eventContinuation.finish()
    }

    public var id: WorkshopSessionID? { get async { await MainActor.run { currentSessionID } } }

    public var devices: [WorkshopDevice] {
        get async {
            await MainActor.run {
                guard let studioID = authorization?.credential.studioDeviceID else { return [] }
                var values = [WorkshopDevice.mac(
                    id: studioID,
                    name: localDeviceName,
                    intelligenceReady: intelligenceReady
                )]
                if let peer = activePeer {
                    values.append(remoteDevice ?? .companion(
                        id: peer.deviceID,
                        name: peer.displayName,
                        connected: connectionState == .connected
                    ))
                }
                return values
            }
        }
    }

    public func start() async {
        stop()
        connectionState = .discovering
        rejectionReason = nil
        guard let sessionID = WorkshopSessionID(
            rawValue: "workshop_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
        ) else {
            connectionState = .rejected
            rejectionReason = WorkshopRuntimeError.invalidIdentity.localizedDescription
            return
        }
        let challenge = randomBytes(count: 32)
        do {
            let authorization = try await authorizer.authorizeWorkshopHost(
                sessionID: sessionID,
                challenge: challenge
            )
            guard authorization.credential.sessionID == sessionID,
                  authorization.credential.challenge == WorkshopBase64URL.encode(challenge)
            else {
                throw WorkshopRuntimeError.invalidCredential
            }
            self.authorization = authorization
            currentSessionID = sessionID
            guard authorization.peers.count == 1 else {
                connectionState = .unavailable
                rejectionReason = "Pair one intended iPhone before starting a live Workshop Session."
                return
            }
            let listener = try NWListener(using: .tcp, on: .any)
            listener.service = .init(type: Self.serviceType)
            listener.stateUpdateHandler = { [weak self] state in
                Task { @MainActor [weak self] in self?.apply(listenerState: state) }
            }
            listener.newConnectionHandler = { [weak self] connection in
                Task { @MainActor [weak self] in self?.accept(connection) }
            }
            self.listener = listener
            listener.start(queue: queue)
        } catch {
            connectionState = .unavailable
            rejectionReason = error.localizedDescription
        }
    }

    public func setIntelligenceReady(_ ready: Bool) {
        guard intelligenceReady != ready else { return }
        intelligenceReady = ready
        publishLocalDeviceSnapshotIfConnected()
    }

    public func stop() {
        listener?.cancel()
        listener = nil
        channel?.cancel()
        channel = nil
        authorization = nil
        activePeer = nil
        remoteDevice = nil
        currentSessionID = nil
        connectionState = .unavailable
        peerName = nil
        sendSequence = 0
        receiveSequence = 0
        pendingPulses.removeAll()
    }

    public func send(_ event: WorkshopEvent) async throws {
        guard event.isApplicationEvent else { throw WorkshopRuntimeError.invalidEvent }
        try await sendEvent(event)
    }

    public func sendPulse() async throws {
        let pulseID = UUID().uuidString.lowercased()
        let sentAt = Date()
        let payload = try JSONEncoder().encode(WorkshopPulsePayload(
            pulseID: pulseID,
            sentAt: WorkshopTimestamp.format(sentAt)
        ))
        pendingPulses[pulseID] = sentAt
        try await sendEvent(WorkshopEvent(type: "workshop.pulse", payload: payload))
    }

    private func accept(_ connection: NWConnection) {
        guard channel == nil, let authorization else {
            connection.cancel()
            return
        }
        connectionState = .authenticating
        let candidate = WorkshopWireChannel(connection: connection, queue: queue)
        candidate.onPacket = { [weak self, weak candidate] data in
            Task { @MainActor [weak self, weak candidate] in
                guard let self, let candidate else { return }
                await self.receiveHandshakeOrEvent(data, from: candidate)
            }
        }
        candidate.onState = { [weak self, weak candidate] state in
            Task { @MainActor [weak self, weak candidate] in
                guard let self, let candidate else { return }
                self.apply(channelState: state, channel: candidate)
            }
        }
        channel = candidate
        candidate.start()
        try? candidate.send(WorkshopWirePacket(kind: .hostCredential, value: authorization.credential))
    }

    private func receiveHandshakeOrEvent(_ data: Data, from candidate: WorkshopWireChannel) async {
        guard candidate === channel, let authorization else { return }
        do {
            let packet = try WorkshopWirePacket.decode(data)
            if activePeer == nil {
                guard packet.kind == .clientProof else { throw WorkshopRuntimeError.invalidCredential }
                let proof = try packet.value(WorkshopClientProof.self)
                let peer = try WorkshopHandshake.authenticate(
                    proof: proof,
                    host: authorization.credential,
                    peers: authorization.peers
                )
                activePeer = peer
                peerName = peer.displayName
                connectionState = .connected
                try await sendEvent(WorkshopEvent(type: "workshop.session.ready"))
                try await sendLocalDeviceSnapshot()
                return
            }
            guard packet.kind == .sealedEvent,
                  let peer = activePeer,
                  let sessionID = currentSessionID
            else { throw WorkshopRuntimeError.invalidCredential }
            let sealed = try packet.value(WorkshopSealedEvent.self)
            let envelope = try WorkshopSessionCrypto.open(
                WorkshopBase64URL.decode(sealed.combined),
                sessionKey: WorkshopBase64URL.decode(peer.sessionKey, expectedBytes: 32),
                direction: "companion-to-mac",
                expectedSessionID: sessionID,
                afterSequence: receiveSequence
            )
            guard envelope.senderDeviceID == peer.deviceID else {
                throw WorkshopRuntimeError.invalidIdentity
            }
            receiveSequence = envelope.sequence
            await accept(envelope)
        } catch {
            rejectionReason = error.localizedDescription
            connectionState = .rejected
            candidate.cancel()
        }
    }

    private func accept(_ envelope: WorkshopEnvelope) async {
        lastEventAt = Date()
        eventContinuation.yield(envelope)
        if envelope.event.type == "workshop.device.snapshot",
           let peer = activePeer,
           let device = try? JSONDecoder().decode(WorkshopDevice.self, from: envelope.event.payload),
           device.id == peer.deviceID,
           device.platform == .iPhone,
           device.connection == .connected {
            remoteDevice = device
        } else if envelope.event.type == "workshop.pulse" {
            let pulse = try? JSONDecoder().decode(WorkshopPulsePayload.self, from: envelope.event.payload)
            if let pulse {
                let payload = try? JSONEncoder().encode(pulse)
                if let payload {
                    try? await sendEvent(WorkshopEvent(type: "workshop.pulse.reply", payload: payload))
                }
            }
        } else if envelope.event.type == "workshop.pulse.reply",
                  let pulse = try? JSONDecoder().decode(WorkshopPulsePayload.self, from: envelope.event.payload),
                  let started = pendingPulses.removeValue(forKey: pulse.pulseID) {
            lastRoundTrip = Date().timeIntervalSince(started)
        }
    }

    private func sendEvent(_ event: WorkshopEvent) async throws {
        guard connectionState == .connected,
              let channel,
              let peer = activePeer,
              let sessionID = currentSessionID,
              let authorization
        else { throw WorkshopRuntimeError.disconnected }
        sendSequence = try nextSequence(sendSequence)
        let envelope = WorkshopEnvelope(
            sessionID: sessionID,
            senderDeviceID: authorization.credential.studioDeviceID,
            sequence: sendSequence,
            event: event
        )
        let combined = try WorkshopSessionCrypto.seal(
            envelope,
            sessionKey: WorkshopBase64URL.decode(peer.sessionKey, expectedBytes: 32),
            direction: "mac-to-companion"
        )
        try channel.send(WorkshopWirePacket(
            kind: .sealedEvent,
            value: WorkshopSealedEvent(combined: WorkshopBase64URL.encode(combined))
        ))
    }

    private func sendLocalDeviceSnapshot() async throws {
        guard let id = authorization?.credential.studioDeviceID else {
            throw WorkshopRuntimeError.invalidIdentity
        }
        let device = WorkshopDevice.mac(
            id: id,
            name: localDeviceName,
            intelligenceReady: intelligenceReady
        )
        try await sendEvent(WorkshopEvent(
            type: "workshop.device.snapshot",
            payload: try JSONEncoder().encode(device)
        ))
    }

    private func publishLocalDeviceSnapshotIfConnected() {
        guard connectionState == .connected else { return }
        Task { @MainActor [weak self] in
            try? await self?.sendLocalDeviceSnapshot()
        }
    }

    private func apply(listenerState state: NWListener.State) {
        switch state {
        case .ready:
            if channel == nil { connectionState = .discovering }
        case .failed, .cancelled:
            if listener != nil { connectionState = .unavailable }
        default:
            break
        }
    }

    private func apply(channelState state: NWConnection.State, channel candidate: WorkshopWireChannel) {
        guard candidate === channel else { return }
        switch state {
        case .failed, .cancelled:
            channel = nil
            activePeer = nil
            remoteDevice = nil
            peerName = nil
            receiveSequence = 0
            sendSequence = 0
            if connectionState != .rejected {
                connectionState = WorkshopConnectionFlow.afterTransportLoss(
                    recoveryAvailable: listener != nil
                )
            }
        default:
            break
        }
    }
}

@MainActor
@Observable
public final class WorkshopClientRuntime: WorkshopSession, @unchecked Sendable {
    public private(set) var connectionState: WorkshopConnectionState = .unavailable
    public private(set) var peerName: String?
    public private(set) var lastEventAt: Date?
    public private(set) var lastRoundTrip: TimeInterval?
    public private(set) var rejectionReason: String?
    public private(set) var currentSessionID: WorkshopSessionID?

    public nonisolated let events: AsyncStream<WorkshopEnvelope>

    private let authorizer: any WorkshopClientAuthorizing
    private let localDeviceName: String
    private let queue = DispatchQueue(label: "com.tohseno.workshop.client")
    private let eventContinuation: AsyncStream<WorkshopEnvelope>.Continuation
    private var browser: NWBrowser?
    private var channel: WorkshopWireChannel?
    private var host: WorkshopHostCredential?
    private var pairing: WorkshopClientPairing?
    private var sessionKey: Data?
    private var sendSequence: UInt64 = 0
    private var receiveSequence: UInt64 = 0
    private var pendingPulses: [String: Date] = [:]
    private var localCameraPermission: WorkshopPermission = .notRequested
    private var localMicrophonePermission: WorkshopPermission = .notRequested
    private var localMotionPermission: WorkshopPermission = .notRequested
    private var remoteDevice: WorkshopDevice?
    private var lastEndpoint: NWEndpoint?

    public init(authorizer: any WorkshopClientAuthorizing, localDeviceName: String) {
        self.authorizer = authorizer
        self.localDeviceName = localDeviceName
        let stream = AsyncStream<WorkshopEnvelope>.makeStream(bufferingPolicy: .bufferingNewest(64))
        events = stream.stream
        eventContinuation = stream.continuation
    }

    deinit {
        eventContinuation.finish()
    }

    public var id: WorkshopSessionID? { get async { await MainActor.run { currentSessionID } } }

    public var devices: [WorkshopDevice] {
        get async {
            await MainActor.run {
                guard let pairing else { return [] }
                return [
                    remoteDevice ?? .mac(
                        id: pairing.studioDeviceID,
                        name: peerName ?? "This Mac",
                        connected: connectionState == .connected,
                        intelligenceReady: false
                    ),
                    .companion(
                        id: pairing.companionDeviceID,
                        name: localDeviceName,
                        connected: connectionState == .connected,
                        cameraPermission: localCameraPermission,
                        microphonePermission: localMicrophonePermission,
                        motionPermission: localMotionPermission
                    ),
                ]
            }
        }
    }

    public func start() async {
        stop()
        do {
            pairing = try await authorizer.workshopPairing()
            connectionState = .discovering
            let browser = NWBrowser(
                for: .bonjour(type: WorkshopHostRuntime.serviceType, domain: nil),
                using: .tcp
            )
            browser.stateUpdateHandler = { [weak self] state in
                Task { @MainActor [weak self] in self?.apply(browserState: state) }
            }
            browser.browseResultsChangedHandler = { [weak self] results, _ in
                guard let endpoint = results.first?.endpoint else { return }
                Task { @MainActor [weak self] in self?.connect(endpoint) }
            }
            self.browser = browser
            browser.start(queue: queue)
        } catch {
            connectionState = error as? WorkshopRuntimeError == .revokedDevice ? .rejected : .unavailable
            rejectionReason = error.localizedDescription
        }
    }

    public func setLocalPermissions(
        camera: WorkshopPermission,
        microphone: WorkshopPermission,
        motion: WorkshopPermission
    ) {
        guard localCameraPermission != camera
                || localMicrophonePermission != microphone
                || localMotionPermission != motion
        else { return }
        localCameraPermission = camera
        localMicrophonePermission = microphone
        localMotionPermission = motion
        publishLocalDeviceSnapshotIfConnected()
    }

    public func stop() {
        browser?.cancel()
        browser = nil
        channel?.cancel()
        channel = nil
        host = nil
        pairing = nil
        sessionKey = nil
        remoteDevice = nil
        lastEndpoint = nil
        currentSessionID = nil
        connectionState = .unavailable
        peerName = nil
        sendSequence = 0
        receiveSequence = 0
        pendingPulses.removeAll()
    }

    public func send(_ event: WorkshopEvent) async throws {
        guard event.isApplicationEvent else { throw WorkshopRuntimeError.invalidEvent }
        try await sendEvent(event)
    }

    public func sendPulse() async throws {
        let pulseID = UUID().uuidString.lowercased()
        let sentAt = Date()
        pendingPulses[pulseID] = sentAt
        let payload = try JSONEncoder().encode(WorkshopPulsePayload(
            pulseID: pulseID,
            sentAt: WorkshopTimestamp.format(sentAt)
        ))
        try await sendEvent(WorkshopEvent(type: "workshop.pulse", payload: payload))
    }

    private func connect(_ endpoint: NWEndpoint) {
        guard channel == nil else { return }
        lastEndpoint = endpoint
        connectionState = .authenticating
        let candidate = WorkshopWireChannel(
            connection: NWConnection(to: endpoint, using: .tcp),
            queue: queue
        )
        candidate.onPacket = { [weak self, weak candidate] data in
            Task { @MainActor [weak self, weak candidate] in
                guard let self, let candidate else { return }
                await self.receiveHandshakeOrEvent(data, from: candidate)
            }
        }
        candidate.onState = { [weak self, weak candidate] state in
            Task { @MainActor [weak self, weak candidate] in
                guard let self, let candidate else { return }
                self.apply(channelState: state, channel: candidate)
            }
        }
        channel = candidate
        candidate.start()
    }

    private func receiveHandshakeOrEvent(_ data: Data, from candidate: WorkshopWireChannel) async {
        guard candidate === channel, let pairing else { return }
        do {
            let packet = try WorkshopWirePacket.decode(data)
            if host == nil {
                guard packet.kind == .hostCredential else { throw WorkshopRuntimeError.invalidCredential }
                let host = try packet.value(WorkshopHostCredential.self)
                try host.verify(
                    studioSigningPublicKey: pairing.studioSigningPublicKey,
                    expectedWorkspaceID: pairing.workspaceID,
                    expectedStudioDeviceID: pairing.studioDeviceID
                )
                let authorization = try await authorizer.authorizeWorkshopClient(
                    host: host,
                    clientNonce: randomBytes(count: 32)
                )
                self.host = host
                sessionKey = authorization.sessionKey
                currentSessionID = host.sessionID
                try candidate.send(WorkshopWirePacket(kind: .clientProof, value: authorization.proof))
                return
            }
            guard packet.kind == .sealedEvent,
                  let sessionKey,
                  let sessionID = currentSessionID
            else { throw WorkshopRuntimeError.invalidCredential }
            let sealed = try packet.value(WorkshopSealedEvent.self)
            let envelope = try WorkshopSessionCrypto.open(
                WorkshopBase64URL.decode(sealed.combined),
                sessionKey: sessionKey,
                direction: "mac-to-companion",
                expectedSessionID: sessionID,
                afterSequence: receiveSequence
            )
            guard envelope.senderDeviceID == pairing.studioDeviceID else {
                throw WorkshopRuntimeError.invalidIdentity
            }
            receiveSequence = envelope.sequence
            if envelope.event.type == "workshop.session.ready" {
                connectionState = .connected
                peerName = "Mac workshop"
                try await sendLocalDeviceSnapshot()
            }
            await accept(envelope)
        } catch {
            rejectionReason = error.localizedDescription
            connectionState = .rejected
            candidate.cancel()
        }
    }

    private func accept(_ envelope: WorkshopEnvelope) async {
        lastEventAt = Date()
        eventContinuation.yield(envelope)
        if envelope.event.type == "workshop.device.snapshot",
           let pairing,
           let device = try? JSONDecoder().decode(WorkshopDevice.self, from: envelope.event.payload),
           device.id == pairing.studioDeviceID,
           device.platform == .macOS,
           device.connection == .connected {
            remoteDevice = device
        } else if envelope.event.type == "workshop.pulse" {
            if let pulse = try? JSONDecoder().decode(WorkshopPulsePayload.self, from: envelope.event.payload),
               let payload = try? JSONEncoder().encode(pulse) {
                try? await sendEvent(WorkshopEvent(type: "workshop.pulse.reply", payload: payload))
            }
        } else if envelope.event.type == "workshop.pulse.reply",
                  let pulse = try? JSONDecoder().decode(WorkshopPulsePayload.self, from: envelope.event.payload),
                  let started = pendingPulses.removeValue(forKey: pulse.pulseID) {
            lastRoundTrip = Date().timeIntervalSince(started)
        }
    }

    private func sendEvent(_ event: WorkshopEvent) async throws {
        guard connectionState == .connected,
              let channel,
              let sessionKey,
              let sessionID = currentSessionID,
              let pairing
        else { throw WorkshopRuntimeError.disconnected }
        sendSequence = try nextSequence(sendSequence)
        let envelope = WorkshopEnvelope(
            sessionID: sessionID,
            senderDeviceID: pairing.companionDeviceID,
            sequence: sendSequence,
            event: event
        )
        let combined = try WorkshopSessionCrypto.seal(
            envelope,
            sessionKey: sessionKey,
            direction: "companion-to-mac"
        )
        try channel.send(WorkshopWirePacket(
            kind: .sealedEvent,
            value: WorkshopSealedEvent(combined: WorkshopBase64URL.encode(combined))
        ))
    }

    private func sendLocalDeviceSnapshot() async throws {
        guard let pairing else { throw WorkshopRuntimeError.invalidIdentity }
        let device = WorkshopDevice.companion(
            id: pairing.companionDeviceID,
            name: localDeviceName,
            connected: true,
            cameraPermission: localCameraPermission,
            microphonePermission: localMicrophonePermission,
            motionPermission: localMotionPermission
        )
        try await sendEvent(WorkshopEvent(
            type: "workshop.device.snapshot",
            payload: try JSONEncoder().encode(device)
        ))
    }

    private func publishLocalDeviceSnapshotIfConnected() {
        guard connectionState == .connected else { return }
        Task { @MainActor [weak self] in
            try? await self?.sendLocalDeviceSnapshot()
        }
    }

    private func apply(browserState state: NWBrowser.State) {
        switch state {
        case .failed, .cancelled:
            if browser != nil { connectionState = .unavailable }
        case .ready:
            if channel == nil { connectionState = .discovering }
        default:
            break
        }
    }

    private func apply(channelState state: NWConnection.State, channel candidate: WorkshopWireChannel) {
        guard candidate === channel else { return }
        switch state {
        case .failed, .cancelled:
            channel = nil
            host = nil
            sessionKey = nil
            remoteDevice = nil
            currentSessionID = nil
            peerName = nil
            receiveSequence = 0
            sendSequence = 0
            if connectionState != .rejected {
                let canRecover = browser != nil
                connectionState = WorkshopConnectionFlow.afterTransportLoss(
                    recoveryAvailable: canRecover
                )
                if canRecover, let endpoint = lastEndpoint {
                    Task { @MainActor [weak self] in
                        try? await Task.sleep(for: .seconds(1))
                        guard let self, self.channel == nil, self.browser != nil else { return }
                        self.connect(endpoint)
                    }
                }
            }
        default:
            break
        }
    }
}

public extension WorkshopDevice {
    static func mac(
        id: WorkshopDeviceID,
        name: String,
        connected: Bool = true,
        intelligenceReady: Bool
    ) -> WorkshopDevice {
        let state: (WorkshopCapability) -> WorkshopCapabilityState = { capability in
            let ready = capability != .intelligence || intelligenceReady
            return WorkshopCapabilityState(
                capability: capability,
                hardware: ready ? .available : .unavailable,
                reachable: connected,
                authorized: connected
            )
        }
        return WorkshopDevice(
            id: id,
            name: name,
            platform: .macOS,
            connection: connected ? .connected : .unavailable,
            capabilities: [.display, .keyboard, .filesystem, .compute, .intelligence].map(state)
        )
    }

    static func companion(
        id: WorkshopDeviceID,
        name: String,
        connected: Bool,
        cameraPermission: WorkshopPermission = .notRequested,
        microphonePermission: WorkshopPermission = .notRequested,
        motionPermission: WorkshopPermission = .notRequested
    ) -> WorkshopDevice {
        let permission: (WorkshopCapability) -> WorkshopPermission = { capability in
            switch capability {
            case .camera: cameraPermission
            case .microphone: microphonePermission
            case .motion: motionPermission
            default: .notApplicable
            }
        }
        return WorkshopDevice(
            id: id,
            name: name,
            platform: .iPhone,
            connection: connected ? .connected : .unavailable,
            capabilities: [
                .display, .touch, .camera, .microphone, .motion,
            ].map { capability in
                WorkshopCapabilityState(
                    capability: capability,
                    hardware: .available,
                    permission: permission(capability),
                    reachable: connected,
                    authorized: connected
                )
            }
        )
    }
}

private struct WorkshopPulsePayload: Codable {
    let pulseID: String
    let sentAt: String
}

private enum WorkshopWireKind: String, Codable {
    case hostCredential = "host_credential"
    case clientProof = "client_proof"
    case sealedEvent = "sealed_event"
}

private struct WorkshopSealedEvent: Codable {
    let combined: String
}

private struct WorkshopWirePacket: Codable {
    static let schemaV1 = "tohseno.workshop-wire/1"
    let schema: String
    let kind: WorkshopWireKind
    let payload: Data

    init<Value: Encodable>(kind: WorkshopWireKind, value: Value) throws {
        schema = Self.schemaV1
        self.kind = kind
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        payload = try encoder.encode(value)
    }

    func value<Value: Decodable>(_ type: Value.Type) throws -> Value {
        guard schema == Self.schemaV1 else { throw WorkshopRuntimeError.unsupportedEnvelope }
        return try JSONDecoder().decode(type, from: payload)
    }

    static func decode(_ data: Data) throws -> WorkshopWirePacket {
        guard data.count <= WorkshopEvent.maximumPayloadBytes + 128 * 1024 else {
            throw WorkshopRuntimeError.unsupportedEnvelope
        }
        let value = try JSONDecoder().decode(Self.self, from: data)
        guard value.schema == schemaV1 else { throw WorkshopRuntimeError.unsupportedEnvelope }
        return value
    }
}

private final class WorkshopWireChannel: @unchecked Sendable {
    let connection: NWConnection
    let queue: DispatchQueue
    var onPacket: (@Sendable (Data) -> Void)?
    var onState: (@Sendable (NWConnection.State) -> Void)?
    private var buffer = Data()

    init(connection: NWConnection, queue: DispatchQueue) {
        self.connection = connection
        self.queue = queue
    }

    func start() {
        connection.stateUpdateHandler = { [weak self] state in self?.onState?(state) }
        connection.start(queue: queue)
        receive()
    }

    func cancel() { connection.cancel() }

    func send<Value: Encodable>(_ packet: Value) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let payload = try encoder.encode(packet)
        guard payload.count <= WorkshopEvent.maximumPayloadBytes + 128 * 1024,
              let length = UInt32(exactly: payload.count)
        else { throw WorkshopRuntimeError.unsupportedEnvelope }
        var bigEndian = length.bigEndian
        var frame = Data(bytes: &bigEndian, count: MemoryLayout<UInt32>.size)
        frame.append(payload)
        connection.send(content: frame, completion: .contentProcessed { _ in })
    }

    private func receive() {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, complete, error in
            guard let self else { return }
            if let data { buffer.append(data) }
            do { try drain() }
            catch { connection.cancel(); return }
            if complete || error != nil { connection.cancel(); return }
            receive()
        }
    }

    private func drain() throws {
        while buffer.count >= 4 {
            let length = buffer.prefix(4).reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
            guard length > 0, length <= WorkshopEvent.maximumPayloadBytes + 128 * 1024 else {
                throw WorkshopRuntimeError.unsupportedEnvelope
            }
            let frameLength = 4 + Int(length)
            guard buffer.count >= frameLength else { return }
            let payload = Data(buffer[4 ..< frameLength])
            buffer.removeFirst(frameLength)
            onPacket?(payload)
        }
    }
}

private func randomBytes(count: Int) -> Data {
    var generator = SystemRandomNumberGenerator()
    return Data((0 ..< count).map { _ in UInt8.random(in: .min ... .max, using: &generator) })
}

private func nextSequence(_ current: UInt64) throws -> UInt64 {
    guard current < UInt64.max else { throw WorkshopRuntimeError.replayedEnvelope }
    return current + 1
}
