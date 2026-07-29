import CryptoKit
import Foundation

public struct ContinuityPublicKey: Codable, Equatable, Sendable {
    public let x: String
    public let y: String

    public init(x: String, y: String) {
        self.x = x
        self.y = y
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["x", "y"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        x = try container.decode(String.self, forKey: .x)
        y = try container.decode(String.self, forKey: .y)
    }

    private enum CodingKeys: String, CodingKey {
        case x
        case y
    }
}

public struct ContinuityIssuer: Codable, Equatable, Sendable {
    public let installationID: String
    public let publicKey: ContinuityPublicKey

    public init(installationID: String, publicKey: ContinuityPublicKey) {
        self.installationID = installationID
        self.publicKey = publicKey
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["installation_id", "public_key"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        installationID = try container.decode(
            String.self,
            forKey: .installationID
        )
        publicKey = try container.decode(
            ContinuityPublicKey.self,
            forKey: .publicKey
        )
    }

    private enum CodingKeys: String, CodingKey {
        case installationID = "installation_id"
        case publicKey = "public_key"
    }
}

public struct ContinuityAudience: Codable, Equatable, Sendable {
    public let shotID: String
    public let installationID: String?

    public init(shotID: String, installationID: String?) {
        self.shotID = shotID
        self.installationID = installationID
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["shot_id", "installation_id"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        shotID = try container.decode(String.self, forKey: .shotID)
        installationID = try container.decodeIfPresent(
            String.self,
            forKey: .installationID
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(shotID, forKey: .shotID)
        if let installationID {
            try container.encode(installationID, forKey: .installationID)
        } else {
            try container.encodeNil(forKey: .installationID)
        }
    }

    private enum CodingKeys: String, CodingKey {
        case shotID = "shot_id"
        case installationID = "installation_id"
    }
}

public struct ContinuityStatement: Codable, Equatable, Sendable {
    public let schema: String
    public let issuer: ContinuityIssuer
    public let audience: ContinuityAudience
    public let originatingShotID: String
    public let claims: [String]
    public let nonce: String
    public let issuedAt: UInt64
    public let expiresAt: UInt64

    public init(
        schema: String = "tohseno.continuity-statement/1",
        issuer: ContinuityIssuer,
        audience: ContinuityAudience,
        originatingShotID: String,
        claims: [String],
        nonce: String,
        issuedAt: UInt64,
        expiresAt: UInt64
    ) {
        self.schema = schema
        self.issuer = issuer
        self.audience = audience
        self.originatingShotID = originatingShotID
        self.claims = claims
        self.nonce = nonce
        self.issuedAt = issuedAt
        self.expiresAt = expiresAt
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(
            decoder,
            [
                "schema",
                "issuer",
                "audience",
                "originating_shot_id",
                "claims",
                "nonce",
                "issued_at",
                "expires_at",
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        issuer = try container.decode(ContinuityIssuer.self, forKey: .issuer)
        audience = try container.decode(
            ContinuityAudience.self,
            forKey: .audience
        )
        originatingShotID = try container.decode(
            String.self,
            forKey: .originatingShotID
        )
        claims = try container.decode([String].self, forKey: .claims)
        nonce = try container.decode(String.self, forKey: .nonce)
        issuedAt = try container.decode(UInt64.self, forKey: .issuedAt)
        expiresAt = try container.decode(UInt64.self, forKey: .expiresAt)
    }

    private enum CodingKeys: String, CodingKey {
        case schema
        case issuer
        case audience
        case originatingShotID = "originating_shot_id"
        case claims
        case nonce
        case issuedAt = "issued_at"
        case expiresAt = "expires_at"
    }

    /// RFC 8785 canonical JSON for this closed, ASCII-constrained statement.
    ///
    /// Keys are emitted in UTF-16 lexicographic order. All schema strings,
    /// identifiers, and claims are constrained to ASCII values requiring no
    /// JSON escaping; integers are within the interoperable JSON safe range.
    public func canonicalJSON() throws -> Data {
        try validateStructure()
        let installation = audience.installationID
            .map { #""\#($0)""# }
            ?? "null"
        let encodedClaims = claims.map { #""\#($0)""# }.joined(separator: ",")
        let json = """
        {"audience":{"installation_id":\(installation),"shot_id":"\(audience.shotID)"},"claims":[\(encodedClaims)],"expires_at":\(expiresAt),"issued_at":\(issuedAt),"issuer":{"installation_id":"\(issuer.installationID)","public_key":{"x":"\(issuer.publicKey.x)","y":"\(issuer.publicKey.y)"}},"nonce":"\(nonce)","originating_shot_id":"\(originatingShotID)","schema":"tohseno.continuity-statement/1"}
        """
        return Data(json.utf8)
    }

    public func digest() throws -> Data {
        Data(SHA256.hash(data: try canonicalJSON()))
    }

    public func validate(nowUnix: UInt64) throws {
        try validateStructure()
        guard issuedAt <= nowUnix, expiresAt > nowUnix else {
            throw ContinuityError.expired
        }
    }

    private func validateStructure() throws {
        let maximumSafeInteger: UInt64 = 9_007_199_254_740_991
        guard schema == "tohseno.continuity-statement/1",
              issuedAt > 0,
              issuedAt <= maximumSafeInteger,
              expiresAt <= maximumSafeInteger,
              issuedAt < expiresAt,
              isHex32(issuer.installationID),
              isHex32(issuer.publicKey.x),
              isHex32(issuer.publicKey.y),
              isValidP256PublicKey(
                  x: issuer.publicKey.x,
                  y: issuer.publicKey.y
              ),
              isNonzeroHex32(audience.shotID),
              audience.installationID.map(isHex32) ?? true,
              isNonzeroHex32(originatingShotID),
              isNonzeroHex32(nonce),
              !claims.isEmpty,
              claims.count <= 16,
              claims == claims.sorted(),
              Set(claims).count == claims.count,
              claims.allSatisfy(isClaimToken),
              issuer.installationID
                  == Self.installationID(
                      x: issuer.publicKey.x,
                      y: issuer.publicKey.y
                  )
        else {
            throw ContinuityError.invalidStatement
        }
    }

    public static func installationID(x: String, y: String) -> String? {
        guard let xBytes = Data(fasciaHex: x, expectedBytes: 32),
              let yBytes = Data(fasciaHex: y, expectedBytes: 32)
        else {
            return nil
        }
        var input = Data("TOHSENO-INSTALLATION-ID-V1\0".utf8)
        input.append(xBytes)
        input.append(yBytes)
        return Data(SHA256.hash(data: input)).fasciaHex(prefix: true)
    }
}

public struct ContinuitySignatureComponents: Codable, Equatable, Sendable {
    public let r: String
    public let s: String

    public init(r: String, s: String) {
        self.r = r
        self.s = s
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["r", "s"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        r = try container.decode(String.self, forKey: .r)
        s = try container.decode(String.self, forKey: .s)
    }

    private enum CodingKeys: String, CodingKey {
        case r
        case s
    }
}

public struct ContinuitySignature: Codable, Equatable, Sendable {
    public let algorithm: String
    public let digest: String
    public let signature: ContinuitySignatureComponents
    public let lowS: Bool

    public init(
        algorithm: String = "p256",
        digest: String,
        signature: ContinuitySignatureComponents,
        lowS: Bool
    ) {
        self.algorithm = algorithm
        self.digest = digest
        self.signature = signature
        self.lowS = lowS
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(
            decoder,
            ["algorithm", "digest", "signature", "low_s"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        algorithm = try container.decode(String.self, forKey: .algorithm)
        digest = try container.decode(String.self, forKey: .digest)
        signature = try container.decode(
            ContinuitySignatureComponents.self,
            forKey: .signature
        )
        lowS = try container.decode(Bool.self, forKey: .lowS)
    }

    private enum CodingKeys: String, CodingKey {
        case algorithm
        case digest
        case signature
        case lowS = "low_s"
    }
}

public struct ContinuityEnvelope: Codable, Equatable, Sendable {
    public let schema: String
    public let statement: ContinuityStatement
    public let signature: ContinuitySignature

    public init(
        schema: String = "tohseno.continuity/1",
        statement: ContinuityStatement,
        signature: ContinuitySignature
    ) {
        self.schema = schema
        self.statement = statement
        self.signature = signature
    }

    public init(from decoder: Decoder) throws {
        try requireExactKeys(decoder, ["schema", "statement", "signature"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schema = try container.decode(String.self, forKey: .schema)
        statement = try container.decode(
            ContinuityStatement.self,
            forKey: .statement
        )
        signature = try container.decode(
            ContinuitySignature.self,
            forKey: .signature
        )
    }

    private enum CodingKeys: String, CodingKey {
        case schema
        case statement
        case signature
    }

    public static func issue(
        identity: InstallationIdentity,
        audience: ContinuityAudience,
        originatingShotID: String,
        claims: [String],
        nonce: Data,
        issuedAt: UInt64,
        expiresAt: UInt64,
        nowUnix: UInt64
    ) async throws -> ContinuityEnvelope {
        let key = try await identity.descriptor()
        let statement = ContinuityStatement(
            issuer: ContinuityIssuer(
                installationID: key.installationID,
                publicKey: ContinuityPublicKey(x: key.x, y: key.y)
            ),
            audience: audience,
            originatingShotID: originatingShotID,
            claims: claims,
            nonce: nonce.fasciaHex(prefix: true),
            issuedAt: issuedAt,
            expiresAt: expiresAt
        )
        try statement.validate(nowUnix: nowUnix)
        let signed = try await identity.sign(message: statement.canonicalJSON())
        guard signed.digest == (try statement.digest()).fasciaHex(prefix: true) else {
            throw ContinuityError.invalidSignature
        }
        guard signed.algorithm == "p256", signed.lowS else {
            throw ContinuityError.invalidSignature
        }
        return ContinuityEnvelope(
            statement: statement,
            signature: ContinuitySignature(
                digest: signed.digest,
                signature: ContinuitySignatureComponents(
                    r: signed.r,
                    s: signed.s
                ),
                lowS: signed.lowS
            )
        )
    }

    public func verify(nowUnix: UInt64) throws -> Bool {
        guard schema == "tohseno.continuity/1",
              signature.algorithm == "p256",
              signature.lowS
        else {
            throw ContinuityError.invalidEnvelope
        }
        try statement.validate(nowUnix: nowUnix)
        let canonical = try statement.canonicalJSON()
        let digest = Data(SHA256.hash(data: canonical))
        guard signature.digest == digest.fasciaHex(prefix: true),
              let x = Data(
                  fasciaHex: statement.issuer.publicKey.x,
                  expectedBytes: 32
              ),
              let y = Data(
                  fasciaHex: statement.issuer.publicKey.y,
                  expectedBytes: 32
              ),
              isHex32(signature.signature.r),
              isHex32(signature.signature.s),
              let r = Data(
                  fasciaHex: signature.signature.r,
                  expectedBytes: 32
              ),
              let s = Data(
                  fasciaHex: signature.signature.s,
                  expectedBytes: 32
              ),
              (try? FasciaP256.lowS(s)) == s
        else {
            return false
        }
        let publicKey = try P256.Signing.PublicKey(
            x963Representation: Data([0x04]) + x + y
        )
        let rawSignature = try P256.Signing.ECDSASignature(
            rawRepresentation: r + s
        )
        return publicKey.isValidSignature(rawSignature, for: canonical)
    }

    public func transportJSON() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }

    public static func decodeTransportJSON(_ data: Data) throws -> ContinuityEnvelope {
        try fasciaStrictJSONPreflight(data)
        return try JSONDecoder().decode(ContinuityEnvelope.self, from: data)
    }
}

public enum ContinuityError: Error, Equatable, Sendable {
    case invalidStatement
    case invalidEnvelope
    case invalidSignature
    case expired
}

enum FasciaStrictJSONError: Error, Equatable, Sendable {
    case invalid
    case duplicateKey
    case sizeLimit
    case depthLimit
}

/// Bounded JSON grammar preflight shared by public Fascia transports.
///
/// Foundation's decoder accepts duplicate object members. That makes the
/// interpreted value dependent on decoder behavior, so every object is
/// checked before normal typed decoding. Escaped spellings of the same key
/// are decoded before uniqueness comparison.
func fasciaStrictJSONPreflight(_ data: Data) throws {
    let maximumBytes = 1024 * 1024
    guard !data.isEmpty, data.count <= maximumBytes else {
        throw data.isEmpty
            ? FasciaStrictJSONError.invalid
            : FasciaStrictJSONError.sizeLimit
    }
    var parser = FasciaStrictJSONParser(bytes: Array(data))
    try parser.parse()
}

private struct FasciaStrictJSONParser {
    private static let maximumDepth = 64
    private static let maximumContainerEntries = 4096
    private static let maximumValues = 100_000

    let bytes: [UInt8]
    var index = 0
    var values = 0

    mutating func parse() throws {
        skipWhitespace()
        try parseValue(depth: 0)
        skipWhitespace()
        guard index == bytes.count else {
            throw FasciaStrictJSONError.invalid
        }
    }

    private mutating func parseValue(depth: Int) throws {
        guard depth <= Self.maximumDepth else {
            throw FasciaStrictJSONError.depthLimit
        }
        values += 1
        guard values <= Self.maximumValues, index < bytes.count else {
            throw FasciaStrictJSONError.invalid
        }
        switch bytes[index] {
        case 0x7b:
            try parseObject(depth: depth)
        case 0x5b:
            try parseArray(depth: depth)
        case 0x22:
            _ = try parseString()
        case 0x74:
            try consumeLiteral([0x74, 0x72, 0x75, 0x65])
        case 0x66:
            try consumeLiteral([0x66, 0x61, 0x6c, 0x73, 0x65])
        case 0x6e:
            try consumeLiteral([0x6e, 0x75, 0x6c, 0x6c])
        case 0x2d, 0x30 ... 0x39:
            try parseNumber()
        default:
            throw FasciaStrictJSONError.invalid
        }
    }

    private mutating func parseObject(depth: Int) throws {
        index += 1
        skipWhitespace()
        if consume(0x7d) {
            return
        }
        var keys = Set<String>()
        var entries = 0
        while true {
            guard index < bytes.count, bytes[index] == 0x22 else {
                throw FasciaStrictJSONError.invalid
            }
            let key = try parseString()
            guard keys.insert(key).inserted else {
                throw FasciaStrictJSONError.duplicateKey
            }
            entries += 1
            guard entries <= Self.maximumContainerEntries else {
                throw FasciaStrictJSONError.sizeLimit
            }
            skipWhitespace()
            guard consume(0x3a) else {
                throw FasciaStrictJSONError.invalid
            }
            skipWhitespace()
            try parseValue(depth: depth + 1)
            skipWhitespace()
            if consume(0x7d) {
                return
            }
            guard consume(0x2c) else {
                throw FasciaStrictJSONError.invalid
            }
            skipWhitespace()
        }
    }

    private mutating func parseArray(depth: Int) throws {
        index += 1
        skipWhitespace()
        if consume(0x5d) {
            return
        }
        var entries = 0
        while true {
            entries += 1
            guard entries <= Self.maximumContainerEntries else {
                throw FasciaStrictJSONError.sizeLimit
            }
            try parseValue(depth: depth + 1)
            skipWhitespace()
            if consume(0x5d) {
                return
            }
            guard consume(0x2c) else {
                throw FasciaStrictJSONError.invalid
            }
            skipWhitespace()
        }
    }

    private mutating func parseString() throws -> String {
        let start = index
        index += 1
        while index < bytes.count {
            switch bytes[index] {
            case 0x22:
                index += 1
                let encoded = Data(bytes[start ..< index])
                guard let value = try? JSONDecoder().decode(
                    String.self,
                    from: encoded
                ) else {
                    throw FasciaStrictJSONError.invalid
                }
                return value
            case 0x5c:
                index += 1
                guard index < bytes.count else {
                    throw FasciaStrictJSONError.invalid
                }
                index += 1
            case 0x00 ... 0x1f:
                throw FasciaStrictJSONError.invalid
            default:
                index += 1
            }
        }
        throw FasciaStrictJSONError.invalid
    }

    private mutating func parseNumber() throws {
        if consume(0x2d), index == bytes.count {
            throw FasciaStrictJSONError.invalid
        }
        if consume(0x30) {
            if index < bytes.count, (0x30 ... 0x39).contains(bytes[index]) {
                throw FasciaStrictJSONError.invalid
            }
        } else {
            guard index < bytes.count,
                  (0x31 ... 0x39).contains(bytes[index])
            else {
                throw FasciaStrictJSONError.invalid
            }
            consumeDigits()
        }
        if consume(0x2e) {
            guard consumeDigits() else {
                throw FasciaStrictJSONError.invalid
            }
        }
        if index < bytes.count, bytes[index] == 0x65 || bytes[index] == 0x45 {
            index += 1
            if index < bytes.count, bytes[index] == 0x2b || bytes[index] == 0x2d {
                index += 1
            }
            guard consumeDigits() else {
                throw FasciaStrictJSONError.invalid
            }
        }
    }

    @discardableResult
    private mutating func consumeDigits() -> Bool {
        let start = index
        while index < bytes.count, (0x30 ... 0x39).contains(bytes[index]) {
            index += 1
        }
        return index > start
    }

    private mutating func consumeLiteral(_ literal: [UInt8]) throws {
        guard index + literal.count <= bytes.count,
              Array(bytes[index ..< index + literal.count]) == literal
        else {
            throw FasciaStrictJSONError.invalid
        }
        index += literal.count
    }

    @discardableResult
    private mutating func consume(_ byte: UInt8) -> Bool {
        guard index < bytes.count, bytes[index] == byte else {
            return false
        }
        index += 1
        return true
    }

    private mutating func skipWhitespace() {
        while index < bytes.count,
              [0x20, 0x09, 0x0a, 0x0d].contains(bytes[index])
        {
            index += 1
        }
    }
}

private struct ContinuityCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
        self.stringValue = stringValue
        intValue = nil
    }

    init?(intValue: Int) {
        stringValue = String(intValue)
        self.intValue = intValue
    }
}

private func requireExactKeys(
    _ decoder: Decoder,
    _ required: Set<String>
) throws {
    let container = try decoder.container(keyedBy: ContinuityCodingKey.self)
    let observed = Set(container.allKeys.map(\.stringValue))
    guard observed == required else {
        throw DecodingError.dataCorrupted(.init(
            codingPath: decoder.codingPath,
            debugDescription: "continuity object keys do not match its closed schema"
        ))
    }
}

private func isHex32(_ value: String) -> Bool {
    guard value.count == 66, value.hasPrefix("0x") else {
        return false
    }
    return value.dropFirst(2).unicodeScalars.allSatisfy {
        ("0" ... "9").contains(Character($0))
            || ("a" ... "f").contains(Character($0))
    }
}

private func isNonzeroHex32(_ value: String) -> Bool {
    isHex32(value) && value.dropFirst(2).contains { $0 != "0" }
}

private func isValidP256PublicKey(x: String, y: String) -> Bool {
    guard let xBytes = Data(fasciaHex: x, expectedBytes: 32),
          let yBytes = Data(fasciaHex: y, expectedBytes: 32)
    else {
        return false
    }
    return (try? P256.Signing.PublicKey(
        x963Representation: Data([0x04]) + xBytes + yBytes
    )) != nil
}

private func isClaimToken(_ value: String) -> Bool {
    guard (1 ... 64).contains(value.utf8.count) else {
        return false
    }

    var previousWasSeparator = false
    for (index, byte) in value.utf8.enumerated() {
        let isSeparator = byte == 0x2e || byte == 0x5f || byte == 0x2d
        let isAlphanumeric = (0x61 ... 0x7a).contains(byte)
            || (0x30 ... 0x39).contains(byte)
        if !isAlphanumeric && !isSeparator {
            return false
        }
        if isSeparator && (index == 0 || previousWasSeparator) {
            return false
        }
        previousWasSeparator = isSeparator
    }
    return !previousWasSeparator
}
