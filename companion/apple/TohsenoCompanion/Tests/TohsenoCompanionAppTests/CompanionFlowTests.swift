import Foundation
import Testing
import TohsenoCompanionKit
@testable import TohsenoCompanionApp

/// A stand-in for the Mac. The SDK's own tests cover pairing, envelopes,
/// durability, and reconciliation; these tests cover what the *product* does.
actor StubBackend: CompanionBackend {
    var shots: [ShotSummary]
    var unacknowledged = 0
    var reachable = true
    private(set) var submissions: [EvolutionRequest] = []
    private(set) var reconciles = 0
    private var failure: TohsenoCompanionError?

    let connectionStates: AsyncStream<CompanionConnectionState>
    let events: AsyncStream<WorkspaceEvent>
    private let eventContinuation: AsyncStream<WorkspaceEvent>.Continuation

    init(shots: [ShotSummary] = []) {
        self.shots = shots
        (connectionStates, _) = AsyncStream.makeStream(of: CompanionConnectionState.self)
        (events, eventContinuation) = AsyncStream.makeStream(of: WorkspaceEvent.self)
    }

    func set(shots: [ShotSummary]) { self.shots = shots }
    func set(unacknowledged: Int) { self.unacknowledged = unacknowledged }
    func set(reachable: Bool) { self.reachable = reachable }
    func rejectNext(with error: TohsenoCompanionError) { failure = error }

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
                allowedActions: [.workspaceRead, .shotEvolve],
                revoked: false
            ),
            nextCursor: 0
        )
    }

    func reconcile() async throws {
        reconciles += 1
        if !reachable { throw TohsenoCompanionError.transportUnavailable }
    }

    func iconBytes(for descriptor: IconDescriptor) async throws -> Data? { nil }

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

    func unacknowledgedCommandCount() async throws -> Int { unacknowledged }
    func createIdentity() async throws -> RecoveryPhrase {
        try RecoveryPhrase(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        )
    }

    func pair(invitation: String, displayName: String) async throws {
        if invitation.hasPrefix("tohseno://pair/") { return }
        throw TohsenoCompanionError.invalidInvitation("fixture")
    }

    func startSynchronization() async throws {}
}

@MainActor
func model(_ backend: StubBackend) async -> CompanionModel {
    let model = CompanionModel(backend: backend, deviceName: "Test iPhone")
    await model.refresh()
    return model
}

@Suite("Choose app → what should change → Evolve App")
struct CompanionFlowTests {
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
    @Test("Pairing is one scan and lands directly in Your Apps")
    func pairing() async {
        let backend = StubBackend(shots: [shot(version: 1)])
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone")
        #expect(subject.screen == .firstRun)
        await subject.createIdentity()
        #expect(subject.recoveryWords?.split(separator: " ").count == 12)
        await subject.pair(scanned: "tohseno://pair/v1/fixture")
        #expect(subject.screen == .apps)
        #expect(subject.recoveryWords == nil, "recovery words are shown once, not kept on screen")
        #expect(subject.notice == nil)
    }

    @MainActor
    @Test("A bad code is a sentence, not a protocol error")
    func badPairingCode() async {
        let subject = CompanionModel(backend: StubBackend(), deviceName: "Test iPhone")
        await subject.pair(scanned: "https://example.com/not-a-pairing-code")
        #expect(subject.screen == .firstRun)
        #expect(subject.notice == "That code didn’t work. Show a new one on your Mac and scan again.")
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
