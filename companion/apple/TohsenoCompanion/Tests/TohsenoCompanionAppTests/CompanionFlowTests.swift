import Foundation
import Testing
import TohsenoCompanionKit
@testable import TohsenoCompanionApp

/// A stand-in for the Mac. The SDK's own tests cover pairing, envelopes,
/// durability, and reconciliation; these tests cover what the *product* does.
actor StubBackend: CompanionBackend {
    var shots: [ShotSummary]
    var iconData: [String: Data]
    var unacknowledged = 0
    var reachable = true
    private(set) var submissions: [EvolutionRequest] = []
    private(set) var projectSubmissions: [ProjectEvolutionRequest] = []
    private(set) var creations: [CreateShotRequest] = []
    private(set) var builderAnnouncements: [BuilderDeviceAnnouncement] = []
    private(set) var networkRequests: [(NetworkReleaseAction, String, String)] = []
    private(set) var followRequests: [(String, Bool)] = []
    private(set) var privateUpdateRequests: [PrivateUpdateItem] = []
    private(set) var privateUpdateReadRequests: [(String, Bool)] = []
    private(set) var reconciles = 0
    private(set) var synchronizations = 0
    private(set) var pairedInvitations: [String] = []
    private var failure: TohsenoCompanionError?
    private var pairingFailure: TohsenoCompanionError?

    let connectionStates: AsyncStream<CompanionConnectionState>
    let events: AsyncStream<WorkspaceEvent>
    private let eventContinuation: AsyncStream<WorkspaceEvent>.Continuation

    init(shots: [ShotSummary] = [], iconData: [String: Data] = [:]) {
        self.shots = shots
        self.iconData = iconData
        (connectionStates, _) = AsyncStream.makeStream(of: CompanionConnectionState.self)
        (events, eventContinuation) = AsyncStream.makeStream(of: WorkspaceEvent.self)
    }

    func set(shots: [ShotSummary]) { self.shots = shots }
    func set(unacknowledged: Int) { self.unacknowledged = unacknowledged }
    func set(reachable: Bool) { self.reachable = reachable }
    func rejectNext(with error: TohsenoCompanionError) { failure = error }
    func rejectNextPairing(with error: TohsenoCompanionError) { pairingFailure = error }

    func synchronizedWorkspace() async throws -> WorkspaceSnapshot {
        WorkspaceSnapshot(
            workspaceID: "workspace_fixture",
            snapshotVersion: 1,
            generatedAt: "2026-08-18T00:00:00Z",
            serviceVersion: "0.9.0",
            shots: shots,
            activeExecutions: [],
            deviceCapabilityState: DeviceCapabilityState(
                deviceID: "device_fixture",
                capabilityID: "capability_fixture",
                revocationEpoch: 0,
                allowedActions: [.workspaceRead, .shotCreate, .shotEvolve],
                revoked: false
            ),
            nextCursor: 0
        )
    }

    func reconcile() async throws {
        reconciles += 1
        if !reachable { throw TohsenoCompanionError.transportUnavailable }
    }

    func iconBytes(for descriptor: IconDescriptor) async throws -> Data? {
        iconData[descriptor.blobID]
    }

    func requestEvolution(_ request: EvolutionRequest) async throws -> CommandReceipt {
        if let failure {
            self.failure = nil
            throw failure
        }
        submissions.append(request)
        // The SDK persists the signed command before it tries the relay, so an
        // unreachable Mac is not an error — it is an unacknowledged command.
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: request.commandID, state: .received)
    }

    func requestProjectEvolution(_ request: ProjectEvolutionRequest) async throws -> CommandReceipt {
        if let failure {
            self.failure = nil
            throw failure
        }
        projectSubmissions.append(request)
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: request.commandID, state: .received)
    }

    func requestShotCreation(_ request: CreateShotRequest) async throws -> CommandReceipt {
        if let failure {
            self.failure = nil
            throw failure
        }
        creations.append(request)
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: request.commandID, state: .received)
    }

    func announceBuilderDevice(
        _ builderDevice: BuilderDeviceAnnouncement,
        commandID: String
    ) async throws -> CommandReceipt {
        builderAnnouncements.append(builderDevice)
        return CommandReceipt(commandID: commandID, state: .completed)
    }

    func approvePublication(
        jobID: String,
        catalog: BuilderDeviceSignature,
        registry: BuilderDeviceSignature,
        claimEdition: ApprovedClaimEdition?,
        approvedAt: String,
        commandID: String
    ) async throws -> CommandReceipt {
        CommandReceipt(commandID: commandID, state: .completed, resultID: jobID)
    }

    func requestNetworkRelease(
        action: NetworkReleaseAction,
        shotID: String,
        releaseDigest: String,
        commandID: String
    ) async throws -> CommandReceipt {
        networkRequests.append((action, shotID, releaseDigest))
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: commandID, state: .received, resultID: releaseDigest)
    }

    func setBuilderFollow(
        builderID: String,
        followed: Bool,
        commandID: String
    ) async throws -> CommandReceipt {
        followRequests.append((builderID, followed))
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: commandID, state: .received)
    }

    func upsertPrivateUpdate(
        _ update: PrivateUpdateItem,
        commandID: String
    ) async throws -> CommandReceipt {
        privateUpdateRequests.append(update)
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: commandID, state: .received, resultID: update.updateID)
    }

    func setPrivateUpdateRead(
        updateID: String,
        read: Bool,
        commandID: String
    ) async throws -> CommandReceipt {
        privateUpdateReadRequests.append((updateID, read))
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: commandID, state: .received, resultID: updateID)
    }

    func unacknowledgedCommandCount() async throws -> Int { unacknowledged }
    func createIdentity() async throws -> RecoveryPhrase {
        try RecoveryPhrase(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        )
    }

    func pair(invitation: String, displayName: String) async throws {
        pairedInvitations.append(invitation)
        if let pairingFailure {
            self.pairingFailure = nil
            throw pairingFailure
        }
        if invitation.hasPrefix("tohseno://pair/") { return }
        throw TohsenoCompanionError.invalidInvitation("fixture")
    }

    func startSynchronization() async throws { synchronizations += 1 }
}

