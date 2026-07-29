import Foundation

public struct P256SignatureComponents: Equatable, Sendable {
    public let r: Data
    public let s: Data

    public init(r: Data, s: Data) throws {
        try ECDSASignatureCodec.validateScalar(r, name: "r")
        try ECDSASignatureCodec.validateScalar(s, name: "s")
        self.r = r
        self.s = s
    }
}

public enum ECDSASignatureCodec {
    // SEC 2 P-256 group order.
    public static let p256Order = Data([
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
        0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
    ])

    public static let p256HalfOrder = Data([
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
        0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
    ])

    public static func fixedWidthComponents(
        fromDER signature: Data,
        normalizeLowS: Bool = true
    ) throws -> P256SignatureComponents {
        var reader = DERReader(bytes: Array(signature))
        let sequenceLength = try reader.readConstructed(tag: 0x30)
        guard sequenceLength <= reader.bytes.count - reader.offset else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        let sequenceEnd = reader.offset + sequenceLength
        guard sequenceEnd == reader.bytes.count else {
            throw AppleIdentityError.invalidSignatureEncoding
        }

        let r = try reader.readPositiveInteger32()
        let parsedS = try reader.readPositiveInteger32()
        guard reader.offset == sequenceEnd else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        let s = normalizeLowS ? try lowS(parsedS) : parsedS
        return try P256SignatureComponents(r: r, s: s)
    }

    public static func derSignature(from components: P256SignatureComponents) throws -> Data {
        try validateScalar(components.r, name: "r")
        try validateScalar(components.s, name: "s")
        let r = encodeInteger(components.r)
        let s = encodeInteger(components.s)
        let body = r + s
        return Data([0x30]) + encodeLength(body.count) + body
    }

    public static func lowS(_ scalar: Data) throws -> Data {
        try validateScalar(scalar, name: "s")
        if compare(scalar, p256HalfOrder) <= 0 {
            return scalar
        }
        return subtract(p256Order, scalar)
    }

    public static func isLowS(_ scalar: Data) -> Bool {
        scalar.count == 32
            && !scalar.allSatisfy { $0 == 0 }
            && compare(scalar, p256HalfOrder) <= 0
    }

    static func validateScalar(_ scalar: Data, name: String) throws {
        guard scalar.count == 32,
              !scalar.allSatisfy({ $0 == 0 }),
              compare(scalar, p256Order) < 0
        else {
            throw AppleIdentityError.invalidScalar(name)
        }
    }

    private static func encodeInteger(_ scalar: Data) -> Data {
        var bytes = Array(scalar)
        while bytes.count > 1, bytes.first == 0 {
            bytes.removeFirst()
        }
        if let first = bytes.first, first & 0x80 != 0 {
            bytes.insert(0, at: 0)
        }
        return Data([0x02]) + encodeLength(bytes.count) + Data(bytes)
    }

    private static func encodeLength(_ count: Int) -> Data {
        precondition(count >= 0)
        if count < 0x80 {
            return Data([UInt8(count)])
        }
        var value = count
        var bytes: [UInt8] = []
        while value > 0 {
            bytes.insert(UInt8(value & 0xff), at: 0)
            value >>= 8
        }
        return Data([0x80 | UInt8(bytes.count)]) + Data(bytes)
    }

    private static func compare(_ left: Data, _ right: Data) -> Int {
        let lhs = Array(left)
        let rhs = Array(right)
        if lhs.count != rhs.count {
            return lhs.count < rhs.count ? -1 : 1
        }
        for (a, b) in zip(lhs, rhs) where a != b {
            return a < b ? -1 : 1
        }
        return 0
    }

    private static func subtract(_ left: Data, _ right: Data) -> Data {
        precondition(left.count == right.count)
        precondition(compare(left, right) >= 0)
        let lhs = Array(left)
        let rhs = Array(right)
        var result = [UInt8](repeating: 0, count: lhs.count)
        var borrow = 0
        for index in stride(from: lhs.count - 1, through: 0, by: -1) {
            var value = Int(lhs[index]) - Int(rhs[index]) - borrow
            if value < 0 {
                value += 256
                borrow = 1
            } else {
                borrow = 0
            }
            result[index] = UInt8(value)
        }
        precondition(borrow == 0)
        return Data(result)
    }
}

private struct DERReader {
    let bytes: [UInt8]
    var offset = 0

    mutating func readConstructed(tag: UInt8) throws -> Int {
        guard readByte() == tag else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        return try readLength()
    }

    mutating func readPositiveInteger32() throws -> Data {
        guard readByte() == 0x02 else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        let length = try readLength()
        guard length > 0, offset + length <= bytes.count else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        var integer = Array(bytes[offset ..< offset + length])
        offset += length

        guard integer.first.map({ $0 & 0x80 == 0 }) == true else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        if integer.count > 1, integer.first == 0 {
            guard integer[1] & 0x80 != 0 else {
                throw AppleIdentityError.invalidSignatureEncoding
            }
            integer.removeFirst()
        }
        guard integer.count <= 32 else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        integer.insert(
            contentsOf: repeatElement(0, count: 32 - integer.count),
            at: 0
        )
        return Data(integer)
    }

    mutating func readLength() throws -> Int {
        guard let first = readByte() else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        if first & 0x80 == 0 {
            return Int(first)
        }
        let byteCount = Int(first & 0x7f)
        guard byteCount > 0, byteCount <= MemoryLayout<Int>.size,
              offset + byteCount <= bytes.count,
              bytes[offset] != 0
        else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        var value = 0
        for _ in 0 ..< byteCount {
            guard let byte = readByte(), value <= (Int.max >> 8) else {
                throw AppleIdentityError.invalidSignatureEncoding
            }
            value = (value << 8) | Int(byte)
        }
        guard value >= 0x80 else {
            throw AppleIdentityError.invalidSignatureEncoding
        }
        return value
    }

    mutating func readByte() -> UInt8? {
        guard offset < bytes.count else {
            return nil
        }
        defer { offset += 1 }
        return bytes[offset]
    }
}
