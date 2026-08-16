import Foundation
import CryptoKit
import XCTest
@testable import TohsenoCompanionKit

final class ClientTests: XCTestCase {
    private let instant = try! CompanionTimestamp.parse("2026-08-15T12:01:00Z")

    func testPairSnapshotOfflineOutboxExactlyOnceAndRevocation() async throws {
        let clock = LockedClock(instant)
        let phone = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 0, count: 16))
        )
        let studio = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 1, count: 16))
        )
        let endpoint = try RelayEndpoint(
            id: "official-v1",
            baseURL: URL(string: "http://127.0.0.1:3100")!,
            allowLoopbackHTTP: true
        )
        let allowlist = try RelayAllowlist([endpoint])
        let invitation = try signedInvitation(studio: studio)
        let grant = try signedGrant(studio: studio, phone: phone)
        let responseMailbox = String(repeating: "a", count: 32)
        let commandMailbox = String(repeating: "b", count: 32)
        let commandWrite = Base64URL.encode(Data(repeating: 44, count: 32))
        let grantPackage = CompanionPairingGrantPackage(
            capabilityGrant: grant,
            studioAgreementPublicKey: studio.description.agreementPublicKey,
            commandMailboxID: commandMailbox,
            commandMailboxWriteCapability: commandWrite
        )
        let grantEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 1,
            envelopeID: "11111111-1111-4111-8111-111111111111",
            plaintext: StrictJSON.encode(grantPackage)
        )
        let snapshot = fixtureSnapshot(phone: phone, grant: grant)
        let snapshotEvent = WorkspaceEvent(
            eventID: "event_snapshot",
            workspaceID: grant.workspaceID,
            cursor: 1,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .workspaceSnapshot(snapshot)
        )
        let snapshotEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 2,
            envelopeID: "22222222-2222-4222-8222-222222222222",
            plaintext: StrictJSON.encode(snapshotEvent)
        )
        let iconBlob = try fixtureIconBlob()
        let iconEvent = WorkspaceEvent(
            eventID: "event_icon",
            workspaceID: grant.workspaceID,
            cursor: 2,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .iconBlob(iconBlob)
        )
        let iconEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 3,
            envelopeID: "55555555-5555-4555-8555-555555555555",
            plaintext: StrictJSON.encode(iconEvent)
        )
        let relay = FakeRelay(
            mailboxID: responseMailbox,
            envelopes: [
                RelayMailboxEnvelope(cursor: 1, envelope: grantEnvelope),
                RelayMailboxEnvelope(cursor: 2, envelope: snapshotEnvelope),
                RelayMailboxEnvelope(cursor: 3, envelope: iconEnvelope),
            ]
        )
        let secrets = InMemoryCompanionSecretStore()
        let durableState = InMemoryCompanionStateStore()
        let durablePayloads = InspectablePayloadStore()
        let client = TohsenoCompanionClient(
            identityStore: secrets,
            stateStore: durableState,
            payloadStore: durablePayloads,
            relay: relay,
            relayAllowlist: allowlist,
            entropySource: DeterministicEntropy(),
            now: { clock.value() }
        )
        let phrase = try await client.createIdentity()
        XCTAssertEqual(phrase.rawEntropy, Data(repeating: 0, count: 16))
        try await client.pair(with: try invitationURI(invitation), displayName: "Fixture iPhone")
        let synchronized = try await client.currentWorkspace()
        XCTAssertEqual(
            synchronized,
            fixtureSnapshot(phone: phone, grant: grant, nextCursor: 3)
        )
        XCTAssertFalse(String(data: try StrictJSON.encode(synchronized), encoding: .utf8)!.contains("source"))
        let descriptor = try XCTUnwrap(synchronized.shots.first?.icon)
        let synchronizedIcon = try await client.iconBlob(for: descriptor)
        XCTAssertEqual(synchronizedIcon, iconBlob)
        let storedState = await durableState.load()
        let encryptedState = try XCTUnwrap(storedState)
        XCTAssertFalse(String(decoding: encryptedState, as: UTF8.self).contains("iVBOR"))

        // Retention loss is reconciled through an ordinary signed,
        // idempotent workspace.snapshot.request. The relay never fabricates
        // state and the Mac responds with a fresh authoritative snapshot.
        await relay.setReset(resetBefore: 5, head: 5)
        try await client.reconcile()
        let snapshotRequestEnvelope = try await relay.upload(at: 0)
        let snapshotRequestPlaintext = try await CompanionEnvelopeCrypto.open(
            snapshotRequestEnvelope,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            expectedMailboxID: commandMailbox,
            recipient: studio,
            now: instant,
            replay: try CompanionReplayProtection(capacity: 128)
        )
        let snapshotRequest = try StrictJSON.decode(
            CompanionCommand.self,
            from: snapshotRequestPlaintext
        )
        try snapshotRequest.verify(
            expectedSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedDeviceID: phone.description.deviceID,
            now: instant
        )
        XCTAssertEqual(snapshotRequest.payload, .workspaceSnapshotRequest)

        let refreshedSnapshot = fixtureSnapshot(
            phone: phone,
            grant: grant,
            snapshotVersion: 2,
            nextCursor: 3
        )
        let refreshedEvent = WorkspaceEvent(
            eventID: "event_refreshed_snapshot",
            workspaceID: grant.workspaceID,
            cursor: 2,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .workspaceSnapshot(refreshedSnapshot)
        )
        await relay.clearReset()
        try await relay.append(RelayMailboxEnvelope(
            cursor: 6,
            envelope: studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 4,
                envelopeID: "66666666-6666-4666-8666-666666666666",
                plaintext: StrictJSON.encode(refreshedEvent)
            )
        ))
        let refreshedIconEvent = WorkspaceEvent(
            eventID: "event_refreshed_icon",
            workspaceID: grant.workspaceID,
            cursor: 3,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .iconBlob(iconBlob)
        )
        try await relay.append(RelayMailboxEnvelope(
            cursor: 7,
            envelope: studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 5,
                envelopeID: "77777777-7777-4777-8777-777777777777",
                plaintext: StrictJSON.encode(refreshedIconEvent)
            )
        ))
        try await client.reconcile()
        let reconciledSnapshot = try await client.currentWorkspace()
        XCTAssertEqual(
            reconciledSnapshot,
            fixtureSnapshot(phone: phone, grant: grant, snapshotVersion: 2, nextCursor: 4)
        )

        let feedback = FeedbackRequest(
            commandID: "command_feedback",
            shotID: "shot_fixture",
            expressionID: "expression_fixture",
            versionID: "version_fixture",
            versionOrdinal: 3,
            body: "Make the accepted version clearer."
        )
        let firstReceipt = try await client.submitFeedback(feedback)
        XCTAssertEqual(firstReceipt.state, .received)
        let firstUploadCount = await relay.uploadCount()
        XCTAssertEqual(firstUploadCount, 2)
        let attemptsBeforeDuplicate = await relay.uploadAttemptCount()
        let duplicateReceipt = try await client.submitFeedback(feedback)
        XCTAssertEqual(duplicateReceipt.commandID, feedback.commandID)
        let duplicateUploadCount = await relay.uploadCount()
        XCTAssertEqual(duplicateUploadCount, 2, "same idempotency key must reuse the durable envelope")
        let attemptsAfterDuplicate = await relay.uploadAttemptCount()
        XCTAssertEqual(attemptsAfterDuplicate, attemptsBeforeDuplicate + 2)

        let receipt = CommandReceipt(commandID: feedback.commandID, state: .completed, resultID: "feedback_result")
        let receiptEvent = WorkspaceEvent(
            eventID: "event_receipt",
            workspaceID: grant.workspaceID,
            cursor: 4,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .commandAcknowledged(receipt)
        )
        try await relay.append(RelayMailboxEnvelope(
            cursor: 8,
            envelope: studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 6,
                envelopeID: "33333333-3333-4333-8333-333333333333",
                plaintext: StrictJSON.encode(receiptEvent)
            )
        ))
        try await client.reconcile()

        await relay.setOffline(true)
        _ = try await client.submitMarketingNote(MarketingRequest(
            commandID: "command_marketing",
            noteID: "note_fixture",
            shotID: "shot_fixture",
            body: "Private campaign thought."
        ))
        let reference = try CompanionReferenceBlob(
            blobID: "reference_fixture",
            originName: "reference.png",
            mediaType: "image/png",
            bytes: iconBlob.bytes
        )
        _ = try await client.requestEvolution(EvolutionRequest(
            commandID: "command_evolution",
            shotID: "shot_fixture",
            baseExpressionID: "expression_fixture",
            baseVersionID: "version_fixture",
            baseVersionOrdinal: 3,
            intention: "Make the flow faster.",
            references: [reference]
        ))
        _ = try await client.requestShotCreation(CreateShotRequest(
            commandID: "command_creation",
            suggestedName: "fixture-child",
            intention: "Create from the exact reference.",
            references: [reference]
        ))
        let offlineUploadCount = await relay.uploadCount()
        XCTAssertEqual(offlineUploadCount, 2)

        // Simulate termination and launch: the new client decrypts and flushes
        // the same durable envelopes instead of signing duplicate commands.
        await relay.setOffline(false)
        let relaunched = TohsenoCompanionClient(
            identityStore: secrets,
            stateStore: durableState,
            payloadStore: durablePayloads,
            relay: relay,
            relayAllowlist: allowlist,
            entropySource: DeterministicEntropy(),
            now: { clock.value() }
        )
        try await relaunched.reconcile()
        let restoredIcon = try await relaunched.iconBlob(for: descriptor)
        XCTAssertEqual(restoredIcon, iconBlob)
        let reconnectedUploadCount = await relay.uploadCount()
        XCTAssertEqual(reconnectedUploadCount, 7)

        // The exact reference is an ordinary signed/encrypted mailbox payload
        // and is uploaded before the command that commits to its descriptor.
        let referenceEnvelope = try await relay.upload(at: 3)
        let creationReferenceEnvelope = try await relay.upload(at: 5)
        let commandEnvelope = try await relay.upload(at: 4)
        let creationCommandEnvelope = try await relay.upload(at: 6)
        let outboundReplay = try CompanionReplayProtection(capacity: 128)
        let referencePlaintext = try await CompanionEnvelopeCrypto.open(
            referenceEnvelope,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            expectedMailboxID: commandMailbox,
            recipient: studio,
            now: instant,
            replay: outboundReplay
        )
        let chunk = try StrictJSON.decode(
            CompanionReferenceBlobChunk.self,
            from: referencePlaintext,
            maximumBytes: CompanionLimits.maximumEnvelopeBodyBytes
        )
        var assembler = CompanionReferenceBlobAssembler()
        XCTAssertEqual(try assembler.admit(chunk), .complete(reference))
        let commandPlaintext = try await CompanionEnvelopeCrypto.open(
            commandEnvelope,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            expectedMailboxID: commandMailbox,
            recipient: studio,
            now: instant,
            replay: outboundReplay
        )
        let evolutionCommand = try StrictJSON.decode(CompanionCommand.self, from: commandPlaintext)
        guard case let .shotEvolveRequest(_, _, _, _, _, _, references) = evolutionCommand.payload else {
            return XCTFail("expected evolution command")
        }
        XCTAssertEqual(references, [reference.descriptor])

        let creationReferencePlaintext = try await CompanionEnvelopeCrypto.open(
            creationReferenceEnvelope,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            expectedMailboxID: commandMailbox,
            recipient: studio,
            now: instant,
            replay: outboundReplay
        )
        XCTAssertEqual(
            try StrictJSON.decode(
                CompanionReferenceBlobChunk.self,
                from: creationReferencePlaintext,
                maximumBytes: CompanionLimits.maximumEnvelopeBodyBytes
            ),
            chunk
        )
        let creationCommandPlaintext = try await CompanionEnvelopeCrypto.open(
            creationCommandEnvelope,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            expectedMailboxID: commandMailbox,
            recipient: studio,
            now: instant,
            replay: outboundReplay
        )
        let creationCommand = try StrictJSON.decode(CompanionCommand.self, from: creationCommandPlaintext)
        guard case let .shotCreateRequest(_, _, creationReferences) = creationCommand.payload else {
            return XCTFail("expected creation command")
        }
        XCTAssertEqual(creationReferences, [reference.descriptor])

        _ = try await relaunched.requestEvolution(EvolutionRequest(
            commandID: "command_evolution",
            shotID: "shot_fixture",
            baseExpressionID: "expression_fixture",
            baseVersionID: "version_fixture",
            baseVersionOrdinal: 3,
            intention: "Make the flow faster.",
            references: [reference]
        ))
        _ = try await relaunched.requestShotCreation(CreateShotRequest(
            commandID: "command_creation",
            suggestedName: "fixture-child",
            intention: "Create from the exact reference.",
            references: [reference]
        ))
        try await relaunched.reconcile()
        let stableUploadCount = await relay.uploadCount()
        XCTAssertEqual(stableUploadCount, 7, "uploaded reference chunks and commands are not duplicated")
        let retainedPayloadCount = await durablePayloads.storedCount()
        XCTAssertEqual(
            retainedPayloadCount,
            4,
            "uploaded exact chunk ciphertext and local reseal material remain until command receipt"
        )

        // Once the original six-day delivery envelope expires, CompanionKit
        // preserves the signed command and exact canonical chunk bytes, then
        // reseals them under fresh routing metadata. The old envelope ID is
        // never reused as if it were still live.
        let future = instant.addingTimeInterval(6 * 24 * 60 * 60 + 120)
        clock.set(future)
        try await relaunched.reconcile()
        let resealedUploadCount = await relay.uploadCount()
        let resealedPayloadCount = await durablePayloads.storedCount()
        XCTAssertEqual(resealedUploadCount, 13)
        XCTAssertEqual(resealedPayloadCount, 4)
        let resealedReference = try await relay.upload(at: 9)
        XCTAssertNotEqual(resealedReference.envelopeID, referenceEnvelope.envelopeID)
        let resealedPlaintext = try await CompanionEnvelopeCrypto.open(
            resealedReference,
            expectedSenderSigningPublicKey: phone.signingKey.publicKey.rawRepresentation,
            expectedSenderDeviceID: phone.description.deviceID,
            expectedMailboxID: commandMailbox,
            recipient: studio,
            now: future,
            replay: try CompanionReplayProtection(capacity: 128)
        )
        XCTAssertEqual(
            try StrictJSON.decode(
                CompanionReferenceBlobChunk.self,
                from: resealedPlaintext,
                maximumBytes: CompanionLimits.maximumEnvelopeBodyBytes
            ),
            chunk
        )

        let evolutionReceipt = CommandReceipt(
            commandID: "command_evolution",
            state: .accepted,
            resultID: "execution_fixture"
        )
        try await relay.append(RelayMailboxEnvelope(
            cursor: 9,
            envelope: studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 7,
                envelopeID: "88888888-8888-4888-8888-888888888888",
                plaintext: StrictJSON.encode(WorkspaceEvent(
                    eventID: "event_evolution_receipt",
                    workspaceID: grant.workspaceID,
                    cursor: 5,
                    emittedAt: CompanionTimestamp.format(future),
                    payload: .commandAcknowledged(evolutionReceipt)
                )),
                createdAt: CompanionTimestamp.format(future),
                expiresAt: CompanionTimestamp.format(future.addingTimeInterval(24 * 60 * 60))
            )
        ))
        try await relaunched.reconcile()
        let selectivelyRetainedPayloadCount = await durablePayloads.storedCount()
        XCTAssertEqual(
            selectivelyRetainedPayloadCount,
            2,
            "a durable command receipt retires only that command's reference payloads"
        )

        // Commands older than the protocol's 30-day offline-admission limit
        // are neither re-signed nor silently dropped. They remain protected
        // locally, while inbound reconciliation can still deliver revocation.
        let beyondAdmission = instant.addingTimeInterval(31 * 24 * 60 * 60)
        clock.set(beyondAdmission)
        await XCTAssertThrowsErrorAsync {
            _ = try await relaunched.requestShotCreation(CreateShotRequest(
                commandID: "command_creation",
                suggestedName: "fixture-child",
                intention: "Create from the exact reference.",
                references: [reference]
            ))
        }
        try await relaunched.reconcile()
        let expiredButOwnedPayloadCount = await durablePayloads.storedCount()
        XCTAssertEqual(expiredButOwnedPayloadCount, 2)

        let revocation = WorkspaceEvent(
            eventID: "event_revoked",
            workspaceID: grant.workspaceID,
            cursor: 6,
            emittedAt: CompanionTimestamp.format(beyondAdmission),
            payload: .deviceRevoked(deviceID: phone.description.deviceID, revocationEpoch: 1)
        )
        try await relay.append(RelayMailboxEnvelope(
            cursor: 10,
            envelope: studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 8,
                envelopeID: "44444444-4444-4444-8444-444444444444",
                plaintext: StrictJSON.encode(revocation),
                createdAt: CompanionTimestamp.format(beyondAdmission),
                expiresAt: CompanionTimestamp.format(beyondAdmission.addingTimeInterval(24 * 60 * 60))
            )
        ))
        try await relaunched.reconcile()
        let revokedPayloadCount = await durablePayloads.storedCount()
        XCTAssertEqual(revokedPayloadCount, 0)
        await XCTAssertThrowsErrorAsync {
            _ = try await relaunched.submitFeedback(feedback)
        }
    }

    func testPairWaitsForSnapshotPublishedAfterTheGrant() async throws {
        let now = instant
        let phone = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 0, count: 16))
        )
        let studio = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 1, count: 16))
        )
        let invitation = try signedInvitation(studio: studio)
        let grant = try signedGrant(studio: studio, phone: phone)
        let responseMailbox = String(repeating: "a", count: 32)
        let grantPackage = CompanionPairingGrantPackage(
            capabilityGrant: grant,
            studioAgreementPublicKey: studio.description.agreementPublicKey,
            commandMailboxID: String(repeating: "b", count: 32),
            commandMailboxWriteCapability: Base64URL.encode(Data(repeating: 44, count: 32))
        )
        let grantEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 1,
            envelopeID: "11111111-1111-4111-8111-111111111111",
            plaintext: StrictJSON.encode(grantPackage)
        )
        let snapshot = fixtureSnapshot(phone: phone, grant: grant)
        let snapshotEvent = WorkspaceEvent(
            eventID: "event_delayed_snapshot",
            workspaceID: grant.workspaceID,
            cursor: 1,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .workspaceSnapshot(snapshot)
        )
        let snapshotEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 2,
            envelopeID: "22222222-2222-4222-8222-222222222222",
            plaintext: StrictJSON.encode(snapshotEvent)
        )
        let relay = FakeRelay(
            mailboxID: responseMailbox,
            envelopes: [RelayMailboxEnvelope(cursor: 1, envelope: grantEnvelope)]
        )
        await relay.release(
            RelayMailboxEnvelope(cursor: 2, envelope: snapshotEnvelope),
            afterFetchCount: 2
        )
        let client = TohsenoCompanionClient(
            identityStore: InMemoryCompanionSecretStore(),
            stateStore: InMemoryCompanionStateStore(),
            payloadStore: InMemoryCompanionPayloadStore(),
            relay: relay,
            relayAllowlist: try RelayAllowlist([RelayEndpoint(
                id: "official-v1",
                baseURL: URL(string: "http://127.0.0.1:3100")!,
                allowLoopbackHTTP: true
            )]),
            entropySource: DeterministicEntropy(),
            now: { now },
            synchronizationSleep: { _ in }
        )
        _ = try await client.createIdentity()
        try await client.pair(with: try invitationURI(invitation), displayName: "Delayed snapshot")
        let received = try await client.currentWorkspace()
        let fetchCount = await relay.fetchCount()
        XCTAssertEqual(received, snapshot)
        XCTAssertGreaterThanOrEqual(fetchCount, 3)
    }

    func testPushRegistrationAndWakeReconcileAuthenticatedMailbox() async throws {
        let clock = LockedClock(instant)
        let phone = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 0, count: 16))
        )
        let studio = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 1, count: 16))
        )
        let endpoint = try RelayEndpoint(
            id: "official-v1",
            baseURL: URL(string: "http://127.0.0.1:3100")!,
            allowLoopbackHTTP: true
        )
        let invitation = try signedInvitation(studio: studio)
        let grant = try signedGrant(studio: studio, phone: phone)
        let responseMailbox = String(repeating: "a", count: 32)
        let grantPackage = CompanionPairingGrantPackage(
            capabilityGrant: grant,
            studioAgreementPublicKey: studio.description.agreementPublicKey,
            commandMailboxID: String(repeating: "b", count: 32),
            commandMailboxWriteCapability: Base64URL.encode(Data(repeating: 44, count: 32))
        )
        let grantEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 1,
            envelopeID: "11111111-1111-4111-8111-111111111111",
            plaintext: StrictJSON.encode(grantPackage)
        )
        let snapshot = fixtureSnapshot(phone: phone, grant: grant)
        let snapshotEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 2,
            envelopeID: "22222222-2222-4222-8222-222222222222",
            plaintext: StrictJSON.encode(WorkspaceEvent(
                eventID: "event_snapshot",
                workspaceID: grant.workspaceID,
                cursor: 1,
                emittedAt: "2026-08-15T12:01:00Z",
                payload: .workspaceSnapshot(snapshot)
            ))
        )
        let relay = FakeRelay(
            mailboxID: responseMailbox,
            envelopes: [
                RelayMailboxEnvelope(cursor: 1, envelope: grantEnvelope),
                RelayMailboxEnvelope(cursor: 2, envelope: snapshotEnvelope),
            ]
        )
        let client = TohsenoCompanionClient(
            identityStore: InMemoryCompanionSecretStore(),
            stateStore: InMemoryCompanionStateStore(),
            payloadStore: InspectablePayloadStore(),
            relay: relay,
            relayAllowlist: try RelayAllowlist([endpoint]),
            entropySource: DeterministicEntropy(),
            now: { clock.value() }
        )
        _ = try await client.createIdentity()
        try await client.pair(with: try invitationURI(invitation), displayName: "Push Fixture iPhone")

        let token = Data((0 ..< 32).map(UInt8.init))
        try await client.registerForPush(using: FakePushTokenProvider(token: token))
        let registeredPush = await relay.lastPushRegistration()
        let registration = try XCTUnwrap(registeredPush)
        XCTAssertEqual(registration.endpoint, endpoint)
        XCTAssertEqual(registration.mailboxID, responseMailbox)
        XCTAssertEqual(registration.deviceID, phone.description.deviceID)
        XCTAssertEqual(registration.token, token)
        XCTAssertFalse(registration.pushCapability.isEmpty)

        let archiveEvent = WorkspaceEvent(
            eventID: "event_archive",
            workspaceID: grant.workspaceID,
            cursor: 2,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .shotArchive(shotID: "shot_fixture")
        )
        try await relay.append(RelayMailboxEnvelope(
            cursor: 3,
            envelope: studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 3,
                envelopeID: "33333333-3333-4333-8333-333333333333",
                plaintext: StrictJSON.encode(archiveEvent)
            )
        ))
        let workspaceBeforeWake = try await client.currentWorkspace()
        XCTAssertEqual(workspaceBeforeWake.shots.first?.archived, false)
        let fetchesBeforeWake = await relay.fetchCount()

        try await client.handlePushWake()

        let fetchesAfterWake = await relay.fetchCount()
        XCTAssertGreaterThan(fetchesAfterWake, fetchesBeforeWake)
        let workspaceAfterWake = try await client.currentWorkspace()
        XCTAssertEqual(workspaceAfterWake.shots.first?.archived, true)
        let acknowledgedCursor = await relay.lastAcknowledgedCursor()
        XCTAssertEqual(acknowledgedCursor, 3)

        try await client.unregisterPushToken()
        let removedPush = await relay.lastPushRemoval()
        let removal = try XCTUnwrap(removedPush)
        XCTAssertEqual(removal.endpoint, endpoint)
        XCTAssertEqual(removal.mailboxID, responseMailbox)
        XCTAssertEqual(removal.deviceID, phone.description.deviceID)
        XCTAssertEqual(removal.pushCapability, registration.pushCapability)
    }

    func testForegroundLiveSyncDeliversEventsReconnectsCancelsAndConfirmsRevocation() async throws {
        let clock = LockedClock(instant)
        let phone = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 0, count: 16))
        )
        let studio = try CompanionIdentity(
            phrase: RecoveryPhrase(entropy: Data(repeating: 1, count: 16))
        )
        let endpoint = try RelayEndpoint(
            id: "official-v1",
            baseURL: URL(string: "http://127.0.0.1:3100")!,
            allowLoopbackHTTP: true
        )
        let invitation = try signedInvitation(studio: studio)
        let grant = try signedGrant(studio: studio, phone: phone)
        let responseMailbox = String(repeating: "a", count: 32)
        let grantEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 1,
            envelopeID: "11111111-1111-4111-8111-111111111111",
            plaintext: StrictJSON.encode(CompanionPairingGrantPackage(
                capabilityGrant: grant,
                studioAgreementPublicKey: studio.description.agreementPublicKey,
                commandMailboxID: String(repeating: "b", count: 32),
                commandMailboxWriteCapability: Base64URL.encode(Data(repeating: 44, count: 32))
            ))
        )
        let snapshotEnvelope = try studioEnvelope(
            studio: studio,
            phone: phone,
            mailboxID: responseMailbox,
            sequence: 2,
            envelopeID: "22222222-2222-4222-8222-222222222222",
            plaintext: StrictJSON.encode(WorkspaceEvent(
                eventID: "event_snapshot",
                workspaceID: grant.workspaceID,
                cursor: 1,
                emittedAt: "2026-08-15T12:01:00Z",
                payload: .workspaceSnapshot(fixtureSnapshot(phone: phone, grant: grant))
            ))
        )
        let relay = FakeRelay(
            mailboxID: responseMailbox,
            envelopes: [
                RelayMailboxEnvelope(cursor: 1, envelope: grantEnvelope),
                RelayMailboxEnvelope(cursor: 2, envelope: snapshotEnvelope),
            ]
        )
        let delays = DelayRecorder()
        let client = TohsenoCompanionClient(
            identityStore: InMemoryCompanionSecretStore(),
            stateStore: InMemoryCompanionStateStore(),
            payloadStore: InspectablePayloadStore(),
            relay: relay,
            relayAllowlist: try RelayAllowlist([endpoint]),
            entropySource: DeterministicEntropy(),
            now: { clock.value() },
            synchronizationSleep: { nanoseconds in delays.record(nanoseconds) }
        )
        _ = try await client.createIdentity()
        try await client.pair(with: try invitationURI(invitation), displayName: "Live Fixture iPhone")

        let receivedEvents = Task { () -> [String] in
            var identifiers: [String] = []
            for await event in client.workspaceEvents {
                if event.eventID == "event_archive" || event.eventID == "event_execution" {
                    identifiers.append(event.eventID)
                    if identifiers.count == 2 { return identifiers }
                }
            }
            return identifiers
        }
        try await client.startForegroundSynchronization()
        let connectedInitially = await eventually { await relay.liveConnectionCount() == 1 }
        let initialLiveConnections = await relay.liveConnectionCount()
        let initialFetches = await relay.fetchCount()
        XCTAssertTrue(
            connectedInitially,
            "live connections=\(initialLiveConnections), fetches=\(initialFetches)"
        )

        let archiveEvent = WorkspaceEvent(
            eventID: "event_archive",
            workspaceID: grant.workspaceID,
            cursor: 2,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .shotArchive(shotID: "shot_fixture")
        )
        let execution = ExecutionSummary(
            executionID: "execution_live_fixture",
            shotID: "shot_fixture",
            state: .building,
            updatedAt: "2026-08-15T12:01:00Z"
        )
        let executionEvent = WorkspaceEvent(
            eventID: "event_execution",
            workspaceID: grant.workspaceID,
            cursor: 3,
            emittedAt: "2026-08-15T12:01:00Z",
            payload: .executionUpdated(execution)
        )
        let archiveMailboxEnvelope = RelayMailboxEnvelope(
            cursor: 3,
            envelope: try studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 3,
                envelopeID: "33333333-3333-4333-8333-333333333333",
                plaintext: StrictJSON.encode(archiveEvent)
            )
        )
        let executionMailboxEnvelope = RelayMailboxEnvelope(
            cursor: 4,
            envelope: try studioEnvelope(
                studio: studio,
                phone: phone,
                mailboxID: responseMailbox,
                sequence: 4,
                envelopeID: "44444444-4444-4444-8444-444444444444",
                plaintext: StrictJSON.encode(executionEvent)
            )
        )
        await relay.append(archiveMailboxEnvelope)
        await relay.append(executionMailboxEnvelope)
        await relay.emitLive(.envelope(executionMailboxEnvelope))
        let synchronizedLiveEvents = await eventually {
            guard let workspace = try? await client.currentWorkspace() else { return false }
            return workspace.shots.first?.archived == true
                && workspace.activeExecutions.first == execution
        }
        XCTAssertTrue(synchronizedLiveEvents)
        let deliveredEventIDs = await receivedEvents.value
        XCTAssertEqual(Set(deliveredEventIDs), ["event_archive", "event_execution"])

        await relay.finishNextLiveConnectionsImmediately(8)
        await relay.finishLive()
        let reconnected = await eventually { await relay.liveConnectionCount() == 10 }
        XCTAssertTrue(reconnected)
        XCTAssertEqual(
            Array(delays.values().prefix(9)),
            [
                250_000_000, 500_000_000, 1_000_000_000,
                2_000_000_000, 4_000_000_000, 8_000_000_000,
                16_000_000_000, 30_000_000_000, 30_000_000_000,
            ]
        )

        let cancellationsBeforeStop = await relay.liveCancellationCount()
        await client.stopForegroundSynchronization()
        let cancelledCleanly = await eventually {
            await relay.liveCancellationCount() > cancellationsBeforeStop
        }
        XCTAssertTrue(cancelledCleanly)

        try await client.startForegroundSynchronization()
        let restarted = await eventually { await relay.liveConnectionCount() == 11 }
        XCTAssertTrue(restarted)
        await relay.revokeLive(cursor: 5)
        let confirmedRevoked = await eventually {
            do {
                _ = try await client.currentWorkspace()
                return false
            } catch TohsenoCompanionError.capabilityRevoked {
                return true
            } catch {
                return false
            }
        }
        XCTAssertTrue(confirmedRevoked)
        await client.stopForegroundSynchronization()
    }

    private func signedInvitation(studio: CompanionIdentity) throws -> PairingInvitation {
        let ephemeral = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: Data(repeating: 9, count: 32))
        let draft = PairingInvitation(
            sessionID: String(repeating: "p", count: 32),
            workspaceID: "workspace_fixture",
            studioDeviceID: studio.description.deviceID,
            studioSigningPublicKey: studio.description.signingPublicKey,
            studioEphemeralAgreementPublicKey: Base64URL.encode(ephemeral.publicKey.rawRepresentation),
            relayID: "official-v1",
            issuedAt: "2026-08-15T12:00:00Z",
            expiresAt: "2026-08-15T12:02:00Z",
            signature: "pending"
        )
        return PairingInvitation(
            sessionID: draft.sessionID,
            workspaceID: draft.workspaceID,
            studioDeviceID: draft.studioDeviceID,
            studioSigningPublicKey: draft.studioSigningPublicKey,
            studioEphemeralAgreementPublicKey: draft.studioEphemeralAgreementPublicKey,
            relayID: draft.relayID,
            issuedAt: draft.issuedAt,
            expiresAt: draft.expiresAt,
            signature: Base64URL.encode(try studio.sign(
                domain: PairingInvitation.signatureDomain,
                message: draft.canonicalBody()
            ))
        )
    }

    private func signedGrant(studio: CompanionIdentity, phone: CompanionIdentity) throws -> CapabilityGrant {
        let draft = CapabilityGrant(
            capabilityID: "capability_fixture",
            workspaceID: "workspace_fixture",
            deviceID: phone.description.deviceID,
            allowedActions: CompanionCapability.allCases,
            issuedAt: "2026-08-15T12:00:30Z",
            expiresAt: "2026-08-16T12:00:30Z",
            revocationEpoch: 0,
            studioSigningPublicKey: studio.description.signingPublicKey,
            signature: "pending"
        )
        return CapabilityGrant(
            capabilityID: draft.capabilityID,
            workspaceID: draft.workspaceID,
            deviceID: draft.deviceID,
            allowedActions: draft.allowedActions,
            issuedAt: draft.issuedAt,
            expiresAt: draft.expiresAt,
            revocationEpoch: draft.revocationEpoch,
            studioSigningPublicKey: draft.studioSigningPublicKey,
            signature: Base64URL.encode(try studio.sign(
                domain: CapabilityGrant.signatureDomain,
                message: draft.canonicalBody()
            ))
        )
    }

    private func fixtureSnapshot(
        phone: CompanionIdentity,
        grant: CapabilityGrant,
        snapshotVersion: UInt64 = 1,
        nextCursor: UInt64 = 2
    ) -> WorkspaceSnapshot {
        WorkspaceSnapshot(
            workspaceID: grant.workspaceID,
            snapshotVersion: snapshotVersion,
            generatedAt: "2026-08-15T12:01:00Z",
            serviceVersion: "0.9.0",
            shots: [ShotSummary(
                shotID: "shot_fixture",
                displayName: "Fixture",
                kind: .factoryShot,
                icon: IconDescriptor(
                    blobID: "icon_fixture",
                    revision: 7,
                    mediaType: "image/png",
                    byteLength: 68,
                    width: 1,
                    height: 1,
                    placeholder: false
                ),
                iconRevision: 7,
                expressionID: "expression_fixture",
                latestVersionID: "version_fixture",
                latestVersionOrdinal: 3,
                latestVersionCreatedAt: "2026-08-15T11:00:00Z",
                sortIndex: 0,
                supportedCompanionActions: CompanionCapability.allCases
            )],
            activeExecutions: [],
            deviceCapabilityState: DeviceCapabilityState(
                deviceID: phone.description.deviceID,
                capabilityID: grant.capabilityID,
                revocationEpoch: 0,
                allowedActions: grant.allowedActions,
                revoked: false
            ),
            nextCursor: nextCursor
        )
    }

    private func fixtureIconBlob() throws -> CompanionIconBlob {
        let bytes = try XCTUnwrap(Data(base64Encoded:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        ))
        return try CompanionIconBlob(
            blobID: "icon_fixture",
            revision: 7,
            mediaType: "image/png",
            placeholder: false,
            bytes: bytes
        )
    }

    private func studioEnvelope(
        studio: CompanionIdentity,
        phone: CompanionIdentity,
        mailboxID: String,
        sequence: UInt64,
        envelopeID: String,
        plaintext: Data,
        createdAt: String = "2026-08-15T12:01:00Z",
        expiresAt: String = "2026-08-16T12:01:00Z"
    ) throws -> OpaqueCompanionEnvelope {
        try CompanionEnvelopeCrypto.seal(
            sender: studio,
            recipientAgreementPublicKey: phone.agreementKey.publicKey.rawRepresentation,
            metadata: CompanionEnvelopeMetadata(
                envelopeID: envelopeID,
                mailboxID: mailboxID,
                recipientDeviceID: phone.description.deviceID,
                senderSequence: sequence,
                createdAt: createdAt,
                expiresAt: expiresAt
            ),
            plaintext: plaintext,
            ephemeralSecret: Data(repeating: UInt8(20 + sequence), count: 32),
            nonce: Data(repeating: UInt8(40 + sequence), count: 12)
        )
    }

    private func invitationURI(_ invitation: PairingInvitation) throws -> String {
        PairingInvitation.uriPrefix + Base64URL.encode(try invitation.canonicalJSON())
    }
}

