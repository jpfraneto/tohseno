import CryptoKit
import Foundation
import TohsenoAppleIdentity

public struct BuilderDevicePublicIdentity: Codable, Equatable, Sendable {
    public let schema: String
    public let keyID: String
    public let x: String
    public let y: String
    public let securityLevel: String
    public let testOnly: Bool

    enum CodingKeys: String, CodingKey {
        case schema
        case keyID = "key_id"
        case x
        case y
        case securityLevel = "security_level"
        case testOnly = "test_only"
    }
}

public struct BuilderDeviceAuthorization: Codable, Equatable, Sendable {
    public let schema: String
    public let signer: BuilderDevicePublicIdentity
    public let algorithm: String
    public let digest: String
    public let r: String
    public let s: String
    public let lowS: Bool

    enum CodingKeys: String, CodingKey {
        case schema, signer, algorithm, digest, r, s
        case lowS = "low_s"
    }
}

/// The Builder authority is intentionally separate from Companion transport
/// identity. Its private P-256 scalar is generated in Secure Enclave, is
/// non-exportable, and is never included in pairing, backup, or relay data.
public actor BuilderDeviceIdentity {
    public static let productionTag = "tohseno.companion.builder-device.v1"

    private let store: AppleIdentityStore
    private let tag: String

    public init(
        store: AppleIdentityStore = .shared,
        tag: String = BuilderDeviceIdentity.productionTag
    ) {
        self.store = store
        self.tag = tag
    }

    public func ensureCreated(allowSoftwareTest: Bool = false) throws -> BuilderDevicePublicIdentity {
        let description: AppleIdentityDescription
        do {
            description = try store.publicIdentity(tag: tag)
        } catch AppleIdentityError.identityNotFound {
            #if targetEnvironment(simulator)
            guard allowSoftwareTest else { throw AppleIdentityError.secureEnclaveUnavailable }
            description = try store.create(tag: tag, backend: .softwareTest)
            #else
            description = try store.create(tag: tag, backend: .secureEnclave)
            #endif
        }
        if description.testOnly && !allowSoftwareTest {
            throw AppleIdentityError.secureEnclaveUnavailable
        }
        return try Self.publicIdentity(description)
    }

    /// Signs one already-computed 32-byte protocol digest with Apple's exact
    /// prehash API. The returned signature is normalized and verified by the
    /// shared Apple identity implementation before it leaves this actor.
    public func sign(digestHex: String, allowSoftwareTest: Bool = false) throws -> BuilderDeviceAuthorization {
        _ = try ensureCreated(allowSoftwareTest: allowSoftwareTest)
        let digest = try Self.decodeDigest(digestHex)
        let signed = try store.sign(tag: tag, digest: digest)
        if signed.identity.testOnly && !allowSoftwareTest {
            throw AppleIdentityError.secureEnclaveUnavailable
        }
        return BuilderDeviceAuthorization(
            schema: "tohseno.builder-device-authorization/1",
            signer: try Self.publicIdentity(signed.identity),
            algorithm: signed.algorithm,
            digest: signed.digest,
            r: signed.signature.r,
            s: signed.signature.s,
            lowS: signed.lowS
        )
    }

    public func builderID(allowSoftwareTest: Bool = false) throws -> String {
        let identity = try ensureCreated(allowSoftwareTest: allowSoftwareTest)
        guard let keyID = Self.decodeHex(identity.keyID, bytes: 32),
              let x = Self.decodeHex(identity.x, bytes: 32),
              let y = Self.decodeHex(identity.y, bytes: 32),
              let factory = Self.decodeHex("0xb1bd208cd2af98e701f43d06aaa889d3a594df65", bytes: 20),
              let url = Bundle.module.url(forResource: "BuilderAccount.creation", withExtension: "hex")
        else { throw AppleIdentityError.invalidDigest }
        let encoded = try String(contentsOf: url, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard let creation = Self.decodeHex(encoded, bytes: nil) else {
            throw AppleIdentityError.invalidDigest
        }
        var saltInput = Data("TOHSENO-BUILDER-SALT-V1\0".utf8)
        saltInput.append(keyID)
        let salt = Data(SHA256.hash(data: saltInput))
        var initInput = creation
        initInput.append(x); initInput.append(y)
        let initHash = Keccak256.hash(initInput)
        var create2 = Data([0xff]); create2.append(factory); create2.append(salt); create2.append(initHash)
        return "eip155:4663:0x\(Keccak256.hash(create2).suffix(20).map { String(format: "%02x", $0) }.joined())"
    }

    private static func publicIdentity(
        _ value: AppleIdentityDescription
    ) throws -> BuilderDevicePublicIdentity {
        let x = try decodeDigest(value.publicKey.x)
        let y = try decodeDigest(value.publicKey.y)
        let keyID = Keccak256.hash(x + y).hexadecimal
        return BuilderDevicePublicIdentity(
            schema: "tohseno.builder-device-key/1",
            keyID: "0x\(keyID)",
            x: value.publicKey.x,
            y: value.publicKey.y,
            securityLevel: value.securityLevel,
            testOnly: value.testOnly
        )
    }

    private static func decodeDigest(_ value: String) throws -> Data {
        guard value.hasPrefix("0x"), value.count == 66 else {
            throw AppleIdentityError.invalidDigest
        }
        var bytes = Data(capacity: 32)
        var index = value.index(value.startIndex, offsetBy: 2)
        for _ in 0 ..< 32 {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index ..< next], radix: 16) else {
                throw AppleIdentityError.invalidDigest
            }
            bytes.append(byte)
            index = next
        }
        return bytes
    }

    private static func decodeHex(_ value: String, bytes: Int?) -> Data? {
        guard value.hasPrefix("0x"), value.count > 2, value.dropFirst(2).count.isMultiple(of: 2),
              bytes.map({ value.count == 2 + $0 * 2 }) ?? true
        else { return nil }
        var result = Data(); var index = value.index(value.startIndex, offsetBy: 2)
        while index < value.endIndex {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index ..< next], radix: 16) else { return nil }
            result.append(byte); index = next
        }
        return result
    }
}

