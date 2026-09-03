import CryptoKit
import XCTest
@testable import TohsenoWorkshopKit

final class WorkshopKitTests: XCTestCase {
    func testDeviceIdentityAndCapabilityTruthStaySeparate() throws {
        let id = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_phone"))
        let camera = WorkshopCapabilityState(
            capability: .camera,
            hardware: .available,
            permission: .notRequested,
            reachable: true,
            authorized: true
        )
        let device = WorkshopDevice(
            id: id,
            name: "This iPhone",
            platform: .iPhone,
            connection: .connected,
            capabilities: [camera]
        )
        XCTAssertEqual(device.id, id)
        XCTAssertFalse(try XCTUnwrap(device.capability(.camera)).ready)
        XCTAssertEqual(device.capability(.camera)?.explanation, "Permission not requested")
    }

    func testFocusedShotNeedsNoWorkshopDeclaration() {
        let resolution = WorkshopResolver.resolve(declaration: nil, devices: [])
        XCTAssertEqual(resolution.mode, .focused)
        XCTAssertTrue(resolution.runnable)
    }

    func testRequiredCapabilityFailsClosedUntilReady() throws {
        let id = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_mac"))
        let mac = WorkshopDevice.mac(id: id, name: "This Mac", intelligenceReady: false)
        let declaration = WorkshopShotDeclaration(surfaces: [
            WorkshopSurfaceDeclaration(
                role: "server",
                platform: .macOS,
                required: [.compute, .intelligence]
            ),
        ], session: .init(realtime: true))
        let resolution = WorkshopResolver.resolve(declaration: declaration, devices: [mac])
        XCTAssertFalse(resolution.runnable)
        XCTAssertEqual(resolution.surfaces.first?.missingRequiredCapabilities, [.intelligence])
    }

    func testTypedEventRoundTripAndVersionRejection() throws {
        let sessionID = try XCTUnwrap(WorkshopSessionID(rawValue: "workshop_test"))
        let sender = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_mac"))
        let event = try WorkshopEvent(type: "controller.button", payload: Data([1, 2, 3]))
        let envelope = WorkshopEnvelope(
            sessionID: sessionID,
            senderDeviceID: sender,
            sequence: 1,
            event: event
        )
        let key = Data(repeating: 7, count: 32)
        let sealed = try WorkshopSessionCrypto.seal(
            envelope,
            sessionKey: key,
            direction: "mac-to-companion"
        )
        let opened = try WorkshopSessionCrypto.open(
            sealed,
            sessionKey: key,
            direction: "mac-to-companion",
            expectedSessionID: sessionID,
            afterSequence: 0
        )
        XCTAssertEqual(opened, envelope)
        XCTAssertThrowsError(try WorkshopEnvelope(
            schema: "tohseno.workshop-envelope/2",
            sessionID: sessionID,
            senderDeviceID: sender,
            sequence: 1,
            event: event
        ).validate())
    }

    func testSessionRejectsAuthorityActionsAndReplay() throws {
        XCTAssertThrowsError(try WorkshopEvent(type: "claim.authorize"))
        XCTAssertThrowsError(try WorkshopEvent(type: "install.begin"))

        XCTAssertFalse(try WorkshopEvent(type: "workshop.device.snapshot").isApplicationEvent)
        XCTAssertTrue(try WorkshopEvent(type: "controller.button").isApplicationEvent)

        let sessionID = try XCTUnwrap(WorkshopSessionID(rawValue: "workshop_test"))
        let sender = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_mac"))
        let envelope = WorkshopEnvelope(
            sessionID: sessionID,
            senderDeviceID: sender,
            sequence: 1,
            event: try WorkshopEvent(type: "workshop.pulse")
        )
        let key = Data(repeating: 3, count: 32)
        let sealed = try WorkshopSessionCrypto.seal(
            envelope,
            sessionKey: key,
            direction: "mac-to-companion"
        )
        XCTAssertThrowsError(try WorkshopSessionCrypto.open(
            sealed,
            sessionKey: key,
            direction: "mac-to-companion",
            expectedSessionID: sessionID,
            afterSequence: 1
        ))
    }

