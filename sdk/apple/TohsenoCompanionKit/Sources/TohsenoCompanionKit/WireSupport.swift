import CryptoKit
import Foundation

enum CompanionLimits {
    static let maximumJSONBytes = 1024 * 1024
    static let maximumWorkspaceEventBytes = 4 * 1024 * 1024
    static let maximumRelayResponseBytes = 4 * 1024 * 1024
    static let maximumSafeJSONInteger: UInt64 = 9_007_199_254_740_991
    static let maximumTextBytes = 100_000
    static let maximumDeviceNameBytes = 255
    static let maximumIdentifierBytes = 128
    static let maximumEnvelopeCiphertextBytes = 16 * 1024 * 1024 + 16
    static let maximumEnvelopeBodyBytes = 24 * 1024 * 1024
    static let maximumPendingCommands = 32
    static let maximumPendingReferenceChunks = maximumPendingCommands * 8 * 8
    static let maximumPendingPayloadFiles = maximumPendingReferenceChunks * 2
}

enum Base64URL {
    static func encode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    static func decode(_ value: String, expectedBytes: Int? = nil) throws -> Data {
        guard !value.isEmpty,
              value.utf8.allSatisfy({
                  (0x41 ... 0x5a).contains($0)
                      || (0x61 ... 0x7a).contains($0)
                      || (0x30 ... 0x39).contains($0)
                      || $0 == 0x2d
                      || $0 == 0x5f
              })
        else {
            throw TohsenoCompanionError.invalidEncoding("base64url alphabet")
        }
        let remainder = value.utf8.count % 4
        guard remainder != 1 else {
            throw TohsenoCompanionError.invalidEncoding("base64url length")
        }
        let padding = remainder == 0 ? "" : String(repeating: "=", count: 4 - remainder)
        let standard = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/") + padding
        guard let decoded = Data(base64Encoded: standard),
              expectedBytes.map({ decoded.count == $0 }) ?? true,
              encode(decoded) == value
        else {
            throw TohsenoCompanionError.invalidEncoding("non-canonical base64url")
        }
        return decoded
    }
}

enum CanonicalValue: Sendable {
    case null
    case bool(Bool)
    case unsigned(UInt64)
    case string(String)
    case array([CanonicalValue])
    case object([String: CanonicalValue])

    func data() throws -> Data {
        var result = Data()
        try append(to: &result)
        return result
    }

    private func append(to output: inout Data) throws {
        switch self {
        case .null:
            output.append(contentsOf: "null".utf8)
        case let .bool(value):
            output.append(contentsOf: (value ? "true" : "false").utf8)
        case let .unsigned(value):
            guard value <= CompanionLimits.maximumSafeJSONInteger else {
                throw TohsenoCompanionError.invalidEncoding("JSON integer exceeds interoperable range")
            }
            output.append(contentsOf: String(value).utf8)
        case let .string(value):
            try Self.appendJSONString(value, to: &output)
        case let .array(values):
            output.append(0x5b)
            for (index, value) in values.enumerated() {
                if index > 0 { output.append(0x2c) }
                try value.append(to: &output)
            }
            output.append(0x5d)
        case let .object(values):
            guard values.keys.allSatisfy({ $0.unicodeScalars.allSatisfy(\.isASCII) }) else {
                throw TohsenoCompanionError.invalidEncoding("canonical object key is not ASCII")
            }
            output.append(0x7b)
            let keys = values.keys.sorted()
            for (index, key) in keys.enumerated() {
                if index > 0 { output.append(0x2c) }
                try Self.appendJSONString(key, to: &output)
                output.append(0x3a)
                try values[key]!.append(to: &output)
            }
            output.append(0x7d)
        }
    }

