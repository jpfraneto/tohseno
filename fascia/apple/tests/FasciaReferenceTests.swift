import CryptoKit
import Foundation
import Testing
@testable import TohsenoAppleFascia

@Test
func installationIdentityIsStableWithinOneAppAndUnlinkedAcrossApps() async throws {
    #expect(FasciaP256.order.fasciaHex(prefix: false)
        == "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
    #expect(FasciaP256.halfOrder.fasciaHex(prefix: false)
        == "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8")
    let store = MemoryInstallationKeyStore()
    let first = InstallationIdentity(
        applicationIdentifier: "example.first",
        keyStore: store,
        secureEnclaveAvailable: { false }
    )
    let firstReloaded = InstallationIdentity(
        applicationIdentifier: "example.first",
        keyStore: store,
        secureEnclaveAvailable: { false }
    )
    let second = InstallationIdentity(
        applicationIdentifier: "example.second",
        keyStore: store,
        secureEnclaveAvailable: { false }
    )

    let firstKey = try first.prepare()
    #expect(firstKey.backend == .softwareThisDeviceOnly)
    #expect(try firstReloaded.descriptor() == firstKey)
    #expect(try second.prepare().installationID != firstKey.installationID)

    let message = Data("narrow consent".utf8)
    let signature = try first.sign(message: message)
    let x = try #require(Data(fasciaHex: firstKey.x, expectedBytes: 32))
    let y = try #require(Data(fasciaHex: firstKey.y, expectedBytes: 32))
    let r = try #require(Data(fasciaHex: signature.r, expectedBytes: 32))
    let s = try #require(Data(fasciaHex: signature.s, expectedBytes: 32))
    let publicKey = try P256.Signing.PublicKey(
        x963Representation: Data([0x04]) + x + y
    )
    let cryptoSignature = try P256.Signing.ECDSASignature(
        rawRepresentation: r + s
    )
    #expect(publicKey.isValidSignature(cryptoSignature, for: message))
}

@Test
func continuityStatementHasStableRFC8785BytesAndDigest() throws {
    let x = "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    let y = "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
    let installationID = try #require(
        ContinuityStatement.installationID(x: x, y: y)
    )
    let statement = ContinuityStatement(
        issuer: ContinuityIssuer(
            installationID: installationID,
            publicKey: ContinuityPublicKey(x: x, y: y)
        ),
        audience: ContinuityAudience(
            shotID: "0x" + String(repeating: "33", count: 32),
            installationID: nil
        ),
        originatingShotID: "0x" + String(repeating: "11", count: 32),
        claims: ["reading.progress", "theme.preference"],
        nonce: "0x" + String(repeating: "22", count: 32),
        issuedAt: 1_000,
        expiresAt: 2_000
    )
    let canonical = String(decoding: try statement.canonicalJSON(), as: UTF8.self)
    let expected = """
    {"audience":{"installation_id":null,"shot_id":"0x3333333333333333333333333333333333333333333333333333333333333333"},"claims":["reading.progress","theme.preference"],"expires_at":2000,"issued_at":1000,"issuer":{"installation_id":"\(installationID)","public_key":{"x":"\(x)","y":"\(y)"}},"nonce":"0x2222222222222222222222222222222222222222222222222222222222222222","originating_shot_id":"0x1111111111111111111111111111111111111111111111111111111111111111","schema":"tohseno.continuity-statement/1"}
    """
    #expect(canonical == expected)
    #expect(try statement.digest().fasciaHex(prefix: true)
        == "0xf11052f91d94519657cda0e1e53f4f2370f71684fe2f63bad3b0ba7453c86313")
}

