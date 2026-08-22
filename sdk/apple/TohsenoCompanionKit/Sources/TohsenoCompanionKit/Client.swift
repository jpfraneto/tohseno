import Foundation

public enum CompanionConnectionState: Equatable, Sendable {
    case disconnected
    case pairing
    case connected
    case reconnecting
    case revoked
}

public protocol CompanionPushTokenProvider: Sendable {
    func currentAPNSToken() async throws -> Data?
}

public struct FeedbackRequest: Equatable, Sendable {
    public let commandID: String
    public let shotID: String
    public let expressionID: String
    public let versionID: String
    public let versionOrdinal: UInt64
    public let body: String

    public init(
        commandID: String = UUID().uuidString.lowercased(),
        shotID: String,
        expressionID: String,
        versionID: String,
        versionOrdinal: UInt64,
        body: String
    ) {
        self.commandID = commandID
        self.shotID = shotID
        self.expressionID = expressionID
        self.versionID = versionID
        self.versionOrdinal = versionOrdinal
        self.body = body
    }
}

public struct MarketingRequest: Equatable, Sendable {
    public let commandID: String
    public let noteID: String
    public let shotID: String
    public let body: String

    public init(
        commandID: String = UUID().uuidString.lowercased(),
        noteID: String = UUID().uuidString.lowercased(),
        shotID: String,
        body: String
    ) {
        self.commandID = commandID
        self.noteID = noteID
        self.shotID = shotID
        self.body = body
    }
}

public struct EvolutionRequest: Equatable, Sendable {
    public let commandID: String
    public let shotID: String
    public let baseExpressionID: String
    public let baseVersionID: String
    public let baseVersionOrdinal: UInt64
    public let intention: String
    public let selectedFeedbackActionCommitments: [String]
    public let references: [CompanionReferenceBlob]

    public init(
        commandID: String = UUID().uuidString.lowercased(),
        shotID: String,
        baseExpressionID: String,
        baseVersionID: String,
        baseVersionOrdinal: UInt64,
        intention: String,
        selectedFeedbackActionCommitments: [String] = [],
        references: [CompanionReferenceBlob] = []
    ) {
        self.commandID = commandID
        self.shotID = shotID
        self.baseExpressionID = baseExpressionID
        self.baseVersionID = baseVersionID
        self.baseVersionOrdinal = baseVersionOrdinal
        self.intention = intention
        self.selectedFeedbackActionCommitments = selectedFeedbackActionCommitments
        self.references = references
    }
}

public struct CreateShotRequest: Equatable, Sendable {
    public let commandID: String
    public let suggestedName: String?
    public let intention: String
    public let references: [CompanionReferenceBlob]

    public init(
        commandID: String = UUID().uuidString.lowercased(),
        suggestedName: String? = nil,
        intention: String,
        references: [CompanionReferenceBlob] = []
    ) {
        self.commandID = commandID
        self.suggestedName = suggestedName
        self.intention = intention
        self.references = references
    }
}

