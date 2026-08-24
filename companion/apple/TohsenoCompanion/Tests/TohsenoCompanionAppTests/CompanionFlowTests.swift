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
    private(set) var creations: [CreateShotRequest] = []
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

    func requestShotCreation(_ request: CreateShotRequest) async throws -> CommandReceipt {
        if let failure {
            self.failure = nil
            throw failure
        }
        creations.append(request)
        if !reachable { unacknowledged += 1 }
        return CommandReceipt(commandID: request.commandID, state: .received)
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

@MainActor
func model(_ backend: StubBackend) async -> CompanionModel {
    let model = CompanionModel(backend: backend, deviceName: "Test iPhone")
    await model.refresh()
    return model
}

@Suite("Create or choose app → one intent → App")
struct CompanionFlowTests {
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
    @Test("Entitlement replaces the product and unlocks only from a signed Mac event")
    func entitlementScreens() async throws {
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
        #expect(subject.screen == .entitlementDecision)
        subject.apply(event(ProductEntitlementProjection(
            phase: "trial_expired", successfulDays: 4,
            factoryMutationsAllowed: false, purchaseAllowed: false
        ), cursor: 2))
        #expect(subject.screen == .trialEnded)
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

        #expect(subject.notice == "TOHSENO couldn’t reach the development relay on your Mac. The USB cable installs and debugs the app, but it doesn’t carry Companion messages. Connect both devices to the same Wi‑Fi, then try again.")
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