@Test
func continuityMatchesRustCanonicalAndNegativeVectors() throws {
    let vectors = try loadProtocolVectors().continuity
    let canonical = vectors.canonical
    try canonical.statement.validate(nowUnix: canonical.statement.issuedAt)
    #expect(
        try canonical.statement.canonicalJSON()
            == Data(canonical.rfc8785.utf8)
    )
    #expect(
        try canonical.statement.digest().fasciaHex(prefix: true)
            == canonical.sha256
    )
    #expect(canonical.envelope.statement == canonical.statement)
    #expect(try canonical.envelope.verify(nowUnix: canonical.statement.issuedAt))

    let transport = try canonical.envelope.transportJSON()
    #expect(
        try ContinuityEnvelope.decodeTransportJSON(transport)
            == canonical.envelope
    )
    var duplicateEnvelope = Data(
        #"{"schema":"tohseno.continuity/1","#.utf8
    )
    duplicateEnvelope.append(transport.dropFirst())
    #expect(throws: FasciaStrictJSONError.duplicateKey) {
        try ContinuityEnvelope.decodeTransportJSON(duplicateEnvelope)
    }

    for boundary in vectors.activeWindow {
        let accepted = (try? canonical.envelope.verify(
            nowUnix: boundary.nowUnix
        )) == true
        #expect(
            accepted == boundary.valid,
            "active-window disagreement at \(boundary.nowUnix)"
        )
    }

    for vector in vectors.validStatements {
        let accepted = (try? vector.statement.canonicalJSON()) != nil
        #expect(accepted, "\(vector.name) unexpectedly failed")
    }

    for vector in vectors.invalidStatements {
        let accepted: Bool
        do {
            let statement = try JSONDecoder().decode(
                ContinuityStatement.self,
                from: Data(vector.statementJSON.utf8)
            )
            accepted = (try? statement.canonicalJSON()) != nil
        } catch {
            accepted = false
        }
        #expect(!accepted, "\(vector.name) unexpectedly passed")
    }
}

@Test
func continuityIsScopedExpiringAndIndependentlyVerifiable() async throws {
    let identity = InstallationIdentity(
        applicationIdentifier: "example.continuity",
        keyStore: MemoryInstallationKeyStore(),
        secureEnclaveAvailable: { false }
    )
    let envelope = try await ContinuityEnvelope.issue(
        identity: identity,
        audience: ContinuityAudience(
            shotID: "0x" + String(repeating: "33", count: 32),
            installationID: "0x" + String(repeating: "44", count: 32)
        ),
        originatingShotID: "0x" + String(repeating: "11", count: 32),
        claims: ["reading.progress", "theme.preference"],
        nonce: Data(repeating: 0x22, count: 32),
        issuedAt: 1_000,
        expiresAt: 2_000,
        nowUnix: 1_000
    )

    #expect(try envelope.verify(nowUnix: 1_001))
    #expect(throws: ContinuityError.expired) {
        try envelope.verify(nowUnix: 2_000)
    }

    let changedStatement = ContinuityStatement(
        issuer: envelope.statement.issuer,
        audience: ContinuityAudience(
            shotID: envelope.statement.audience.shotID,
            installationID: nil
        ),
        originatingShotID: envelope.statement.originatingShotID,
        claims: envelope.statement.claims,
        nonce: envelope.statement.nonce,
        issuedAt: envelope.statement.issuedAt,
        expiresAt: envelope.statement.expiresAt
    )
    let changed = ContinuityEnvelope(
        statement: changedStatement,
        signature: envelope.signature
    )
    #expect(try !changed.verify(nowUnix: 1_001))

    let json = try String(
        decoding: envelope.transportJSON(),
        as: UTF8.self
    )
    #expect(!json.contains("builder_id"))
    #expect(!json.contains("apple_id"))
    #expect(json.contains("originating_shot_id"))
}