public actor TohsenoCompanionClient {
    public nonisolated let connectionState: AsyncStream<CompanionConnectionState>
    public nonisolated let workspaceEvents: AsyncStream<WorkspaceEvent>

    private let connectionContinuation: AsyncStream<CompanionConnectionState>.Continuation
    private let eventContinuation: AsyncStream<WorkspaceEvent>.Continuation
    private let identityManager: CompanionIdentityManager
    private let stateStore: any CompanionStateStore
    private let payloadStore: any CompanionPayloadStore
    private let relay: any CompanionRelayTransport
    private let allowlist: RelayAllowlist
    private let entropy: any CompanionEntropySource
    private let now: @Sendable () -> Date
    private let synchronizationSleep: @Sendable (UInt64) async throws -> Void
    private var state: CompanionPersistentState?
    private var foregroundSynchronizationTask: Task<Void, Never>?
    private var foregroundSynchronizationGeneration: UInt64 = 0
    private var reconciliationInProgress = false
    private var reconciliationWaiters: [CheckedContinuation<Void, Never>] = []

    public init(
        identityStore: any CompanionSecretStore = KeychainCompanionSecretStore(),
        stateStore: any CompanionStateStore,
        payloadStore: any CompanionPayloadStore,
        relay: any CompanionRelayTransport = URLSessionCompanionRelayTransport(),
        relayAllowlist: RelayAllowlist,
        entropySource: any CompanionEntropySource = SystemCompanionEntropySource(),
        now: @escaping @Sendable () -> Date = Date.init,
        synchronizationSleep: @escaping @Sendable (UInt64) async throws -> Void = { nanoseconds in
            try await Task.sleep(nanoseconds: nanoseconds)
        }
    ) {
        identityManager = CompanionIdentityManager(store: identityStore, entropySource: entropySource)
        self.stateStore = stateStore
        self.payloadStore = payloadStore
        self.relay = relay
        allowlist = relayAllowlist
        entropy = entropySource
        self.now = now
        self.synchronizationSleep = synchronizationSleep
        (connectionState, connectionContinuation) = AsyncStream.makeStream(
            of: CompanionConnectionState.self,
            bufferingPolicy: .bufferingNewest(16)
        )
        (workspaceEvents, eventContinuation) = AsyncStream.makeStream(
            of: WorkspaceEvent.self,
            bufferingPolicy: .bufferingNewest(512)
        )
        connectionContinuation.yield(.disconnected)
    }

    deinit {
        foregroundSynchronizationTask?.cancel()
        connectionContinuation.finish()
        eventContinuation.finish()
    }

    public func createIdentity() async throws -> RecoveryPhrase {
        let phrase = try await identityManager.createIdentity()
        try await stateStore.delete()
        try await payloadStore.deleteAll()
        state = CompanionPersistentState()
        return phrase
    }

    public func restoreIdentity(from phrase: RecoveryPhrase) async throws {
        await stopForegroundSynchronization()
        try await identityManager.restoreIdentity(from: phrase)
        // Restoring a phrase never silently restores a workspace capability.
        try await stateStore.delete()
        try await payloadStore.deleteAll()
        state = CompanionPersistentState()
        connectionContinuation.yield(.disconnected)
    }

    public func publicIdentity() async throws -> CompanionIdentityDescription {
        try await identityManager.publicIdentity()
    }

    public func pair(with invitationURI: String, displayName: String) async throws {
        await stopForegroundSynchronization()
        connectionContinuation.yield(.pairing)
        let (invitation, endpoint) = try PairingInvitation.parse(
            uri: invitationURI,
            allowlist: allowlist,
            now: now()
        )
        let identity = try await identityManager.identity()
        let secrets = try PairingRelaySecrets.generate(entropy: entropy)
        let mailbox = try await relay.createMailbox(endpoint: endpoint, verifiers: secrets.verifiers())
        let proof = try PairingProof.create(
            invitation: invitation,
            identity: identity,
            displayName: displayName,
            createdAt: CompanionTimestamp.format(now())
        )
        let response = try PairingResponseCrypto.seal(
            proof: proof,
            invitation: invitation,
            responseMailboxID: mailbox.mailboxID,
            responseMailboxWriteCapability: secrets.write,
            responseMailboxRevocationCapability: secrets.revocation,
            entropy: entropy
        )
        try await relay.submitPairingResponse(
            endpoint: endpoint,
            sessionID: invitation.sessionID,
            opaqueResponse: response
        )

        let replay = try CompanionReplayProtection(capacity: 4096)
        var cursor: UInt64 = 0
        let deadline = try CompanionTimestamp.parse(invitation.expiresAt).addingTimeInterval(30)
        var delay: UInt64 = 200_000_000
        while now() <= deadline {
            let page = try await relay.fetchEnvelopes(
                endpoint: endpoint,
                mailboxID: mailbox.mailboxID,
                readCapability: secrets.read,
                after: cursor
            )
            try page.validateRouting(mailboxID: mailbox.mailboxID, afterCursor: cursor)
            for item in page.envelopes {
                let plaintext = try await CompanionEnvelopeCrypto.open(
                    item.envelope,
                    expectedSenderSigningPublicKey: Base64URL.decode(
                        invitation.studioSigningPublicKey,
                        expectedBytes: 32
                    ),
                    expectedSenderDeviceID: invitation.studioDeviceID,
                    expectedMailboxID: mailbox.mailboxID,
                    recipient: identity,
                    now: now(),
                    replay: replay
                )
                let package = try StrictJSON.decode(
                    CompanionPairingGrantPackage.self,
                    from: plaintext,
                    maximumBytes: 256 * 1024
                )
                try validate(
                    package,
                    invitation: invitation,
                    identity: identity,
                    trustedStudioKey: Base64URL.decode(
                        invitation.studioSigningPublicKey,
                        expectedBytes: 32
                    )
                )
                let record = CompanionPairingRecord(
                    relayID: endpoint.id,
                    relayBaseURL: endpoint.baseURL.absoluteString,
                    studioDeviceID: invitation.studioDeviceID,
                    studioSigningPublicKey: invitation.studioSigningPublicKey,
                    studioAgreementPublicKey: package.studioAgreementPublicKey,
                    grant: package.capabilityGrant,
                    inbox: secrets.inbox(mailboxID: mailbox.mailboxID),
                    outbox: CompanionOutboxAccess(
                        mailboxID: package.commandMailboxID,
                        writeCapability: package.commandMailboxWriteCapability
                    ),
                    cursor: item.cursor,
                    nextSenderSequence: 1,
                    revoked: false
                )
                state = CompanionPersistentState(
                    pairing: record,
                    workspace: nil,
                    outbox: [],
                    replay: await replay.exportState()
                )
                try await persist(identity: identity)
                try await relay.acknowledge(
                    endpoint: endpoint,
                    mailboxID: mailbox.mailboxID,
                    acknowledgementCapability: secrets.acknowledgement,
                    cursor: item.cursor
                )
                connectionContinuation.yield(.connected)
                // The grant and initial snapshot are separate opaque relay
                // envelopes. A phone can observe the grant in the narrow
                // window before the Mac finishes uploading the snapshot, so
                // one empty reconciliation must not complete pairing with an
                // unusable workspace.
                var snapshotDelay: UInt64 = 100_000_000
                for attempt in 0 ..< 16 {
                    try await reconcile()
                    if state?.workspace != nil { return }
                    if attempt < 15 {
                        try await synchronizationSleep(snapshotDelay)
                        snapshotDelay = min(snapshotDelay * 2, 1_000_000_000)
                    }
                }
                connectionContinuation.yield(.disconnected)
                throw TohsenoCompanionError.workspaceUnavailable
            }
            cursor = page.nextCursor
            try await Task.sleep(nanoseconds: delay)
            delay = min(delay * 2, 2_000_000_000)
        }
        connectionContinuation.yield(.disconnected)
        throw TohsenoCompanionError.invitationExpired
    }

    public func disconnect() async throws {
        await stopForegroundSynchronization()
        try await ensureLoaded()
        state = CompanionPersistentState()
        try await persist(identity: identityManager.identity())
        try await payloadStore.deleteAll()
        connectionContinuation.yield(.disconnected)
    }

    public func currentWorkspace() async throws -> WorkspaceSnapshot {
        try await ensureLoaded()
        try requireActivePairing()
        guard let workspace = state?.workspace else { throw TohsenoCompanionError.workspaceUnavailable }
        return workspace
    }

    /// Signed commands this phone has written that the Mac has not yet
    /// acknowledged.
    ///
    /// The phone is authoritative for its outbox until acknowledgement, so a
    /// product surface can honestly say "waiting for your Mac" without
    /// inferring it from transient connection state. Zero means everything
    /// written here has been received.
    public func unacknowledgedCommandCount() async throws -> Int {
        try await ensureLoaded()
        return state?.outbox.count ?? 0
    }

    /// Return locally cached exact bytes for a workspace icon descriptor.
    /// The cache is populated only by authenticated encrypted `icon.blob`
    /// events and is itself encrypted with the companion storage key.
    public func iconBlob(for descriptor: IconDescriptor) async throws -> CompanionIconBlob? {
        try await ensureLoaded()
        try requireActivePairing()
        guard let blob = state?.iconBlobs[descriptor.blobID] else { return nil }
        try blob.matches(descriptor)
        return blob
    }

    public func submitFeedback(_ request: FeedbackRequest) async throws -> CommandReceipt {
        return try await queue(
            commandID: request.commandID,
            payload: .feedbackSubmit(
                shotID: request.shotID,
                expressionID: request.expressionID,
                versionID: request.versionID,
                versionOrdinal: request.versionOrdinal,
                body: request.body
            )
        )
    }

    public func submitMarketingNote(_ request: MarketingRequest) async throws -> CommandReceipt {
        return try await queue(
            commandID: request.commandID,
            payload: .marketingSubmit(noteID: request.noteID, shotID: request.shotID, body: request.body)
        )
    }

    public func requestEvolution(_ request: EvolutionRequest) async throws -> CommandReceipt {
        let descriptors = try request.references.map { reference in
            try reference.validate()
            return reference.descriptor
        }
        return try await queue(
            commandID: request.commandID,
            payload: .shotEvolveRequest(
                shotID: request.shotID,
                baseExpressionID: request.baseExpressionID,
                baseVersionID: request.baseVersionID,
                baseVersionOrdinal: request.baseVersionOrdinal,
                intention: request.intention,
                selectedFeedbackActionCommitments: request.selectedFeedbackActionCommitments,
                references: descriptors
            ),
            references: request.references
        )
    }

    public func requestShotCreation(_ request: CreateShotRequest) async throws -> CommandReceipt {
        let descriptors = try request.references.map { reference in
            try reference.validate()
            return reference.descriptor
        }
        return try await queue(
            commandID: request.commandID,
            payload: .shotCreateRequest(
                suggestedName: request.suggestedName,
                intention: request.intention,
                references: descriptors
            ),
            references: request.references
        )
    }

    /// Request an authoritative full snapshot through the same signed command
    /// journal. Reconciliation invokes this automatically after relay
    /// retention advances beyond the stored cursor.
    public func requestWorkspaceSnapshot(
        commandID: String = UUID().uuidString.lowercased()
    ) async throws -> CommandReceipt {
        try await queue(commandID: commandID, payload: .workspaceSnapshotRequest)
    }

    public func reconcile() async throws {
        await enterReconciliation()
        defer { leaveReconciliation() }
        try Task.checkCancellation()
        try await reconcilePass()
    }

    private func reconcilePass() async throws {
        try await ensureLoaded()
        try requireActivePairing()
        let identity = try await identityManager.identity()
        do {
            try await flushOutbox(identity: identity)
        } catch TohsenoCompanionError.capabilityRevoked {
            if var pairing = state?.pairing {
                pairing.revoked = true
                state?.pairing = pairing
            }
            state?.workspace = nil
            state?.iconBlobs = [:]
            state?.outbox = []
            state?.referenceOutbox = []
            try await persist(identity: identity)
            try await payloadStore.deleteAll()
            connectionContinuation.yield(.revoked)
            throw TohsenoCompanionError.capabilityRevoked
        }
        guard var pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
        let endpoint = try relayEndpoint(pairing)
        let replay = try CompanionReplayProtection(capacity: 4096, state: state?.replay ?? .init())
        var hasMore = true
        while hasMore {
            let page: RelayMailboxPage
            do {
                page = try await relay.fetchEnvelopes(
                    endpoint: endpoint,
                    mailboxID: pairing.inbox.mailboxID,
                    readCapability: pairing.inbox.readCapability,
                    after: pairing.cursor
                )
            } catch TohsenoCompanionError.capabilityRevoked {
                pairing.revoked = true
                state?.pairing = pairing
                state?.workspace = nil
                state?.iconBlobs = [:]
                state?.outbox = []
                state?.referenceOutbox = []
                try await persist(identity: identity)
                try await payloadStore.deleteAll()
                connectionContinuation.yield(.revoked)
                throw TohsenoCompanionError.capabilityRevoked
            } catch let TohsenoCompanionError.cursorResetRequired(resetBefore, head) {
                pairing.cursor = resetBefore
                state?.pairing = pairing
                state?.workspace = nil
                state?.iconBlobs = [:]
                try await persist(identity: identity)
                connectionContinuation.yield(.reconnecting)
                _ = try await queue(
                    commandID: snapshotResetCommandID(
                        pairing: pairing,
                        resetBefore: resetBefore,
                        head: head
                    ),
                    payload: .workspaceSnapshotRequest
                )
                return
            }
            try page.validateRouting(mailboxID: pairing.inbox.mailboxID, afterCursor: pairing.cursor)
            for item in page.envelopes {
                let plaintext = try await CompanionEnvelopeCrypto.open(
                    item.envelope,
                    expectedSenderSigningPublicKey: Base64URL.decode(
                        pairing.studioSigningPublicKey,
                        expectedBytes: 32
                    ),
                    expectedSenderDeviceID: pairing.studioDeviceID,
                    expectedMailboxID: pairing.inbox.mailboxID,
                    recipient: identity,
                    now: now(),
                    replay: replay
                )
                let event = try StrictJSON.decode(
                    WorkspaceEvent.self,
                    from: plaintext,
                    maximumBytes: CompanionLimits.maximumWorkspaceEventBytes
                )
                try event.validate()
                let retiredPayloadIDs = try apply(
                    event,
                    ownDeviceID: identity.description.deviceID
                )
                pairing = state?.pairing ?? pairing
                pairing.cursor = item.cursor
                state?.pairing = pairing
                state?.replay = await replay.exportState()
                try await persist(identity: identity)
                if state?.pairing?.revoked == true {
                    try await payloadStore.deleteAll()
                } else {
                    for payloadID in retiredPayloadIDs {
                        try await payloadStore.delete(id: payloadID)
                    }
                }
                try await relay.acknowledge(
                    endpoint: endpoint,
                    mailboxID: pairing.inbox.mailboxID,
                    acknowledgementCapability: pairing.inbox.acknowledgementCapability,
                    cursor: item.cursor
                )
                eventContinuation.yield(event)
            }
            hasMore = page.hasMore
            if page.envelopes.isEmpty { hasMore = false }
        }
        connectionContinuation.yield(pairing.revoked ? .revoked : .connected)
    }

    /// Keep an active companion synchronized without polling. This performs an
    /// initial cursor reconciliation, listens to the relay's content-blind SSE
    /// wake stream, and reconnects with bounded exponential backoff whenever a
    /// connection ends. Calling it repeatedly is idempotent.
    public func startForegroundSynchronization() async throws {
        try await ensureLoaded()
        try requireActivePairing()
        guard foregroundSynchronizationTask == nil else { return }
        foregroundSynchronizationGeneration &+= 1
        let generation = foregroundSynchronizationGeneration
        foregroundSynchronizationTask = Task { [weak self] in
            guard let self else { return }
            await self.runForegroundSynchronization(generation: generation)
        }
    }

    /// Stop active synchronization and wait until the live request and any
    /// pending reconnect delay have observed cancellation.
    public func stopForegroundSynchronization() async {
        foregroundSynchronizationGeneration &+= 1
        let task = foregroundSynchronizationTask
        foregroundSynchronizationTask = nil
        task?.cancel()
        await task?.value
        if state?.pairing?.revoked == true { connectionContinuation.yield(.revoked) }
        else { connectionContinuation.yield(.disconnected) }
    }

    public func registerForPush(using provider: any CompanionPushTokenProvider) async throws {
        guard let token = try await provider.currentAPNSToken() else { return }
        try await registerPushToken(token)
    }

    public func registerPushToken(_ token: Data) async throws {
        try await ensureLoaded()
        try requireActivePairing()
        guard let pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
        let identity = try await identityManager.identity()
        try await relay.registerPushToken(
            endpoint: relayEndpoint(pairing),
            mailboxID: pairing.inbox.mailboxID,
            pushCapability: pairing.inbox.pushCapability,
            deviceID: identity.description.deviceID,
            token: token
        )
    }

    public func unregisterPushToken() async throws {
        try await ensureLoaded()
        guard let pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
        let identity = try await identityManager.identity()
        try await relay.unregisterPushToken(
            endpoint: relayEndpoint(pairing),
            mailboxID: pairing.inbox.mailboxID,
            pushCapability: pairing.inbox.pushCapability,
            deviceID: identity.description.deviceID
        )
    }

    /// APNs payloads carry no content. A wake always performs ordinary cursor
    /// reconciliation through the authenticated encrypted mailbox.
    public func handlePushWake() async throws { try await reconcile() }

    private func runForegroundSynchronization(generation: UInt64) async {
        let initialDelay: UInt64 = 250_000_000
        let maximumDelay: UInt64 = 30_000_000_000
        var delay = initialDelay
        while !Task.isCancelled, generation == foregroundSynchronizationGeneration {
            do {
                try await reconcile()
                try Task.checkCancellation()
                guard let pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
                let stream = try await relay.liveEvents(
                    endpoint: relayEndpoint(pairing),
                    mailboxID: pairing.inbox.mailboxID,
                    readCapability: pairing.inbox.readCapability,
                    after: pairing.cursor
                )
                var receivedEvent = false
                for try await event in stream {
                    try Task.checkCancellation()
                    receivedEvent = true
                    delay = initialDelay
                    switch event {
                    case .envelope, .reconcile:
                        try await reconcile()
                    case .revoked:
                        // Confirm revocation through the ordinary authenticated
                        // mailbox path before discarding durable local drafts.
                        try await reconcile()
                        throw TohsenoCompanionError.transportUnavailable
                    }
                }
                if receivedEvent { delay = initialDelay }
                try Task.checkCancellation()
                connectionContinuation.yield(.reconnecting)
            } catch is CancellationError {
                break
            } catch TohsenoCompanionError.capabilityRevoked {
                // `reconcile` persists revoked state before returning this
                // error. A live-connect 410 has no such pass, so do one here.
                do { try await reconcile() } catch { /* persisted when authoritative */ }
                if state?.pairing?.revoked == true { break }
                connectionContinuation.yield(.reconnecting)
            } catch {
                connectionContinuation.yield(.reconnecting)
            }
            do {
                try await synchronizationSleep(delay)
            } catch {
                break
            }
            delay = min(delay * 2, maximumDelay)
        }
        if generation == foregroundSynchronizationGeneration {
            foregroundSynchronizationTask = nil
        }
    }

    private func enterReconciliation() async {
        while reconciliationInProgress {
            await withCheckedContinuation { continuation in
                reconciliationWaiters.append(continuation)
            }
        }
        reconciliationInProgress = true
    }

    private func leaveReconciliation() {
        reconciliationInProgress = false
        if !reconciliationWaiters.isEmpty { reconciliationWaiters.removeFirst().resume() }
    }

    private func queue(
        commandID: String,
        payload: CompanionCommandPayload,
        references: [CompanionReferenceBlob] = []
    ) async throws -> CommandReceipt {
        try await ensureLoaded()
        try requireActivePairing()
        guard var pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
        try pairing.grant.require(payload.requiredCapability)
        if let existing = state?.outbox.first(where: { $0.command.commandID == commandID }) {
            guard existing.command.payload == payload else {
                throw TohsenoCompanionError.commandRejected("idempotency key payload differs")
            }
            try requireCommandAdmissionWindow(existing.command)
            let identity = try await identityManager.identity()
            do {
                try await flushOutbox(identity: identity)
            } catch TohsenoCompanionError.transportUnavailable {
                connectionContinuation.yield(.reconnecting)
            }
            return CommandReceipt(commandID: commandID, state: .received)
        }
        guard references.count <= 8, Set(references.map(\.blobID)).count == references.count else {
            throw TohsenoCompanionError.invalidEncoding("references exceed bounds or repeat")
        }
        for reference in references { try reference.validate() }
        let newChunkCount = references.reduce(into: 0) { total, reference in
            total += (reference.bytes.count + CompanionReferenceBlob.maximumChunkByteLength - 1)
                / CompanionReferenceBlob.maximumChunkByteLength
        }
        guard (state?.outbox.count ?? 0) < CompanionLimits.maximumPendingCommands,
              (state?.referenceOutbox.count ?? 0) + newChunkCount
              <= CompanionLimits.maximumPendingReferenceChunks else {
            throw TohsenoCompanionError.commandRejected("durable companion outbox is full")
        }
        let expectedDescriptors = references.map(\.descriptor)
        let payloadDescriptors: [CompanionReferenceDescriptor] = switch payload {
        case let .shotCreateRequest(_, _, values): values
        case let .shotEvolveRequest(_, _, _, _, _, _, values): values
        default: []
        }
        guard payloadDescriptors == expectedDescriptors else {
            throw TohsenoCompanionError.invalidEncoding("reference payload and exact bytes differ")
        }
        let identity = try await identityManager.identity()
        let createdAt = CompanionTimestamp.format(now())
        let expiresAt = CompanionTimestamp.format(now().addingTimeInterval(6 * 24 * 60 * 60))
        let recipientKey = try Base64URL.decode(
            pairing.studioAgreementPublicKey,
            expectedBytes: 32
        )
        let priorPairing = pairing
        var pendingChunks: [PendingReferenceChunk] = []
        var storedPayloadIDs: [String] = []
        do {
            for reference in references {
                for chunk in try reference.transportChunks() {
                    let chunkBytes = try StrictJSON.encode(chunk)
                    let localPayloadID = "payload_\(UUID().uuidString.lowercased())"
                    try await payloadStore.save(
                        id: localPayloadID,
                        bytes: try CompanionLocalPayloadCodec.seal(
                            chunkBytes,
                            key: identity.storageKey,
                            binding: localReferenceBinding(
                                commandID: commandID,
                                blobID: chunk.blobID,
                                chunkIndex: chunk.chunkIndex,
                                chunkCount: chunk.chunkCount
                            )
                        )
                    )
                    storedPayloadIDs.append(localPayloadID)
                    let envelopeID = UUID().uuidString.lowercased()
                    let envelope = try CompanionEnvelopeCrypto.seal(
                        sender: identity,
                        recipientAgreementPublicKey: recipientKey,
                        metadata: CompanionEnvelopeMetadata(
                            envelopeID: envelopeID,
                            mailboxID: pairing.outbox.mailboxID,
                            recipientDeviceID: pairing.studioDeviceID,
                            senderSequence: pairing.nextSenderSequence,
                            createdAt: createdAt,
                            expiresAt: expiresAt
                        ),
                        plaintext: chunkBytes,
                        entropySource: entropy
                    )
                    try await payloadStore.save(
                        id: envelopeID,
                        bytes: StrictJSON.encode(envelope)
                    )
                    storedPayloadIDs.append(envelopeID)
                    pendingChunks.append(PendingReferenceChunk(
                        commandID: commandID,
                        blobID: chunk.blobID,
                        chunkIndex: chunk.chunkIndex,
                        chunkCount: chunk.chunkCount,
                        envelopeID: envelopeID,
                        localPayloadID: localPayloadID,
                        uploaded: false
                    ))
                    pairing.nextSenderSequence += 1
                }
            }
            let command = try CompanionCommand.sign(
                identity: identity,
                commandID: commandID,
                workspaceID: pairing.grant.workspaceID,
                capabilityID: pairing.grant.capabilityID,
                createdAt: createdAt,
                payload: payload
            )
            let envelope = try CompanionEnvelopeCrypto.seal(
                sender: identity,
                recipientAgreementPublicKey: recipientKey,
                metadata: CompanionEnvelopeMetadata(
                    envelopeID: UUID().uuidString.lowercased(),
                    mailboxID: pairing.outbox.mailboxID,
                    recipientDeviceID: pairing.studioDeviceID,
                    senderSequence: pairing.nextSenderSequence,
                    createdAt: createdAt,
                    expiresAt: expiresAt
                ),
                plaintext: try StrictJSON.encode(command),
                entropySource: entropy
            )
            pairing.nextSenderSequence += 1
            state?.pairing = pairing
            state?.referenceOutbox.append(contentsOf: pendingChunks)
            state?.outbox.append(PendingCompanionCommand(
                command: command,
                envelope: envelope,
                uploaded: false
            ))
            try await persist(identity: identity)
        } catch {
            state?.pairing = priorPairing
            state?.referenceOutbox.removeAll { $0.commandID == commandID }
            state?.outbox.removeAll { $0.command.commandID == commandID }
            for payloadID in storedPayloadIDs {
                try? await payloadStore.delete(id: payloadID)
            }
            throw error
        }
        do {
            try await flushOutbox(identity: identity)
        } catch TohsenoCompanionError.transportUnavailable {
            connectionContinuation.yield(.reconnecting)
        }
        return CommandReceipt(commandID: commandID, state: .received)
    }

    private func flushOutbox(identity: CompanionIdentity) async throws {
        guard var pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
        let endpoint = try relayEndpoint(pairing)
        let recipientKey = try Base64URL.decode(
            pairing.studioAgreementPublicKey,
            expectedBytes: 32
        )
        let commandIDs = Set(state?.outbox.map { $0.command.commandID } ?? [])
        guard state?.referenceOutbox.allSatisfy({ commandIDs.contains($0.commandID) }) == true else {
            throw TohsenoCompanionError.unsafeStorage
        }
        for commandIndex in state?.outbox.indices ?? [].indices {
            let commandID = state!.outbox[commandIndex].command.commandID
            guard commandIsInsideAdmissionWindow(state!.outbox[commandIndex].command) else {
                // Keep receiving receipts, snapshots, and revocation events
                // even when an old outbound draft needs explicit owner action.
                continue
            }
            for referenceIndex in state?.referenceOutbox.indices ?? [].indices {
                guard state?.referenceOutbox[referenceIndex].commandID == commandID else { continue }
                var pending = state!.referenceOutbox[referenceIndex]
                guard let bytes = try await payloadStore.load(id: pending.envelopeID) else {
                    throw TohsenoCompanionError.unsafeStorage
                }
                var envelope = try StrictJSON.decode(
                    OpaqueCompanionEnvelope.self,
                    from: bytes,
                    maximumBytes: CompanionLimits.maximumEnvelopeBodyBytes
                )
                guard envelope.envelopeID == pending.envelopeID,
                      envelope.mailboxID == pairing.outbox.mailboxID else {
                    throw TohsenoCompanionError.unsafeStorage
                }
                if try envelopeNeedsResealing(envelope) {
                    let previousPending = pending
                    let previousPairing = pairing
                    let localPayloadID = try requireLocalPayloadID(pending)
                    guard let sealedPayload = try await payloadStore.load(id: localPayloadID) else {
                        throw TohsenoCompanionError.unsafeStorage
                    }
                    let chunkBytes = try CompanionLocalPayloadCodec.open(
                        sealedPayload,
                        key: identity.storageKey,
                        binding: localReferenceBinding(
                            commandID: pending.commandID,
                            blobID: pending.blobID,
                            chunkIndex: pending.chunkIndex,
                            chunkCount: pending.chunkCount
                        )
                    )
                    let chunk = try StrictJSON.decode(
                        CompanionReferenceBlobChunk.self,
                        from: chunkBytes,
                        maximumBytes: CompanionLimits.maximumEnvelopeBodyBytes
                    )
                    let committedDescriptor = CompanionReferenceDescriptor(
                        blobID: chunk.blobID,
                        originName: chunk.originName,
                        mediaType: chunk.mediaType,
                        byteLength: chunk.byteLength,
                        sha256: chunk.sha256
                    )
                    guard chunk.blobID == pending.blobID,
                          chunk.chunkIndex == pending.chunkIndex,
                          chunk.chunkCount == pending.chunkCount,
                          commandReferenceDescriptors(state!.outbox[commandIndex].command)
                          .contains(committedDescriptor) else {
                        throw TohsenoCompanionError.unsafeStorage
                    }
                    let replacementID = UUID().uuidString.lowercased()
                    let replacement = try CompanionEnvelopeCrypto.seal(
                        sender: identity,
                        recipientAgreementPublicKey: recipientKey,
                        metadata: outboxMetadata(
                            envelopeID: replacementID,
                            pairing: pairing
                        ),
                        plaintext: chunkBytes,
                        entropySource: entropy
                    )
                    try await payloadStore.save(
                        id: replacementID,
                        bytes: StrictJSON.encode(replacement)
                    )
                    pending = PendingReferenceChunk(
                        commandID: pending.commandID,
                        blobID: pending.blobID,
                        chunkIndex: pending.chunkIndex,
                        chunkCount: pending.chunkCount,
                        envelopeID: replacementID,
                        localPayloadID: localPayloadID,
                        uploaded: false
                    )
                    pairing.nextSenderSequence += 1
                    state?.referenceOutbox[referenceIndex] = pending
                    state?.pairing = pairing
                    do {
                        try await persist(identity: identity)
                    } catch {
                        state?.referenceOutbox[referenceIndex] = previousPending
                        pairing = previousPairing
                        state?.pairing = pairing
                        try? await payloadStore.delete(id: replacementID)
                        throw error
                    }
                    // The replacement record is durable before the old exact
                    // ciphertext is removed. A crash only leaves an orphan,
                    // which ensureLoaded removes without losing the command.
                    try await payloadStore.delete(id: previousPending.envelopeID)
                    envelope = replacement
                }
                _ = try await relay.uploadEnvelope(
                    endpoint: endpoint,
                    mailboxID: pairing.outbox.mailboxID,
                    writeCapability: pairing.outbox.writeCapability,
                    envelope: envelope
                )
                if state?.referenceOutbox[referenceIndex].uploaded == false {
                    state?.referenceOutbox[referenceIndex].uploaded = true
                    try await persist(identity: identity)
                }
            }
            guard state?.referenceOutbox
                .filter({ $0.commandID == commandID })
                .allSatisfy(\.uploaded) == true else {
                throw TohsenoCompanionError.unsafeStorage
            }
            var pending = state!.outbox[commandIndex]
            if try envelopeNeedsResealing(pending.envelope) {
                let previousPending = pending
                let previousPairing = pairing
                pending.envelope = try CompanionEnvelopeCrypto.seal(
                    sender: identity,
                    recipientAgreementPublicKey: recipientKey,
                    metadata: outboxMetadata(
                        envelopeID: UUID().uuidString.lowercased(),
                        pairing: pairing
                    ),
                    plaintext: try StrictJSON.encode(pending.command),
                    entropySource: entropy
                )
                pending.uploaded = false
                pairing.nextSenderSequence += 1
                state?.outbox[commandIndex] = pending
                state?.pairing = pairing
                do {
                    try await persist(identity: identity)
                } catch {
                    state?.outbox[commandIndex] = previousPending
                    pairing = previousPairing
                    state?.pairing = pairing
                    throw error
                }
            }
            // Retry the exact same encrypted envelope on every reconciliation
            // while it remains valid. The relay's envelope-ID journal makes
            // this idempotent, and only a Mac-signed command receipt retires
            // the durable phone outbox entry.
            _ = try await relay.uploadEnvelope(
                endpoint: endpoint,
                mailboxID: pairing.outbox.mailboxID,
                writeCapability: pairing.outbox.writeCapability,
                envelope: pending.envelope
            )
            if state?.outbox[commandIndex].uploaded == false {
                state?.outbox[commandIndex].uploaded = true
                try await persist(identity: identity)
            }
        }
    }

    private func apply(_ event: WorkspaceEvent, ownDeviceID: String) throws -> [String] {
        guard let pairing = state?.pairing, event.workspaceID == pairing.grant.workspaceID else {
            throw TohsenoCompanionError.invalidEncoding("event workspace differs")
        }
        if case let .workspaceSnapshot(snapshot) = event.payload {
            guard snapshot.workspaceID == event.workspaceID,
                  snapshot.deviceCapabilityState.deviceID == ownDeviceID else {
                throw TohsenoCompanionError.invalidEncoding("snapshot recipient differs")
            }
            state?.workspace = snapshot
            try pruneIconCache(for: snapshot)
            return []
        }

        var retiredPayloadIDs: [String] = []
        switch event.payload {
        case let .commandAcknowledged(receipt), let .commandRejected(receipt):
            retiredPayloadIDs = payloadIDs(forCommandID: receipt.commandID)
            state?.outbox.removeAll { $0.command.commandID == receipt.commandID }
            state?.referenceOutbox.removeAll { $0.commandID == receipt.commandID }
        case let .deviceRevoked(deviceID, epoch):
            if deviceID == ownDeviceID, epoch >= pairing.grant.revocationEpoch {
                retiredPayloadIDs = allPendingPayloadIDs()
                var revoked = pairing
                revoked.revoked = true
                state?.pairing = revoked
                state?.workspace = nil
                state?.iconBlobs = [:]
                state?.outbox = []
                state?.referenceOutbox = []
                connectionContinuation.yield(.revoked)
                return retiredPayloadIDs
            }
        default:
            break
        }
        guard var workspace = state?.workspace else {
            // Only a full snapshot can reestablish authority after a retained
            // event gap. Incremental events are acknowledged but not guessed.
            return retiredPayloadIDs
        }
        if event.cursor < workspace.nextCursor { return retiredPayloadIDs }
        guard event.cursor == workspace.nextCursor else {
            state?.workspace = nil
            state?.iconBlobs = [:]
            connectionContinuation.yield(.reconnecting)
            return retiredPayloadIDs
        }
        switch event.payload {
        case let .shotUpsert(shot):
            workspace = workspace.replacingShot(shot)
        case let .shotArchive(shotID):
            workspace = workspace.updatingShot(shotID) { $0.with(archived: true) }
        case let .shotRemove(shotID):
            workspace = workspace.removingShot(shotID)
        case let .iconBlob(blob):
            let descriptors = workspace.shots.compactMap(\.icon)
            guard let descriptor = descriptors.first(where: { $0.blobID == blob.blobID }) else {
                throw TohsenoCompanionError.invalidEncoding("icon blob has no workspace descriptor")
            }
            try blob.matches(descriptor)
            state?.iconBlobs[blob.blobID] = blob
        case let .versionAccepted(shotID, expressionID, versionID, ordinal, acceptedAt):
            workspace = workspace.updatingShot(shotID) {
                $0.with(
                    expressionID: expressionID,
                    versionID: versionID,
                    versionOrdinal: ordinal,
                    versionCreatedAt: acceptedAt
                )
            }
        case let .executionQueued(execution), let .executionStarted(execution),
             let .executionUpdated(execution), let .executionWaitingForDevice(execution):
            workspace = workspace.updatingExecution(execution)
        case let .executionCompleted(execution), let .executionFailed(execution):
            workspace = workspace.updatingExecution(execution, terminal: true)
        case .productEntitlement, .commandAcknowledged, .commandRejected, .deviceRevoked:
            break
        case .workspaceSnapshot:
            break
        }
        state?.workspace = workspace.with(nextCursor: event.cursor + 1)
        try pruneIconCache(for: workspace)
        return retiredPayloadIDs
    }

    private func pruneIconCache(for workspace: WorkspaceSnapshot) throws {
        let maximumBytes = 16 * 1024 * 1024
        let descriptors = workspace.shots
            .sorted { ($0.sortIndex, $0.shotID) < ($1.sortIndex, $1.shotID) }
            .compactMap(\.icon)
        var kept: [String: CompanionIconBlob] = [:]
        var total = 0
        for descriptor in descriptors {
            guard let blob = state?.iconBlobs[descriptor.blobID],
                  (try? blob.matches(descriptor)) != nil,
                  blob.bytes.count <= maximumBytes - total
            else { continue }
            kept[blob.blobID] = blob
            total += blob.bytes.count
        }
        state?.iconBlobs = kept
    }

    private func validate(
        _ package: CompanionPairingGrantPackage,
        invitation: PairingInvitation,
        identity: CompanionIdentity,
        trustedStudioKey: Data
    ) throws {
        guard package.schema == CompanionPairingGrantPackage.schemaV1 else {
            throw TohsenoCompanionError.invalidCapability("pairing grant package schema")
        }
        try package.capabilityGrant.verify(
            trustedStudioSigningKey: trustedStudioKey,
            expectedWorkspaceID: invitation.workspaceID,
            expectedDeviceID: identity.description.deviceID,
            now: now()
        )
        let studioAgreement = try Base64URL.decode(
            package.studioAgreementPublicKey,
            expectedBytes: 32
        )
        guard CompanionIdentity.deviceID(
            signingPublicKey: trustedStudioKey,
            agreementPublicKey: studioAgreement
        ) == invitation.studioDeviceID else {
            throw TohsenoCompanionError.invalidCapability("Studio agreement key binding differs")
        }
        try requireIdentifier(package.commandMailboxID, field: "command_mailbox_id")
        _ = try Base64URL.decode(package.commandMailboxWriteCapability, expectedBytes: 32)
    }

    private func ensureLoaded() async throws {
        guard state == nil else { return }
        let identity = try await identityManager.identity()
        if let bytes = try await stateStore.load() {
            state = try CompanionStateCodec.open(bytes, key: identity.storageKey)
        } else {
            state = CompanionPersistentState()
        }
        let persistedCommandIDs = state?.outbox.map { $0.command.commandID } ?? []
        guard persistedCommandIDs.count <= CompanionLimits.maximumPendingCommands,
              Set(persistedCommandIDs).count == persistedCommandIDs.count,
              (state?.referenceOutbox.count ?? 0) <= CompanionLimits.maximumPendingReferenceChunks
        else { throw TohsenoCompanionError.unsafeStorage }
        let retainedValues = state?.referenceOutbox.flatMap { pending in
            [pending.envelopeID] + (pending.localPayloadID.map { [$0] } ?? [])
        } ?? []
        let retained = Set(retainedValues)
        guard retained.count == retainedValues.count,
              retained.count <= CompanionLimits.maximumPendingPayloadFiles else {
            throw TohsenoCompanionError.unsafeStorage
        }
        try await payloadStore.retainOnly(ids: retained)
        if state?.pairing?.revoked == true { connectionContinuation.yield(.revoked) }
        else if state?.pairing != nil { connectionContinuation.yield(.connected) }
    }

    private func persist(identity: CompanionIdentity) async throws {
        guard let state else { throw TohsenoCompanionError.unsafeStorage }
        try await stateStore.save(CompanionStateCodec.seal(state, key: identity.storageKey))
    }

    private func requireActivePairing() throws {
        guard let pairing = state?.pairing else { throw TohsenoCompanionError.notPaired }
        guard !pairing.revoked else { throw TohsenoCompanionError.capabilityRevoked }
    }

    private func envelopeNeedsResealing(_ envelope: OpaqueCompanionEnvelope) throws -> Bool {
        try envelope.validateShape()
        let expiration = try CompanionTimestamp.parse(envelope.expiresAt)
        // Reseal before the relay's 30-second skew boundary rather than race
        // an upload against expiry. The canonical command/chunk bytes remain
        // unchanged; only fresh routing encryption metadata is introduced.
        return expiration <= now().addingTimeInterval(60)
    }

    private func requireCommandAdmissionWindow(_ command: CompanionCommand) throws {
        guard commandIsInsideAdmissionWindow(command) else {
            // Re-signing the same command ID with a new created_at could
            // conflict with a Mac journal entry whose acknowledgement was
            // lost. Preserve the durable entry and require an explicit owner
            // decision instead of silently changing its idempotency digest.
            throw TohsenoCompanionError.commandRejected(
                "durable outbox command exceeded the 30-day offline admission window"
            )
        }
    }

    private func commandIsInsideAdmissionWindow(_ command: CompanionCommand) -> Bool {
        guard let created = try? CompanionTimestamp.parse(command.createdAt) else { return false }
        return now() <= created.addingTimeInterval(30 * 24 * 60 * 60)
    }

    private func outboxMetadata(
        envelopeID: String,
        pairing: CompanionPairingRecord
    ) throws -> CompanionEnvelopeMetadata {
        guard pairing.nextSenderSequence < CompanionLimits.maximumSafeJSONInteger else {
            throw TohsenoCompanionError.invalidEnvelope("sender sequence exhausted")
        }
        let created = now()
        return CompanionEnvelopeMetadata(
            envelopeID: envelopeID,
            mailboxID: pairing.outbox.mailboxID,
            recipientDeviceID: pairing.studioDeviceID,
            senderSequence: pairing.nextSenderSequence,
            createdAt: CompanionTimestamp.format(created),
            // Six days stays inside the relay/protocol seven-day upper bound
            // and leaves a full day for bounded clock skew and retries.
            expiresAt: CompanionTimestamp.format(created.addingTimeInterval(6 * 24 * 60 * 60))
        )
    }

    private func requireLocalPayloadID(_ pending: PendingReferenceChunk) throws -> String {
        guard let value = pending.localPayloadID else {
            // Pre-hardening state may contain an already-uploaded chunk whose
            // exact ciphertext was deleted. It cannot be honestly resealed.
            throw TohsenoCompanionError.envelopeExpired
        }
        try requireIdentifier(value, field: "reference_outbox.local_payload_id")
        return value
    }

    private func localReferenceBinding(
        commandID: String,
        blobID: String,
        chunkIndex: UInt64,
        chunkCount: UInt64
    ) -> Data {
        var result = Data("tohseno.companion.local-reference-payload.v1".utf8)
        for value in [commandID, blobID, String(chunkIndex), String(chunkCount)] {
            result.append(0)
            result.append(contentsOf: value.utf8)
        }
        return result
    }

    private func payloadIDs(forCommandID commandID: String) -> [String] {
        state?.referenceOutbox
            .filter { $0.commandID == commandID }
            .flatMap { pending in
                [pending.envelopeID] + (pending.localPayloadID.map { [$0] } ?? [])
            } ?? []
    }

    private func commandReferenceDescriptors(
        _ command: CompanionCommand
    ) -> [CompanionReferenceDescriptor] {
        switch command.payload {
        case let .shotCreateRequest(_, _, values): values
        case let .shotEvolveRequest(_, _, _, _, _, _, values): values
        default: []
        }
    }

    private func allPendingPayloadIDs() -> [String] {
        state?.referenceOutbox.flatMap { pending in
            [pending.envelopeID] + (pending.localPayloadID.map { [$0] } ?? [])
        } ?? []
    }

    private func relayEndpoint(_ pairing: CompanionPairingRecord) throws -> RelayEndpoint {
        guard let URL = URL(string: pairing.relayBaseURL) else {
            throw TohsenoCompanionError.relayNotAllowed
        }
        let endpoint = try RelayEndpoint(
            id: pairing.relayID,
            baseURL: URL,
            allowLoopbackHTTP: URL.scheme == "http",
            // The persisted endpoint must still equal the configured
            // allowlist below. This permits a debug `.local` endpoint to be
            // reconstructed after relaunch without widening trust.
            allowLocalNetworkHTTP: URL.scheme == "http"
        )
        let allowlisted = try allowlist.endpoint(for: endpoint.id)
        guard allowlisted == endpoint else { throw TohsenoCompanionError.relayNotAllowed }
        return endpoint
    }

    private func snapshotResetCommandID(
        pairing: CompanionPairingRecord,
        resetBefore: UInt64,
        head: UInt64
    ) -> String {
        var material = Data("tohseno.companion.snapshot-reset-command.v1\0".utf8)
        material.append(contentsOf: pairing.grant.workspaceID.utf8)
        material.append(0)
        material.append(contentsOf: pairing.grant.capabilityID.utf8)
        material.append(0)
        material.append(contentsOf: String(resetBefore).utf8)
        material.append(0)
        material.append(contentsOf: String(head).utf8)
        return "snapshot_\(Base64URL.encode(material.companionSHA256))"
    }
}