private func XCTAssertThrowsErrorAsync(
    _ expression: @escaping () async throws -> Void,
    file: StaticString = #filePath,
    line: UInt = #line
) async {
    do {
        try await expression()
        XCTFail("expected an error", file: file, line: line)
    } catch {}
}

private final class DeterministicEntropy: CompanionEntropySource, @unchecked Sendable {
    private let lock = NSLock()
    private var call: UInt8 = 0

    func randomBytes(count: Int) -> Data {
        lock.lock()
        defer { lock.unlock() }
        let value = call
        call &+= 1
        return Data(repeating: value, count: count)
    }
}

private final class LockedClock: @unchecked Sendable {
    private let lock = NSLock()
    private var instant: Date

    init(_ instant: Date) { self.instant = instant }

    func value() -> Date {
        lock.lock()
        defer { lock.unlock() }
        return instant
    }

    func set(_ value: Date) {
        lock.lock()
        instant = value
        lock.unlock()
    }
}

private final class DelayRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var recorded: [UInt64] = []

    func record(_ nanoseconds: UInt64) {
        lock.lock()
        recorded.append(nanoseconds)
        lock.unlock()
    }

    func values() -> [UInt64] {
        lock.lock()
        defer { lock.unlock() }
        return recorded
    }
}

private func eventually(
    attempts: Int = 2_000,
    _ condition: @escaping @Sendable () async -> Bool
) async -> Bool {
    for _ in 0 ..< attempts {
        if await condition() { return true }
        try? await Task.sleep(nanoseconds: 1_000_000)
    }
    return false
}