actor StubSoftwareClaimIdentity: SoftwareClaimIdentity {
    let claimant = "0x4444444444444444444444444444444444444444"

    func claimPublicIdentity() async throws -> BuilderDevicePublicIdentity {
        try JSONDecoder().decode(BuilderDevicePublicIdentity.self, from: JSONSerialization.data(
            withJSONObject: [
                "schema": "tohseno.builder-device-public-identity/1",
                "key_id": "0x" + String(repeating: "11", count: 32),
                "x": "0x" + String(repeating: "22", count: 32),
                "y": "0x" + String(repeating: "33", count: 32),
                "security_level": "secure_enclave",
                "test_only": false,
            ]
        ))
    }

    func claimBuilderID() async throws -> String {
        "eip155:4663:\(claimant)"
    }

    func signClaimDigest(_ digestHex: String) async throws -> BuilderDeviceAuthorization {
        let identity = try await claimPublicIdentity()
        let encodedIdentity = try JSONSerialization.jsonObject(with: JSONEncoder().encode(identity))
        return try JSONDecoder().decode(BuilderDeviceAuthorization.self, from: JSONSerialization.data(
            withJSONObject: [
                "schema": "tohseno.builder-device-authorization/1",
                "signer": encodedIdentity,
                "algorithm": "p256",
                "digest": digestHex,
                "r": "0x" + String(repeating: "01", count: 32),
                "s": "0x" + String(repeating: "02", count: 32),
                "low_s": true,
            ]
        ))
    }
}