@Test
func metadataEnforcesBundleVersionAndDeclaredNetwork() throws {
    let metadata = sampleMetadata(sequence: 2, bundleVersion: 2)
    try metadata.validate()
    let json = try String(
        decoding: metadata.canonicalTransportJSON(),
        as: UTF8.self
    )
    #expect(json.contains(#""protocol":"tohseno""#))
    #expect(json.contains(#""bundle_version":2"#))
    #expect(try TohsenoMetadata.decodeTransportJSON(Data(json.utf8)) == metadata)

    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(sequence: 2, bundleVersion: 3).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            sourceCommit: String(repeating: "a", count: 64)
        ).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            bundleID: "example..app"
        ).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            factoryImplementation: " example/factory"
        ).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            duplicateNetwork: true
        ).validate()
    }
    let zero = "0x" + String(repeating: "00", count: 32)
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            evolutionCommitment: zero
        ).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            sourceTreeSHA256: zero
        ).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            fasciaSHA256: zero
        ).validate()
    }
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            registry: TohsenoRegistryReference(
                chainID: 4_663,
                contract: "0x" + String(repeating: "00", count: 20),
                transaction: zero
            )
        ).validate()
    }
    try sampleMetadata(
        sequence: 2,
        bundleVersion: 2,
        appStoreID: 9_007_199_254_740_991
    ).validate()
    #expect(throws: TohsenoMetadataError.invalid) {
        try sampleMetadata(
            sequence: 2,
            bundleVersion: 2,
            appStoreID: 9_007_199_254_740_992
        ).validate()
    }
}

@Test
func exactEngineMetadataFixtureDecodesUnderNormativeSwift() throws {
    let repositoryRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
    let fixture = try Data(contentsOf: repositoryRoot.appendingPathComponent(
        "protocol/test-vectors/app-metadata-v1.json"
    ))

    // The Rust engine test locks these exact bytes to its production serializer;
    // this side deliberately decodes that same file rather than a Swift copy.
    let metadata = try TohsenoMetadata.decodeTransportJSON(fixture)
    #expect(metadata.schema == "tohseno.app-metadata/1")
    #expect(metadata.shotID
        == "0x0101010101010101010101010101010101010101010101010101010101010101")
    #expect(metadata.distribution.supportedAppleSurfaces == [.iPhone])
    #expect(metadata.capabilities.map(\.capability) == [.localStorage])
    #expect(metadata.network.isEmpty)

    var escapedDuplicate = Data(
        #"{"pro\u0074ocol":"tohseno","#.utf8
    )
    escapedDuplicate.append(fixture.dropFirst())
    #expect(throws: FasciaStrictJSONError.duplicateKey) {
        try TohsenoMetadata.decodeTransportJSON(escapedDuplicate)
    }
    #expect(throws: FasciaStrictJSONError.sizeLimit) {
        try TohsenoMetadata.decodeTransportJSON(
            Data(repeating: 0x20, count: 1024 * 1024 + 1)
        )
    }
    let unknownNestedKey = Data(
        String(decoding: fixture, as: UTF8.self)
            .replacingOccurrences(
                of: #""details": []"#,
                with: #""details": [], "unknown": true"#
            )
            .utf8
    )
    #expect(throws: TohsenoMetadataError.invalid) {
        try TohsenoMetadata.decodeTransportJSON(unknownNestedKey)
    }
}

@Test
func localPersistenceRejectsTraversalAndWritesAtomically() async throws {
    let temporary = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let outside = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let persistence = try LocalPersistence(root: temporary)
    try FileManager.default.createDirectory(
        at: outside,
        withIntermediateDirectories: true
    )
    defer {
        try? FileManager.default.removeItem(at: temporary)
        try? FileManager.default.removeItem(at: outside)
    }

    let value = Data("owned locally".utf8)
    try await persistence.write(value, to: "documents/state.json")
    #expect(try await persistence.read(from: "documents/state.json") == value)
    await #expect(throws: LocalPersistenceError.unsafeRelativePath) {
        try await persistence.write(value, to: "../outside")
    }

    let linkedDirectory = temporary.appendingPathComponent("linked")
    try FileManager.default.createSymbolicLink(
        at: linkedDirectory,
        withDestinationURL: outside
    )
    await #expect(throws: LocalPersistenceError.unsafeRelativePath) {
        try await persistence.write(value, to: "linked/escaped.json")
    }
    #expect(!FileManager.default.fileExists(
        atPath: outside.appendingPathComponent("escaped.json").path
    ))
}

@Test
func localPersistenceRejectsUnsafeApplicationIdentifiers() {
    #expect(throws: LocalPersistenceError.unsafeRelativePath) {
        try LocalPersistence(applicationIdentifier: "../outside")
    }
}