private struct FakePushTokenProvider: CompanionPushTokenProvider {
    let token: Data?

    func currentAPNSToken() async throws -> Data? { token }
}

private struct FakePushRegistration: Equatable, Sendable {
    let endpoint: RelayEndpoint
    let mailboxID: String
    let pushCapability: String
    let deviceID: String
    let token: Data
}

private struct FakePushRemoval: Equatable, Sendable {
    let endpoint: RelayEndpoint
    let mailboxID: String
    let pushCapability: String
    let deviceID: String
}

private actor InspectablePayloadStore: CompanionPayloadStore {
    private var values: [String: Data] = [:]

    func load(id: String) throws -> Data? {
        try requireIdentifier(id, field: "test_payload.id")
        return values[id]
    }

    func save(id: String, bytes: Data) throws {
        try requireIdentifier(id, field: "test_payload.id")
        guard !bytes.isEmpty, bytes.count <= CompanionLimits.maximumEnvelopeBodyBytes else {
            throw TohsenoCompanionError.unsafeStorage
        }
        if let existing = values[id], existing != bytes {
            throw TohsenoCompanionError.unsafeStorage
        }
        values[id] = bytes
    }

    func delete(id: String) throws {
        try requireIdentifier(id, field: "test_payload.id")
        values.removeValue(forKey: id)
    }

    func retainOnly(ids: Set<String>) throws {
        for id in ids { try requireIdentifier(id, field: "test_payload.id") }
        values = values.filter { ids.contains($0.key) }
    }

    func deleteAll() { values = [:] }
    func storedCount() -> Int { values.count }
}