actor StubClaimsHTTP {
    static let contract = "0x6666666666666666666666666666666666666666"
    static let activation = "0x" + String(repeating: "aa", count: 32)
    static let registry = ClaimsClientActivation.shotRegistry
    static let shotID = "0x" + String(repeating: "11", count: 32)
    static let releaseDigest = "0x" + String(repeating: "55", count: 32)
    static let checkpoint = "0x" + String(repeating: "77", count: 32)
    static let transaction = "0x" + String(repeating: "99", count: 32)
    static let builderID = "eip155:4663:0x2222222222222222222222222222222222222222"
    static let jobID = String(repeating: "ab", count: 16)
    static let jobToken = String(repeating: "cd", count: 32)

    private let receiptReleaseDigest: String
    private var claimant: String?
    private var gesture: String?
    private(set) var prepareCount = 0
    private(set) var submitCount = 0
    private(set) var statusCount = 0

    init(tamperedReceipt: Bool = false) {
        receiptReleaseDigest = tamperedReceipt
            ? "0x" + String(repeating: "ee", count: 32)
            : Self.releaseDigest
    }

    func response(for request: URLRequest) async throws -> (Data, URLResponse) {
        guard let url = request.url else { throw URLError(.badURL) }
        if url.path.hasSuffix("/claims/prepare"), request.httpMethod == "POST" {
            prepareCount += 1
            let body = try #require(request.httpBody)
            let object = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
            claimant = try #require(object["claimant"] as? String)
            let canonical = try #require(object["claim_mark"] as? String)
            let bytes = try #require(Data(prefixedHex: canonical))
            gesture = try ClaimMark(canonicalBytes: bytes).gestureCommitment.prefixedHex
            return try json([
                "schema": "tohseno.software-claim-preparation/1",
                "job_id": Self.jobID,
                "job_token": Self.jobToken,
                "chain_id": 4663,
                "claims_contract": Self.contract,
                "claims_activation_signing_digest": Self.activation,
                "shot_registry": Self.registry,
                "shot_id": Self.shotID,
                "builder_id": Self.builderID,
                "release_digest": Self.releaseDigest,
                "checkpoint_digest": Self.checkpoint,
                "checkpoint_sequence": 1,
                "claimant": claimant!,
                "edition": ["max_claims": 0, "closes_at": 0, "total_claims": 0],
                "gesture_commitment": gesture!,
                "nonce": 0,
                "deadline": Int(Date().timeIntervalSince1970) + 600,
            ], status: 201, url: url)
        }
        guard request.value(forHTTPHeaderField: "Authorization") == "Bearer \(Self.jobToken)",
              let claimant, let gesture
        else { throw URLError(.userAuthenticationRequired) }
        if url.path.hasSuffix("/submit"), request.httpMethod == "POST" {
            submitCount += 1
            let body = try #require(request.httpBody)
            let object = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let action = try #require(object["action"] as? [String: Any])
            guard action["shot_id"] as? String == Self.shotID,
                  action["release_digest"] as? String == Self.releaseDigest,
                  action["checkpoint_digest"] as? String == Self.checkpoint,
                  action["claimant"] as? String == claimant,
                  action["gesture_commitment"] as? String == gesture
            else { throw URLError(.cannotParseResponse) }
            return try json(status("authorized", claimant: claimant, gesture: gesture), status: 202, url: url)
        }
        if url.path.hasSuffix("/\(Self.jobID)"), request.httpMethod == "GET" {
            statusCount += 1
            var complete = status("complete", claimant: claimant, gesture: gesture)
            complete["claim"] = [
                "token_id": "1",
                "shot_id": Self.shotID,
                "claim_number": "1",
                "claimant": claimant,
                "release_digest": receiptReleaseDigest,
                "checkpoint_digest": Self.checkpoint,
                "gesture_commitment": gesture,
                "transaction_hash": Self.transaction,
            ]
            return try json(complete, status: 200, url: url)
        }
        throw URLError(.unsupportedURL)
    }

    private func status(
        _ value: String,
        claimant: String,
        gesture: String
    ) -> [String: Any] {
        [
            "schema": "tohseno.software-claim-status/1",
            "job_id": Self.jobID,
            "status": value,
            "shot_id": Self.shotID,
            "release_digest": Self.releaseDigest,
            "gesture_commitment": gesture,
            "claim": NSNull(),
            "failure": NSNull(),
        ]
    }

    private func json(
        _ object: [String: Any],
        status: Int,
        url: URL
    ) throws -> (Data, URLResponse) {
        let data = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        let response = try #require(HTTPURLResponse(
            url: url,
            statusCode: status,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        ))
        return (data, response)
    }
}

private func claimablePublicApp() throws -> PublicAppRelease {
    let data = try JSONSerialization.data(withJSONObject: [
        "release_digest": StubClaimsHTTP.releaseDigest,
        "route": "/s/\(StubClaimsHTTP.shotID)",
        "source_url": "https://tohseno.com/api/registry/v1/blobs/" + String(repeating: "88", count: 32),
        "icon_url": NSNull(),
        "release": [
            "shot_id": StubClaimsHTTP.shotID,
            "builder_id": StubClaimsHTTP.builderID,
            "checkpoint_sequence": 1,
            "public_checkpoint_digest": StubClaimsHTTP.checkpoint,
            "display": [
                "name": "Orbit",
                "description": "One exact app",
                "builder_handle": NSNull(),
                "app_slug": NSNull(),
            ],
            "permissions": ["install_allowed": true, "fork_allowed": true],
        ],
    ], options: [.sortedKeys])
    return try JSONDecoder().decode(PublicAppRelease.self, from: data)
}