private extension WorkspaceSnapshot {
    func with(nextCursor: UInt64) -> Self {
        Self(
            workspaceID: workspaceID, snapshotVersion: snapshotVersion, generatedAt: generatedAt,
            serviceVersion: serviceVersion, shots: shots, activeExecutions: activeExecutions,
            deviceCapabilityState: deviceCapabilityState, nextCursor: nextCursor
        )
    }

    func replacingShot(_ shot: ShotSummary) -> Self {
        var values = shots.filter { $0.shotID != shot.shotID }
        values.append(shot)
        values.sort { ($0.sortIndex, $0.shotID) < ($1.sortIndex, $1.shotID) }
        return replacing(shots: values, executions: activeExecutions)
    }

    func removingShot(_ shotID: String) -> Self {
        replacing(
            shots: shots.filter { $0.shotID != shotID },
            executions: activeExecutions.filter { $0.shotID != shotID }
        )
    }

    func updatingShot(_ shotID: String, transform: (ShotSummary) -> ShotSummary) -> Self {
        replacing(
            shots: shots.map { $0.shotID == shotID ? transform($0) : $0 },
            executions: activeExecutions
        )
    }

    func updatingExecution(_ execution: ExecutionSummary, terminal: Bool = false) -> Self {
        var executions = activeExecutions.filter { $0.executionID != execution.executionID }
        if !terminal { executions.append(execution) }
        return replacing(
            shots: shots.map { $0.shotID == execution.shotID ? $0.with(execution: execution) : $0 },
            executions: executions
        )
    }