    private static func appendJSONString(_ value: String, to output: inout Data) throws {
        output.append(0x22)
        for scalar in value.unicodeScalars {
            switch scalar.value {
            case 0x08: output.append(contentsOf: "\\b".utf8)
            case 0x09: output.append(contentsOf: "\\t".utf8)
            case 0x0a: output.append(contentsOf: "\\n".utf8)
            case 0x0c: output.append(contentsOf: "\\f".utf8)
            case 0x0d: output.append(contentsOf: "\\r".utf8)
            case 0x22: output.append(contentsOf: "\\\"".utf8)
            case 0x5c: output.append(contentsOf: "\\\\".utf8)
            case 0x00 ... 0x1f:
                output.append(contentsOf: String(format: "\\u%04x", scalar.value).utf8)
            default:
                output.append(contentsOf: String(scalar).utf8)
            }
        }
        output.append(0x22)
    }
}

func requireBoundedText(_ value: String, field: String, maximum: Int = CompanionLimits.maximumTextBytes) throws {
    guard !value.isEmpty, value.utf8.count <= maximum,
          !value.unicodeScalars.contains(where: { $0.value == 0 })
    else {
        throw TohsenoCompanionError.invalidEncoding("\(field) is empty or exceeds its bound")
    }
}

func requireIdentifier(_ value: String, field: String) throws {
    try requireBoundedText(value, field: field, maximum: CompanionLimits.maximumIdentifierBytes)
    guard value.unicodeScalars.allSatisfy({ scalar in
        switch scalar.value {
        case 0x2d, 0x2e, 0x30 ... 0x39, 0x3a, 0x41 ... 0x5a, 0x5f, 0x61 ... 0x7a:
            true
        default:
            false
        }
    }) else {
        throw TohsenoCompanionError.invalidEncoding("\(field) contains unsupported characters")
    }
}

func requireExactKeys(_ decoder: Decoder, _ expected: Set<String>) throws {
    let container = try decoder.container(keyedBy: AnyCodingKey.self)
    let observed = Set(container.allKeys.map(\.stringValue))
    guard observed == expected else {
        throw DecodingError.dataCorrupted(.init(
            codingPath: decoder.codingPath,
            debugDescription: "closed companion object keys differ from its schema"
        ))
    }
}

private struct AnyCodingKey: CodingKey {
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

enum StrictJSON {
    static func decode<T: Decodable>(
        _ type: T.Type,
        from data: Data,
        maximumBytes: Int = CompanionLimits.maximumJSONBytes
    ) throws -> T {
        guard !data.isEmpty, data.count <= maximumBytes else {
            throw TohsenoCompanionError.responseTooLarge
        }
        var parser = StrictJSONParser(bytes: Array(data))
        try parser.parse()
        return try JSONDecoder().decode(type, from: data)
    }

    static func encode<T: Encodable>(_ value: T) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(value)
    }
}

private struct StrictJSONParser {
    static let maximumDepth = 64
    static let maximumValues = 100_000
    static let maximumContainerEntries = 4096

    let bytes: [UInt8]
    var index = 0
    var values = 0

    mutating func parse() throws {
        skipWhitespace()
        try parseValue(depth: 0)
        skipWhitespace()
        guard index == bytes.count else { throw invalid() }
    }

    mutating func parseValue(depth: Int) throws {
        guard depth <= Self.maximumDepth, index < bytes.count else { throw invalid() }
        values += 1
        guard values <= Self.maximumValues else { throw invalid() }
        switch bytes[index] {
        case 0x7b: try parseObject(depth: depth)
        case 0x5b: try parseArray(depth: depth)
        case 0x22: _ = try parseString()
        case 0x74: try consumeLiteral(Array("true".utf8))
        case 0x66: try consumeLiteral(Array("false".utf8))
        case 0x6e: try consumeLiteral(Array("null".utf8))
        case 0x2d, 0x30 ... 0x39: try parseNumber()
        default: throw invalid()
        }
    }