private func sampleMetadata(
    sequence: UInt32,
    bundleVersion: UInt32,
    sourceCommit: String = String(repeating: "a", count: 40),
    bundleID: String = "example.app",
    factoryImplementation: String = "example/factory",
    duplicateNetwork: Bool = false,
    evolutionCommitment: String = "0x" + String(repeating: "03", count: 32),
    sourceTreeSHA256: String = "0x" + String(repeating: "04", count: 32),
    fasciaSHA256: String = "0x" + String(repeating: "05", count: 32),
    registry: TohsenoRegistryReference? = nil,
    appStoreID: UInt64? = nil
) -> TohsenoMetadata {
    let endpoint = TohsenoNetworkDeclaration(
        endpoint: "https://example.invalid",
        purpose: "User-selected import"
    )
    return TohsenoMetadata(
        shotID: "0x" + String(repeating: "01", count: 32),
        builderID: "eip155:4663:0x1111111111111111111111111111111111111111",
        sequence: sequence,
        previous: sequence == 1
            ? nil
            : "0x" + String(repeating: "02", count: 32),
        evolutionCommitment: evolutionCommitment,
        sourceTreeSHA256: sourceTreeSHA256,
        fasciaSHA256: fasciaSHA256,
        bundleID: bundleID,
        bundleVersion: bundleVersion,
        factory: TohsenoFactoryReference(
            implementation: factoryImplementation,
            version: "1.0.0-rc.1",
            sourceCommit: sourceCommit
        ),
        distribution: TohsenoDistribution(
            state: appStoreID == nil ? .local : .appStore,
            supportedAppleSurfaces: [.iPhone],
            appStoreID: appStoreID
        ),
        capabilities: [
            TohsenoCapabilityDeclaration(
                capability: .localStorage,
                purpose: "Save the user's documents"
            ),
            TohsenoCapabilityDeclaration(
                capability: .networkAccess,
                purpose: "Fetch a chosen public document"
            ),
        ],
        network: duplicateNetwork ? [endpoint, endpoint] : [endpoint],
        registry: registry
    )
}

private struct ProtocolVectors: Decodable {
    let continuity: ContinuityVectors
}

private struct ContinuityVectors: Decodable {
    let activeWindow: [ContinuityActiveWindowVector]
    let canonical: ContinuityCanonicalVector
    let invalidStatements: [ContinuityInvalidStatementVector]
    let validStatements: [ContinuityValidStatementVector]

    private enum CodingKeys: String, CodingKey {
        case activeWindow = "active_window"
        case canonical
        case invalidStatements = "invalid_statements"
        case validStatements = "valid_statements"
    }
}

private struct ContinuityActiveWindowVector: Decodable {
    let nowUnix: UInt64
    let valid: Bool

    private enum CodingKeys: String, CodingKey {
        case nowUnix = "now_unix"
        case valid
    }
}

private struct ContinuityCanonicalVector: Decodable {
    let envelope: ContinuityEnvelope
    let rfc8785: String
    let sha256: String
    let statement: ContinuityStatement
}

private struct ContinuityInvalidStatementVector: Decodable {
    let name: String
    let statementJSON: String

    private enum CodingKeys: String, CodingKey {
        case name
        case statementJSON = "statement_json"
    }
}

private struct ContinuityValidStatementVector: Decodable {
    let name: String
    let statement: ContinuityStatement
}

private func loadProtocolVectors() throws -> ProtocolVectors {
    let repositoryRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
    let url = repositoryRoot
        .appendingPathComponent("protocol", isDirectory: true)
        .appendingPathComponent("test-vectors", isDirectory: true)
        .appendingPathComponent("protocol-v1.json")
    return try JSONDecoder().decode(
        ProtocolVectors.self,
        from: Data(contentsOf: url)
    )
}

private final class MemoryInstallationKeyStore:
    InstallationIdentityKeyStore,
    @unchecked Sendable
{
    private let lock = NSLock()
    private var values: [String: Data] = [:]

    func read(account: String) throws -> Data? {
        lock.withLock {
            values[account]
        }
    }

    func write(_ data: Data, account: String) throws {
        lock.withLock {
            values[account] = data
        }
    }
}