    func replacing(shots: [ShotSummary], executions: [ExecutionSummary]) -> Self {
        Self(
            workspaceID: workspaceID, snapshotVersion: snapshotVersion, generatedAt: generatedAt,
            serviceVersion: serviceVersion, shots: shots, activeExecutions: executions,
            deviceCapabilityState: deviceCapabilityState, nextCursor: nextCursor
        )
    }
}

private extension ShotSummary {
    func with(
        archived: Bool? = nil,
        expressionID: String? = nil,
        versionID: String? = nil,
        versionOrdinal: UInt64? = nil,
        versionCreatedAt: String? = nil,
        execution: ExecutionSummary? = nil
    ) -> Self {
        Self(
            shotID: shotID, displayName: displayName, bundleIdentifier: bundleIdentifier,
            kind: kind, icon: icon, iconRevision: iconRevision,
            expressionID: expressionID ?? self.expressionID,
            latestVersionID: versionID ?? latestVersionID,
            latestVersionOrdinal: versionOrdinal ?? latestVersionOrdinal,
            latestVersionCreatedAt: versionCreatedAt ?? latestVersionCreatedAt,
            execution: execution ?? self.execution, archived: archived ?? self.archived,
            retired: retired, sortIndex: sortIndex,
            supportedCompanionActions: supportedCompanionActions
        )
    }
}