enum Keccak256 {
    private static let rotations: [UInt64] = [
        0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39,
        41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
    ]
    private static let constants: [UInt64] = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808A,
        0x8000000080008000, 0x000000000000808B, 0x0000000080000001,
        0x8000000080008081, 0x8000000000008009, 0x000000000000008A,
        0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
        0x000000008000808B, 0x800000000000008B, 0x8000000000008089,
        0x8000000000008003, 0x8000000000008002, 0x8000000000000080,
        0x000000000000800A, 0x800000008000000A, 0x8000000080008081,
        0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
    ]

    static func hash(_ data: Data) -> Data {
        let rate = 136
        var message = [UInt8](data)
        message.append(0x01)
        while message.count % rate != rate - 1 { message.append(0) }
        message.append(0x80)
        var state = [UInt64](repeating: 0, count: 25)
        for offset in stride(from: 0, to: message.count, by: rate) {
            for lane in 0 ..< rate / 8 {
                var value: UInt64 = 0
                for byte in 0 ..< 8 {
                    value |= UInt64(message[offset + lane * 8 + byte]) << UInt64(byte * 8)
                }
                state[lane] ^= value
            }
            permute(&state)
        }
        var output = Data(capacity: 32)
        for lane in 0 ..< 4 {
            for byte in 0 ..< 8 {
                output.append(UInt8(truncatingIfNeeded: state[lane] >> UInt64(byte * 8)))
            }
        }
        return output
    }

    private static func permute(_ state: inout [UInt64]) {
        for constant in constants {
            var column = [UInt64](repeating: 0, count: 5)
            for x in 0 ..< 5 {
                column[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20]
            }
            var theta = [UInt64](repeating: 0, count: 5)
            for x in 0 ..< 5 { theta[x] = column[(x + 4) % 5] ^ column[(x + 1) % 5].rotatedLeft(1) }
            for index in 0 ..< 25 { state[index] ^= theta[index % 5] }
            var rhoPi = [UInt64](repeating: 0, count: 25)
            for x in 0 ..< 5 {
                for y in 0 ..< 5 {
                    rhoPi[y + 5 * ((2 * x + 3 * y) % 5)] = state[x + 5 * y].rotatedLeft(rotations[x + 5 * y])
                }
            }
            for x in 0 ..< 5 {
                for y in 0 ..< 5 {
                    state[x + 5 * y] = rhoPi[x + 5 * y]
                        ^ ((~rhoPi[(x + 1) % 5 + 5 * y]) & rhoPi[(x + 2) % 5 + 5 * y])
                }
            }
            state[0] ^= constant
        }
    }
}

private extension UInt64 {
    func rotatedLeft(_ amount: UInt64) -> UInt64 {
        let shift = amount & 63
        guard shift != 0 else { return self }
        return (self << shift) | (self >> (64 - shift))
    }
}

private extension Data {
    var hexadecimal: String {
        map { String(format: "%02x", $0) }.joined()
    }
}