    mutating func parseObject(depth: Int) throws {
        index += 1
        skipWhitespace()
        if consume(0x7d) { return }
        var keys = Set<String>()
        var entries = 0
        while true {
            let key = try parseString()
            guard keys.insert(key).inserted else { throw invalid() }
            entries += 1
            guard entries <= Self.maximumContainerEntries else { throw invalid() }
            skipWhitespace()
            guard consume(0x3a) else { throw invalid() }
            skipWhitespace()
            try parseValue(depth: depth + 1)
            skipWhitespace()
            if consume(0x7d) { return }
            guard consume(0x2c) else { throw invalid() }
            skipWhitespace()
        }
    }

    mutating func parseArray(depth: Int) throws {
        index += 1
        skipWhitespace()
        if consume(0x5d) { return }
        var entries = 0
        while true {
            entries += 1
            guard entries <= Self.maximumContainerEntries else { throw invalid() }
            try parseValue(depth: depth + 1)
            skipWhitespace()
            if consume(0x5d) { return }
            guard consume(0x2c) else { throw invalid() }
            skipWhitespace()
        }
    }

    mutating func parseString() throws -> String {
        guard consume(0x22) else { throw invalid() }
        let start = index - 1
        while index < bytes.count {
            switch bytes[index] {
            case 0x22:
                index += 1
                let encoded = Data(bytes[start ..< index])
                guard let value = try? JSONDecoder().decode(String.self, from: encoded) else {
                    throw invalid()
                }
                return value
            case 0x5c:
                index += 1
                guard index < bytes.count else { throw invalid() }
                if bytes[index] == 0x75 {
                    guard index + 4 < bytes.count,
                          bytes[(index + 1) ... (index + 4)].allSatisfy({ byte in
                              (0x30 ... 0x39).contains(byte)
                                  || (0x41 ... 0x46).contains(byte)
                                  || (0x61 ... 0x66).contains(byte)
                          }) else { throw invalid() }
                    index += 5
                } else {
                    guard [0x22, 0x2f, 0x5c, 0x62, 0x66, 0x6e, 0x72, 0x74].contains(bytes[index]) else {
                        throw invalid()
                    }
                    index += 1
                }
            case 0x00 ... 0x1f: throw invalid()
            default: index += 1
            }
        }
        throw invalid()
    }

    mutating func parseNumber() throws {
        _ = consume(0x2d)
        guard index < bytes.count else { throw invalid() }
        if consume(0x30) {
            if index < bytes.count, (0x30 ... 0x39).contains(bytes[index]) { throw invalid() }
        } else {
            guard index < bytes.count, (0x31 ... 0x39).contains(bytes[index]) else { throw invalid() }
            consumeDigits()
        }
        if consume(0x2e) {
            let start = index
            consumeDigits()
            guard index > start else { throw invalid() }
        }
        if index < bytes.count, bytes[index] == 0x65 || bytes[index] == 0x45 {
            index += 1
            if index < bytes.count, bytes[index] == 0x2b || bytes[index] == 0x2d { index += 1 }
            let start = index
            consumeDigits()
            guard index > start else { throw invalid() }
        }
    }

    mutating func consumeDigits() {
        while index < bytes.count, (0x30 ... 0x39).contains(bytes[index]) { index += 1 }
    }

    mutating func consumeLiteral(_ literal: [UInt8]) throws {
        guard index + literal.count <= bytes.count,
              Array(bytes[index ..< index + literal.count]) == literal else { throw invalid() }
        index += literal.count
    }

    mutating func consume(_ byte: UInt8) -> Bool {
        guard index < bytes.count, bytes[index] == byte else { return false }
        index += 1
        return true
    }

    mutating func skipWhitespace() {
        while index < bytes.count, [0x20, 0x09, 0x0a, 0x0d].contains(bytes[index]) { index += 1 }
    }

    func invalid() -> TohsenoCompanionError {
        .invalidEncoding("strict JSON preflight failed")
    }
}

extension Data {
    var companionSHA256: Data { Data(SHA256.hash(data: self)) }
}
