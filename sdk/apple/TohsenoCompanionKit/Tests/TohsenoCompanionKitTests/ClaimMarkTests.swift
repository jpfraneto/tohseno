import Foundation
import XCTest
@testable import TohsenoCompanionKit

final class ClaimMarkTests: XCTestCase {
    func testReleasedClaimsCoordinatesMatchSignedActivation() throws {
        XCTAssertEqual(
            ClaimsClientActivation.claimsContract,
            "0x5012703d48d99224ac0035d58bc373de9e8b1934"
        )
        XCTAssertEqual(
            ClaimsClientActivation.activationSigningDigest,
            "0xec418380f588b9a6f72fc251b7a0ae7bee8a19a1d843017e4733ebd2d094966d"
        )
    }

    func testSharedClaimMarkVectorsMatchRustExactly() throws {
        let fixture = try loadClaimMarkFixture()
        XCTAssertEqual(fixture.schema, "tohseno.claim-mark-vectors/1")
        XCTAssertEqual(fixture.vectors.count, 9)
        for vector in fixture.vectors {
            do {
                let mark: ClaimMark
                if vector.kind == "accessibility_hold" {
                    mark = .accessibilityHold()
                } else {
                    let canvas = try XCTUnwrap(vector.canvas)
                    mark = try ClaimMark(
                        stroke: try XCTUnwrap(vector.points),
                        canvasWidth: canvas.width,
                        canvasHeight: canvas.height
                    )
                }
                XCTAssertTrue(vector.accepted, vector.id)
                XCTAssertNil(vector.error, vector.id)
                XCTAssertEqual(mark.canonicalBytes.hex, vector.canonicalHex, vector.id)
                XCTAssertEqual(mark.gestureCommitment.hex, vector.gestureCommitment, vector.id)
                XCTAssertEqual(try ClaimMark(canonicalBytes: mark.canonicalBytes), mark, vector.id)
            } catch let error as ClaimMarkError {
                XCTAssertFalse(vector.accepted, vector.id)
                XCTAssertEqual(error.code, vector.error, vector.id)
                XCTAssertNil(vector.canonicalHex, vector.id)
                XCTAssertNil(vector.gestureCommitment, vector.id)
            }
        }
    }

    func testAccessibilityEncodingRejectsFabricatedGeometry() throws {
        var bytes = ClaimMark.accessibilityHold().canonicalBytes
        bytes[bytes.index(before: bytes.endIndex)] ^= 1
        XCTAssertThrowsError(try ClaimMark(canonicalBytes: bytes))
    }

    func testSharedClaimActionVectorsMatchSolidityAndRustExactly() throws {
        let fixture = try loadClaimActionFixture()
        XCTAssertEqual(fixture.schema, "tohseno.claim-action-vectors/1")
        XCTAssertEqual(ClaimsActionEncoding.openTypeHashHex, fixture.openClaimEdition.typeHash)
        XCTAssertEqual(ClaimsActionEncoding.claimTypeHashHex, fixture.claimSoftware.typeHash)
        XCTAssertEqual(
            try ClaimsActionEncoding.domainSeparator(
                chainID: fixture.domain.chainID,
                claimsContract: fixture.domain.verifyingContract
            ).hex,
            fixture.domainSeparator
        )
        XCTAssertEqual(
            try fixture.openClaimEdition.action.structHash(expectedRegistry: fixture.shotRegistry).hex,
            fixture.openClaimEdition.structHash
        )
        XCTAssertEqual(
            try fixture.openClaimEdition.action.digest(
                chainID: fixture.domain.chainID,
                claimsContract: fixture.domain.verifyingContract,
                expectedRegistry: fixture.shotRegistry
            ).hex,
            fixture.openClaimEdition.digest
        )
        XCTAssertEqual(
            try fixture.claimSoftware.action.structHash(expectedRegistry: fixture.shotRegistry).hex,
            fixture.claimSoftware.structHash
        )
        XCTAssertEqual(
            try fixture.claimSoftware.action.digest(
                chainID: fixture.domain.chainID,
                claimsContract: fixture.domain.verifyingContract,
                expectedRegistry: fixture.shotRegistry
            ).hex,
            fixture.claimSoftware.digest
        )
    }
}

private struct ClaimMarkFixture: Decodable {
    let schema: String
    let vectors: [ClaimMarkVector]
}

private struct ClaimMarkVector: Decodable {
    let id: String
    let kind: String
    let canvas: ClaimCanvas?
    let points: [ClaimMarkPoint]?
    let accepted: Bool
    let error: String?
    let canonicalHex: String?
    let gestureCommitment: String?

    enum CodingKeys: String, CodingKey {
        case id, kind, canvas, points, accepted, error
        case canonicalHex = "canonical_hex"
        case gestureCommitment = "gesture_commitment"
    }
}

private struct ClaimCanvas: Decodable {
    let width: Double
    let height: Double
}

private struct ClaimActionFixture: Decodable {
    let schema: String
    let domain: ClaimActionDomain
    let domainSeparator: String
    let shotRegistry: String
    let openClaimEdition: OpenClaimVector
    let claimSoftware: SoftwareClaimVector

    enum CodingKeys: String, CodingKey {
        case schema, domain
        case domainSeparator = "domain_separator"
        case shotRegistry = "shot_registry"
        case openClaimEdition = "open_claim_edition"
        case claimSoftware = "claim_software"
    }
}

private struct ClaimActionDomain: Decodable {
    let chainID: UInt64
    let verifyingContract: String

    enum CodingKeys: String, CodingKey {
        case chainID = "chain_id"
        case verifyingContract = "verifying_contract"
    }
}

private struct OpenClaimVector: Decodable {
    let typeHash: String
    let action: OpenClaimEditionAction
    let structHash: String
    let digest: String

    enum CodingKeys: String, CodingKey {
        case action, digest
        case typeHash = "type_hash"
        case structHash = "struct_hash"
    }
}

private struct SoftwareClaimVector: Decodable {
    let typeHash: String
    let action: SoftwareClaimAction
    let structHash: String
    let digest: String

    enum CodingKeys: String, CodingKey {
        case action, digest
        case typeHash = "type_hash"
        case structHash = "struct_hash"
    }
}

private func loadClaimMarkFixture() throws -> ClaimMarkFixture {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0 ..< 10 {
        let candidate = directory.appendingPathComponent("fixtures/claim-mark-v1.json")
        if FileManager.default.fileExists(atPath: candidate.path) {
            return try JSONDecoder().decode(ClaimMarkFixture.self, from: Data(contentsOf: candidate))
        }
        directory.deleteLastPathComponent()
    }
    throw ClaimMarkError.invalidEncoding
}

private func loadClaimActionFixture() throws -> ClaimActionFixture {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0 ..< 10 {
        let candidate = directory.appendingPathComponent("fixtures/claim-actions-v1.json")
        if FileManager.default.fileExists(atPath: candidate.path) {
            return try JSONDecoder().decode(ClaimActionFixture.self, from: Data(contentsOf: candidate))
        }
        directory.deleteLastPathComponent()
    }
    throw ClaimMarkError.invalidEncoding
}

private extension Data {
    var hex: String {
        "0x" + map { String(format: "%02x", $0) }.joined()
    }
}

private extension ClaimMarkError {
    var code: String {
        switch self {
        case .invalidCanvas: "invalid_canvas"
        case .invalidPoint: "invalid_point"
        case .tooShort: "too_short"
        case .openStroke: "open_stroke"
        case .doesNotEncloseArtifact: "does_not_enclose_artifact"
        case .invalidEncoding: "invalid_encoding"
        }
    }
}