private actor FakeRelay: CompanionRelayTransport {
    private let mailboxID: String
    private var envelopes: [RelayMailboxEnvelope]
    private var uploads: [OpaqueCompanionEnvelope] = []
    private var uploadAttempts: [OpaqueCompanionEnvelope] = []
    private var offline = false
    private var reset: (before: UInt64, head: UInt64)?
    private var revoked = false
    private var fetches = 0
    private var acknowledgedCursors: [UInt64] = []
    private var pushRegistrations: [FakePushRegistration] = []
    private var pushRemovals: [FakePushRemoval] = []
    private var liveContinuation: AsyncThrowingStream<RelayLiveEvent, Error>.Continuation?
    private var liveContinuationID: Int?
    private var liveConnections = 0
    private var liveCancellations = 0
    private var immediatelyFinishedLiveConnections = 0
    private var delayedEnvelope: RelayMailboxEnvelope?
    private var delayedEnvelopeFetchCount: Int?

    init(mailboxID: String, envelopes: [RelayMailboxEnvelope]) {
        self.mailboxID = mailboxID
        self.envelopes = envelopes
    }

    func setOffline(_ value: Bool) { offline = value }
    func setReset(resetBefore: UInt64, head: UInt64) {
        reset = (resetBefore, head)
    }
    func clearReset() { reset = nil }
    func append(_ value: RelayMailboxEnvelope) { envelopes.append(value) }
    func release(_ value: RelayMailboxEnvelope, afterFetchCount: Int) {
        delayedEnvelope = value
        delayedEnvelopeFetchCount = afterFetchCount
    }
    func uploadCount() -> Int { uploads.count }
    func uploadAttemptCount() -> Int { uploadAttempts.count }
    func fetchCount() -> Int { fetches }
    func lastAcknowledgedCursor() -> UInt64? { acknowledgedCursors.last }
    func lastPushRegistration() -> FakePushRegistration? { pushRegistrations.last }
    func lastPushRemoval() -> FakePushRemoval? { pushRemovals.last }
    func liveConnectionCount() -> Int { liveConnections }
    func liveCancellationCount() -> Int { liveCancellations }
    func finishNextLiveConnectionsImmediately(_ count: Int) {
        immediatelyFinishedLiveConnections = count
    }
    func emitLive(_ event: RelayLiveEvent) { liveContinuation?.yield(event) }
    func finishLive() { liveContinuation?.finish() }
    func revokeLive(cursor: UInt64) {
        revoked = true
        liveContinuation?.yield(.revoked(cursor: cursor))
        liveContinuation?.finish()
    }
    func upload(at index: Int) throws -> OpaqueCompanionEnvelope {
        guard uploads.indices.contains(index) else {
            throw TohsenoCompanionError.transportUnavailable
        }
        return uploads[index]
    }

    func createMailbox(endpoint: RelayEndpoint, verifiers: RelayMailboxVerifiers) -> RelayCreatedMailbox {
        RelayCreatedMailbox(
            schema: "tohseno.companion-mailbox-created/1",
            mailboxID: mailboxID,
            createdAt: "2026-08-15T12:01:00Z"
        )
    }

    func submitPairingResponse(endpoint: RelayEndpoint, sessionID: String, opaqueResponse: Data) throws {
        let response = try StrictJSON.decode(EncryptedPairingResponse.self, from: opaqueResponse)
        guard response.schema == EncryptedPairingResponse.schemaV1 else {
            throw TohsenoCompanionError.invalidEncoding("pairing response schema")
        }
    }

    func uploadEnvelope(
        endpoint: RelayEndpoint,
        mailboxID: String,
        writeCapability: String,
        envelope: OpaqueCompanionEnvelope
    ) throws -> RelayEnvelopeUploadReceipt {
        if offline { throw TohsenoCompanionError.transportUnavailable }
        uploadAttempts.append(envelope)
        if !uploads.contains(where: { $0.envelopeID == envelope.envelopeID }) { uploads.append(envelope) }
        return RelayEnvelopeUploadReceipt(
            schema: "tohseno.companion-envelope-accepted/1",
            accepted: true,
            duplicate: false,
            cursor: UInt64(uploads.count)
        )
    }

    func fetchEnvelopes(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) throws -> RelayMailboxPage {
        if revoked { throw TohsenoCompanionError.capabilityRevoked }
        if offline { throw TohsenoCompanionError.transportUnavailable }
        fetches += 1
        if let delayedEnvelope, let delayedEnvelopeFetchCount,
           fetches > delayedEnvelopeFetchCount {
            envelopes.append(delayedEnvelope)
            self.delayedEnvelope = nil
            self.delayedEnvelopeFetchCount = nil
        }
        if let reset {
            throw TohsenoCompanionError.cursorResetRequired(
                resetBefore: reset.before,
                head: reset.head
            )
        }
        let page = envelopes.filter { $0.cursor > cursor }.sorted { $0.cursor < $1.cursor }
        return RelayMailboxPage(
            schema: RelayMailboxPage.schemaV1,
            envelopes: page,
            nextCursor: page.last?.cursor ?? cursor,
            headCursor: envelopes.map(\.cursor).max() ?? cursor,
            hasMore: false
        )
    }

    func acknowledge(
        endpoint: RelayEndpoint,
        mailboxID: String,
        acknowledgementCapability: String,
        cursor: UInt64
    ) {
        acknowledgedCursors.append(cursor)
    }

    func liveEvents(
        endpoint: RelayEndpoint,
        mailboxID: String,
        readCapability: String,
        after cursor: UInt64
    ) throws -> AsyncThrowingStream<RelayLiveEvent, Error> {
        if revoked { throw TohsenoCompanionError.capabilityRevoked }
        if offline { throw TohsenoCompanionError.transportUnavailable }
        liveConnections += 1
        let connectionID = liveConnections
        let (stream, continuation) = AsyncThrowingStream.makeStream(
            of: RelayLiveEvent.self,
            throwing: Error.self
        )
        liveContinuation = continuation
        liveContinuationID = connectionID
        continuation.onTermination = { [weak self] _ in
            Task { await self?.recordLiveCancellation(connectionID: connectionID) }
        }
        if immediatelyFinishedLiveConnections > 0 {
            immediatelyFinishedLiveConnections -= 1
            continuation.finish()
        }
        return stream
    }

    private func recordLiveCancellation(connectionID: Int) {
        liveCancellations += 1
        if liveContinuationID == connectionID {
            liveContinuation = nil
            liveContinuationID = nil
        }
    }

    func registerPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String,
        token: Data
    ) {
        pushRegistrations.append(FakePushRegistration(
            endpoint: endpoint,
            mailboxID: mailboxID,
            pushCapability: pushCapability,
            deviceID: deviceID,
            token: token
        ))
    }

    func unregisterPushToken(
        endpoint: RelayEndpoint,
        mailboxID: String,
        pushCapability: String,
        deviceID: String
    ) {
        pushRemovals.append(FakePushRemoval(
            endpoint: endpoint,
            mailboxID: mailboxID,
            pushCapability: pushCapability,
            deviceID: deviceID
        ))
    }
}