    func testPairedHandshakeCredentialAndProof() throws {
        let studio = Curve25519.Signing.PrivateKey()
        let companion = Curve25519.Signing.PrivateKey()
        let studioID = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_mac"))
        let companionID = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_phone"))
        let sessionID = try XCTUnwrap(WorkshopSessionID(rawValue: "workshop_test"))
        let now = Date()
        let unsigned = WorkshopHostCredential(
            workspaceID: "workspace_test",
            studioDeviceID: studioID,
            sessionID: sessionID,
            challenge: WorkshopBase64URL.encode(Data(repeating: 4, count: 32)),
            issuedAt: WorkshopTimestamp.format(now),
            expiresAt: WorkshopTimestamp.format(now.addingTimeInterval(120)),
            signature: "pending"
        )
        let signature = try studio.signature(for: WorkshopHostCredential.domainMessage(
            WorkshopHostCredential.signatureDomain,
            unsigned.signingBody()
        ))
        let host = WorkshopHostCredential(
            workspaceID: unsigned.workspaceID,
            studioDeviceID: studioID,
            sessionID: sessionID,
            challenge: unsigned.challenge,
            issuedAt: unsigned.issuedAt,
            expiresAt: unsigned.expiresAt,
            signature: WorkshopBase64URL.encode(signature)
        )
        XCTAssertNoThrow(try host.verify(
            studioSigningPublicKey: studio.publicKey.rawRepresentation,
            expectedWorkspaceID: "workspace_test",
            expectedStudioDeviceID: studioID,
            now: now
        ))

        let unsignedProof = WorkshopClientProof(
            sessionID: sessionID,
            companionDeviceID: companionID,
            revocationEpoch: 2,
            hostCredentialDigest: WorkshopBase64URL.encode(host.digest()),
            clientNonce: WorkshopBase64URL.encode(Data(repeating: 8, count: 32)),
            signature: "pending"
        )
        let proofSignature = try companion.signature(for: WorkshopHostCredential.domainMessage(
            WorkshopClientProof.signatureDomain,
            unsignedProof.signingBody()
        ))
        let proof = WorkshopClientProof(
            sessionID: sessionID,
            companionDeviceID: companionID,
            revocationEpoch: unsignedProof.revocationEpoch,
            hostCredentialDigest: unsignedProof.hostCredentialDigest,
            clientNonce: unsignedProof.clientNonce,
            signature: WorkshopBase64URL.encode(proofSignature)
        )
        XCTAssertNoThrow(try proof.verify(
            host: host,
            companionSigningPublicKey: companion.publicKey.rawRepresentation
        ))
        XCTAssertThrowsError(try proof.verify(
            host: host,
            companionSigningPublicKey: Curve25519.Signing.PrivateKey().publicKey.rawRepresentation
        ))

        let trusted = WorkshopTrustedPeer(
            deviceID: companionID,
            displayName: "This iPhone",
            signingPublicKey: WorkshopBase64URL.encode(companion.publicKey.rawRepresentation),
            sessionKey: WorkshopBase64URL.encode(Data(repeating: 6, count: 32)),
            revocationEpoch: 2
        )
        XCTAssertEqual(
            try WorkshopHandshake.authenticate(proof: proof, host: host, peers: [trusted]),
            trusted
        )
        let stale = WorkshopTrustedPeer(
            deviceID: companionID,
            displayName: trusted.displayName,
            signingPublicKey: trusted.signingPublicKey,
            sessionKey: trusted.sessionKey,
            revocationEpoch: 3
        )
        XCTAssertThrowsError(
            try WorkshopHandshake.authenticate(proof: proof, host: host, peers: [stale])
        ) { error in
            XCTAssertEqual(error as? WorkshopRuntimeError, .revokedDevice)
        }
        XCTAssertThrowsError(
            try WorkshopHandshake.authenticate(proof: proof, host: host, peers: [])
        ) { error in
            XCTAssertEqual(error as? WorkshopRuntimeError, .unpairedDevice)
        }
    }

    func testDisconnectAndReconnectTransitionsStayEphemeral() {
        XCTAssertEqual(
            WorkshopConnectionFlow.afterTransportLoss(recoveryAvailable: true),
            .reconnecting
        )
        XCTAssertEqual(
            WorkshopConnectionFlow.afterTransportLoss(recoveryAvailable: false),
            .unavailable
        )
    }

    func testSessionKeyMatchesRustInteropVector() throws {
        let sessionID = try XCTUnwrap(WorkshopSessionID(rawValue: "workshop_fixture"))
        let deviceID = try XCTUnwrap(WorkshopDeviceID(rawValue: "device_fixture"))
        let key = WorkshopSessionCrypto.deriveSessionKey(
            sharedSecretBytes: Data(repeating: 7, count: 32),
            challenge: Data(repeating: 9, count: 32),
            sessionID: sessionID,
            workspaceID: "workspace_fixture",
            companionDeviceID: deviceID,
            revocationEpoch: 3
        )
        XCTAssertEqual(
            WorkshopBase64URL.encode(key),
            "DgVKhBE1_-3OW6X2_hEnxdM2AtNXxKlm2ZvJa5RYSUg"
        )
    }

    func testPulseStateExplainsEveryConnection() {
        XCTAssertEqual(
            WorkshopPulseState(connection: .connected, peerName: "This iPhone").headline,
            "Connected to This iPhone"
        )
        XCTAssertEqual(
            WorkshopPulseState(connection: .reconnecting).headline,
            "Workshop connection was interrupted; reconnecting"
        )
        XCTAssertEqual(
            WorkshopPulseState(connection: .rejected).headline,
            "This device is not authorized for the Workshop"
        )
    }
}