private extension Data {
    init?(prefixedHex: String) {
        guard prefixedHex.hasPrefix("0x"), prefixedHex.count % 2 == 0 else { return nil }
        var bytes = Data()
        var index = prefixedHex.index(prefixedHex.startIndex, offsetBy: 2)
        while index < prefixedHex.endIndex {
            let next = prefixedHex.index(index, offsetBy: 2)
            guard let byte = UInt8(prefixedHex[index ..< next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self = bytes
    }

    var prefixedHex: String { "0x" + map { String(format: "%02x", $0) }.joined() }
}

@MainActor
func model(_ backend: StubBackend) async -> CompanionModel {
    let model = CompanionModel(backend: backend, deviceName: "Test iPhone")
    await model.refresh()
    return model
}

@Suite("Choose an app → request its next evolution")
struct CompanionFlowTests {
    @MainActor
    @Test("Each relaunch announces the same DeviceKey under a fresh command identity")
    func builderAnnouncementCommandIDsAreFresh() {
        let first = CompanionModel.builderAnnouncementCommandID()
        let second = CompanionModel.builderAnnouncementCommandID()

        #expect(first != second)
        #expect(first.range(of: #"^builder_announce_[0-9a-f]{32}$"#, options: .regularExpression) != nil)
        #expect(second.range(of: #"^builder_announce_[0-9a-f]{32}$"#, options: .regularExpression) != nil)
    }

    @MainActor
    @Test("A canonical Claim while the Mac is offline durably queues that exact release")
    func claimWhileMacOffline() async throws {
        let backend = StubBackend()
        await backend.set(reachable: false)
        let http = StubClaimsHTTP()
        let network = PublicNetworkClient(
            origin: URL(string: "https://tohseno.example")!,
            claims: ClaimsClientCoordinates(
                shotRegistry: StubClaimsHTTP.registry,
                claimsContract: StubClaimsHTTP.contract,
                activationSigningDigest: StubClaimsHTTP.activation
            ),
            transport: { try await http.response(for: $0) }
        )
        let suite = "tohseno.claim-flow.\(UUID().uuidString)"
        let storage = try #require(UserDefaults(suiteName: suite))
        storage.removePersistentDomain(forName: suite)
        defer { storage.removePersistentDomain(forName: suite) }
        let subject = CompanionModel(
            backend: backend,
            deviceName: "Test iPhone",
            network: network,
            claimIdentity: StubSoftwareClaimIdentity(),
            storage: storage
        )
        let app = try claimablePublicApp()

        await subject.claim(app, mark: ClaimMark.accessibilityHold())

        #expect(subject.claimStates[StubClaimsHTTP.shotID] == "Claimed #1")
        #expect(subject.claimedSoftware.map(\.claim.tokenID) == ["1"])
        #expect(subject.networkNotice == "Claimed #1. Waiting for your Mac.")
        let requests = await backend.networkRequests
        #expect(requests.count == 1)
        #expect(requests[0].0 == .install)
        #expect(requests[0].1 == StubClaimsHTTP.shotID)
        #expect(requests[0].2 == StubClaimsHTTP.releaseDigest)
        #expect(await backend.unacknowledged == 2, "the private Claim update and exact install intention are both durable")
        #expect(await backend.privateUpdateRequests.map(\.kind) == [.claimed])
        #expect(await http.prepareCount == 1)
        #expect(await http.submitCount == 1)
        #expect(await http.statusCount == 1)
        let persisted = try #require(storage.data(forKey: "tohseno.claimed-software.v1"))
        #expect(try JSONDecoder().decode([ClaimedSoftwareEncounter].self, from: persisted).count == 1)
    }

    @MainActor
    @Test("A server cannot substitute the release in a completed Claim receipt")
    func substitutedClaimReceipt() async throws {
        let backend = StubBackend()
        let http = StubClaimsHTTP(tamperedReceipt: true)
        let network = PublicNetworkClient(
            origin: URL(string: "https://tohseno.example")!,
            claims: ClaimsClientCoordinates(
                shotRegistry: StubClaimsHTTP.registry,
                claimsContract: StubClaimsHTTP.contract,
                activationSigningDigest: StubClaimsHTTP.activation
            ),
            transport: { try await http.response(for: $0) }
        )
        let suite = "tohseno.claim-substitution.\(UUID().uuidString)"
        let storage = try #require(UserDefaults(suiteName: suite))
        storage.removePersistentDomain(forName: suite)
        defer { storage.removePersistentDomain(forName: suite) }
        let subject = CompanionModel(
            backend: backend,
            deviceName: "Test iPhone",
            network: network,
            claimIdentity: StubSoftwareClaimIdentity(),
            storage: storage
        )

        await subject.claim(try claimablePublicApp(), mark: ClaimMark.accessibilityHold())

        #expect(subject.claimedSoftware.isEmpty)
        #expect(subject.claimStates[StubClaimsHTTP.shotID] == "Claiming…")
        #expect(await backend.networkRequests.isEmpty)
        #expect(storage.data(forKey: "tohseno.pending-software-claim.v1") != nil)
    }

    @MainActor
    @Test("Follow is one private exact-Builder preference and queues while offline")
    func privateFollow() async throws {
        let backend = StubBackend()
        await backend.set(reachable: false)
        let suite = "tohseno.follow.\(UUID().uuidString)"
        let storage = try #require(UserDefaults(suiteName: suite))
        storage.removePersistentDomain(forName: suite)
        defer { storage.removePersistentDomain(forName: suite) }
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone", storage: storage)
        let builder = "eip155:4663:0x2222222222222222222222222222222222222222"

        subject.toggleFollow(builderID: "@mutable-handle")
        subject.toggleFollow(builderID: builder)
        for _ in 0 ..< 20 where await backend.followRequests.isEmpty { await Task.yield() }

        #expect(subject.followedBuilderIDs == [builder])
        #expect(await backend.followRequests.count == 1)
        #expect(await backend.followRequests[0].0 == builder)
        #expect(await backend.followRequests[0].1 == true)
        #expect(storage.stringArray(forKey: "tohseno.followed-builders.v1") == [builder])
    }

    @MainActor
    @Test("The private Updates projection preserves evidence identity and reconciles read state")
    func privateUpdatesReadState() async throws {
        let backend = StubBackend()
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone")
        let update = PrivateUpdateItem(
            kind: .editionClosed,
            subjectID: StubClaimsHTTP.shotID,
            evidenceID: StubClaimsHTTP.transaction,
            title: "Your Claim Edition closed",
            detail: "The finite edition reached its canonical boundary.",
            occurredAt: "2026-08-31T12:00:00Z"
        )
        subject.apply(WorkspaceEvent(
            eventID: "event_private_updates",
            workspaceID: "workspace_fixture",
            cursor: 8,
            emittedAt: "2026-08-31T12:00:01Z",
            payload: .privateUpdates(PrivateUpdateProjection(
                items: [update], updatedAt: "2026-08-31T12:00:01Z"
            ))
        ))

        #expect(subject.privateUpdates == [update])
        subject.setPrivateUpdateRead(update)
        for _ in 0 ..< 20 where await backend.privateUpdateReadRequests.isEmpty { await Task.yield() }

        #expect(subject.privateUpdates[0].updateID == update.updateID)
        #expect(subject.privateUpdates[0].readAt != nil)
        #expect(await backend.privateUpdateReadRequests.count == 1)
        #expect(await backend.privateUpdateReadRequests[0].0 == update.updateID)
        #expect(await backend.privateUpdateReadRequests[0].1 == true)
    }

    @MainActor
    @Test("The main CTA sends one new-app intent through the durable backend")
    func createApp() async {
        let backend = StubBackend()
        let subject = await model(backend)
        subject.openCreate()
        #expect(subject.screen == .create)
        #expect(!subject.canCreate)

        subject.appName = "tiny-timer"
        subject.intent = "A tiny timer with one large start button."
        #expect(subject.canCreate)
        await subject.create()

        let creations = await backend.creations
        #expect(creations.count == 1)
        #expect(creations[0].suggestedName == "tiny-timer")
        #expect(creations[0].intention == "A tiny timer with one large start button.")
        #expect(subject.screen == .apps)
        #expect(subject.intent.isEmpty)
    }

    @MainActor
    @Test("One Shot can leave naming to the existing factory")
    func unnamedOneShot() async {
        let backend = StubBackend()
        let subject = CompanionModel(backend: backend, deviceName: "Fixture iPhone")
        subject.openCreate()
        subject.intent = "Make one calm breathing timer."
        #expect(subject.appName.isEmpty)
        #expect(subject.canCreate)

        await subject.create()

        let creations = await backend.creations
        #expect(creations.count == 1)
        #expect(creations[0].suggestedName == nil)
        #expect(creations[0].intention == "Make one calm breathing timer.")
        #expect(subject.screen == .apps)
    }

    @MainActor
    @Test("Your Apps lists the person's apps and nothing else")
    func yourApps() async {
        let retired = ShotSummary(
            shotID: "shot_gone", displayName: "gone", kind: .factoryShot,
            iconRevision: 1, retired: true, sortIndex: 1,
            supportedCompanionActions: [.workspaceRead]
        )
        let recording = ShotSummary(
            shotID: "shot_notes", displayName: "notes", kind: .recordingOnly,
            iconRevision: 1, sortIndex: 2, supportedCompanionActions: [.workspaceRead]
        )
        let subject = await model(StubBackend(shots: [shot(version: 4), retired, recording]))
        #expect(subject.apps.map(\.displayName) == ["anky"])
        #expect(subject.screen == .apps)
    }

    @MainActor
    @Test("Your Apps loads each app's real icon by its immutable blob identity")
    func appIcons() async throws {
        let icon = IconDescriptor(
            blobID: "icon_anky",
            revision: 2,
            mediaType: "image/png",
            byteLength: 3,
            width: 1,
            height: 1,
            placeholder: false
        )
        let anky = ShotSummary(
            shotID: "shot_anky",
            displayName: "anky",
            kind: .factoryShot,
            icon: icon,
            iconRevision: 2,
            expressionID: "expression_anky",
            latestVersionID: "version_anky",
            latestVersionOrdinal: 4,
            latestVersionCreatedAt: "2026-08-18T00:00:00Z",
            sortIndex: 0,
            supportedCompanionActions: [.workspaceRead]
        )
        let bytes = Data([0x01, 0x02, 0x03])
        let subject = await model(StubBackend(shots: [anky], iconData: [icon.blobID: bytes]))

        #expect(subject.icon(for: anky) == bytes)
        #expect(subject.icons[anky.shotID] == nil, "shot IDs and blob IDs are distinct namespaces")
    }

    @MainActor
    @Test("One tap sends one command, with no confirmation and no version picker")
    func oneTap() async throws {
        let backend = StubBackend(shots: [shot(version: 4)])
        let subject = await model(backend)
        subject.open(try #require(subject.apps.first))
        #expect(!subject.canEvolve, "an empty intent cannot be sent")

        subject.intent = "Make the timer smaller."
        #expect(subject.canEvolve)
        await subject.evolve()

        let submissions = await backend.submissions
        #expect(submissions.count == 1)
        // The exact accepted base was bound at submission by the app, not
        // chosen by the person.
        #expect(submissions[0].baseVersionOrdinal == 4)
        #expect(submissions[0].baseExpressionID == "expression_anky")
        #expect(submissions[0].intention == "Make the timer smaller.")
        // The composer is cleared, so a second tap cannot resend the same text.
        #expect(subject.intent.isEmpty)
        #expect(!subject.canEvolve)
        await subject.evolve()
        #expect(await backend.submissions.count == 1)
    }

    @MainActor
    @Test("Opening a yellow in-flight app is read-only and cannot spend another build")
    func openingInFlightAppIsReadOnly() async throws {
        let backend = StubBackend(shots: [shot(version: 4, execution: .building)])
        let subject = await model(backend)
        let anky = try #require(subject.apps.first)

        subject.open(anky)

        #expect(subject.screen == .app(anky.shotID))
        #expect(await backend.submissions.isEmpty)
        #expect(subject.presentation(for: anky).headline == "Building anky…")
        subject.intent = "Queue another expensive change."
        #expect(!subject.canEvolve)
        await subject.evolve()
        #expect(await backend.submissions.isEmpty)
    }

    @MainActor
    @Test("An adopted project routes one request by stable project identity")
    func adoptedProjectEvolution() async throws {
        let adopted = ShotSummary(
            shotID: "project_fixture",
            displayName: "Fixture",
            bundleIdentifier: "com.example.fixture",
            kind: .adoptedProject,
            sourceState: "state_fixture",
            iconRevision: 1,
            sortIndex: 0,
            supportedCompanionActions: [.workspaceRead, .shotEvolve]
        )
        let backend = StubBackend(shots: [adopted])
        let subject = await model(backend)
        subject.open(try #require(subject.apps.first))
        subject.intent = "Change X to Y."

        #expect(subject.canEvolve)
        await subject.evolve()

        let submissions = await backend.projectSubmissions
        #expect(submissions.count == 1)
        #expect(submissions[0].projectID == "project_fixture")
        #expect(submissions[0].baseSourceState == "state_fixture")
        #expect(submissions[0].intention == "Change X to Y.")
        #expect(await backend.submissions.isEmpty)
    }

    @MainActor
    @Test("Manual Sync reconciles once and never submits an app mutation")
    func manualSyncIsReadOnly() async {
        let backend = StubBackend(shots: [shot(version: 4)])
        let subject = await model(backend)
        #expect(await backend.reconciles == 1)

        await subject.syncNow()

        #expect(await backend.reconciles == 2)
        #expect(await backend.submissions.isEmpty)
        #expect(await backend.creations.isEmpty)
        #expect(!subject.syncing)
    }

    @MainActor
    @Test("Manual Sync says when the Mac could not be reached")
    func manualSyncFailureIsVisible() async {
        let backend = StubBackend(shots: [shot(version: 4)])
        let subject = await model(backend)
        await backend.set(reachable: false)

        await subject.syncNow()

        #expect(subject.notice == "Couldn’t sync with your Mac.")
        #expect(await backend.submissions.isEmpty)
    }

    @MainActor
    @Test("An unreachable Mac still accepts the request; the phone says so honestly")
    func offlineSubmission() async throws {
        let backend = StubBackend(shots: [shot(version: 4)])
        await backend.set(reachable: false)
        let subject = await model(backend)
        let anky = try #require(subject.apps.first)
        subject.open(anky)
        subject.intent = "Keep the writing on screen."
        await subject.evolve()

        #expect(await backend.submissions.count == 1)
        #expect(subject.unacknowledged == 1)
        let presentation = subject.presentation(for: anky)
        #expect(presentation.headline == "Waiting for your Mac…")
        // Nothing is claimed about a build that has not been received.
        #expect(presentation.state == .waiting)
        #expect(subject.notice == nil, "waiting is not an error the person must read")
    }

    @MainActor
    @Test("A busy Mac simply waits; there is no queue to manage")
    func busyMac() async throws {
        let backend = StubBackend(shots: [shot(version: 4)])
        let subject = await model(backend)
        let anky = try #require(subject.apps.first)
        subject.open(anky)
        subject.intent = "Second request while the Mac is building something else."
        await subject.evolve()

        // The Mac received it and has not started it yet: queued, which is the
        // same "Waiting" the person already understands.
        await backend.set(shots: [shot(version: 4, execution: .queued)])
        await subject.refresh()
        let waiting = subject.presentation(for: try #require(subject.apps.first))
        #expect(waiting.state == .waiting)
        #expect(waiting.headline == "Waiting…")
    }

    @MainActor
    @Test("A stale base is explained, not silently rebased")
    func staleBase() async throws {
        #expect(
            CompanionModel.humanRejection("stale_base_version")
                == "This app changed while your request was waiting. Review it and try again."
        )
        #expect(
            CompanionModel.humanRejection("stale_project_source_state")
                == "This app changed while your request was waiting. Review it and try again."
        )
        #expect(
            CompanionModel.humanRejection("project_busy")
                == "This app is already being changed. Wait for it to finish, then send the next request."
        )
        #expect(
            CompanionModel.humanRejection("device_revoked")
                == "This iPhone no longer has access to your Mac."
        )
        #expect(CompanionModel.humanRejection(nil) == "Your Mac couldn’t accept that request.")
    }

    @MainActor
    @Test("A refused command surfaces one human sentence")
    func refusedCommand() async throws {
        let backend = StubBackend(shots: [shot(version: 4)])
        let subject = await model(backend)
        subject.open(try #require(subject.apps.first))
        subject.intent = "Change something."
        await backend.rejectNext(with: .capabilityRevoked)
        await subject.evolve()
        #expect(subject.notice == "This iPhone no longer has access to your Mac.")
        // The words stay in the box so the person does not lose them.
        #expect(subject.intent == "Change something.")
    }

    @MainActor
    @Test("An app with no accepted version yet cannot be evolved")
    func nothingToEvolveYet() async throws {
        let backend = StubBackend(shots: [shot(version: nil, execution: .building)])
        let subject = await model(backend)
        subject.open(try #require(subject.apps.first))
        subject.intent = "Change something."
        #expect(!subject.canEvolve)
        await subject.evolve()
        #expect(await backend.submissions.isEmpty)
    }

    @MainActor
    @Test("Legacy entitlement projections never replace the person-to-person product")
    func legacyEntitlementDoesNotGate() async throws {
        let subject = await model(StubBackend(shots: [shot(version: 4)]))
        func event(_ projection: ProductEntitlementProjection, cursor: UInt64) -> WorkspaceEvent {
            WorkspaceEvent(
                eventID: "event_entitlement_\(cursor)",
                workspaceID: "workspace_fixture",
                cursor: cursor,
                emittedAt: "2026-08-18T00:00:00Z",
                payload: .productEntitlement(projection)
            )
        }
        subject.apply(event(ProductEntitlementProjection(
            phase: "trial_qualified", successfulDays: 5,
            factoryMutationsAllowed: false, purchaseAllowed: true
        ), cursor: 1))
        #expect(subject.screen == .apps)
        subject.apply(event(ProductEntitlementProjection(
            phase: "trial_expired", successfulDays: 4,
            factoryMutationsAllowed: false, purchaseAllowed: false
        ), cursor: 2))
        #expect(subject.screen == .apps)
        subject.apply(event(ProductEntitlementProjection(
            phase: "pro_yearly", successfulDays: 5,
            factoryMutationsAllowed: true, purchaseAllowed: false
        ), cursor: 3))
        #expect(subject.screen == .apps)
    }

    @MainActor
    @Test("Cable bootstrap shows recovery words once before private pairing")
    func cableBootstrap() async {
        let backend = StubBackend(shots: [shot(version: 1)])
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone")
        #expect(subject.screen == .loading)
        await subject.bootstrapFromCable(URL(string: "tohseno://pair/v1/fixture")!)
        #expect(subject.recoveryWords?.split(separator: " ").count == 12)
        #expect(await backend.pairedInvitations.isEmpty)
        await subject.confirmRecoveryWords()
        #expect(await backend.pairedInvitations == ["tohseno://pair/v1/fixture"])
        #expect(subject.screen == .apps)
        #expect(subject.recoveryWords == nil, "recovery words are shown once, not kept on screen")
        #expect(subject.notice == nil)
    }

    @MainActor
    @Test("A paired Companion reconnects its live channel after relaunch")
    func relaunchReconnects() async {
        let backend = StubBackend(shots: [shot(version: 1)])
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone")

        await subject.refresh()

        #expect(subject.screen == .apps)
        #expect(await backend.reconciles == 1)
        #expect(await backend.synchronizations == 1)
    }

    @MainActor
    @Test("An invalid cable payload is refused without pairing")
    func badPairingCode() async {
        let subject = CompanionModel(backend: StubBackend(), deviceName: "Test iPhone")
        await subject.pair(scanned: "https://example.com/not-a-pairing-code")
        #expect(subject.screen == .firstRun)
        #expect(subject.notice == "The private connection was invalid. Reconnect this iPhone to your Mac.")
    }

    @MainActor
    @Test("An unreachable Mac gives an actionable network message")
    func unreachablePairingMac() async {
        let backend = StubBackend()
        await backend.rejectNextPairing(with: .transportUnavailable)
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone")

        await subject.pair(scanned: "tohseno://pair/v1/fixture")

        #expect(subject.notice == "Tohseno couldn’t reach the development relay on your Mac. The USB cable installs and debugs the app, but it doesn’t carry Companion messages. Connect both devices to the same Wi‑Fi, then try again.")
    }

    @MainActor
    @Test("Pairing failures distinguish expired, clock, used, and refused codes")
    func pairingFailureMessages() {
        #expect(CompanionModel.humanPairingFailure(TohsenoCompanionError.invitationExpired)
            == "The private connection expired. Reconnect this iPhone to your Mac.")
        #expect(CompanionModel.humanPairingFailure(TohsenoCompanionError.invitationNotYetValid)
            == "Your iPhone and Mac clocks don’t agree. Set Date & Time to automatic on both devices, then try again.")
        #expect(CompanionModel.humanPairingFailure(TohsenoCompanionError.relayFailure(409))
            == "The private connection expired or was already used. Reconnect this iPhone to your Mac.")
        #expect(CompanionModel.humanPairingFailure(TohsenoCompanionError.relayFailure(500))
            == "Your Mac refused the private connection. Reconnect this iPhone and try again.")
    }

    @Test("Screenshots are recognized by their own bytes")
    func screenshots() {
        // A one-pixel PNG: signature, then a well-formed IHDR chunk.
        let png = Data(
            [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
                + [0x00, 0x00, 0x00, 0x0D] + Array("IHDR".utf8)
                + [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01]
        )
        let jpegSignature = Data([0xFF, 0xD8, 0xFF] + Array(repeating: 0, count: 32))
        #expect(CompanionAttachments.mediaType(of: png) == "image/png")
        #expect(CompanionAttachments.mediaType(of: jpegSignature) == "image/jpeg")
        #expect(CompanionAttachments.mediaType(of: Data("not an image".utf8)) == nil)
        #expect(CompanionAttachments.blob(from: png, index: 0)?.originName == "screenshot-1.png")
        #expect(CompanionAttachments.blob(from: Data("no".utf8), index: 0) == nil)
        // A file that only claims to be an image is refused by the SDK's own
        // structural check, so it never reaches the Mac.
        #expect(CompanionAttachments.blob(from: jpegSignature, index: 0) == nil)
        #expect(CompanionAttachments.maximumCount == 8)
    }
}
