import CryptoKit
import Foundation
import Security

public enum InstallationIdentityBackend: String, Codable, Sendable {
    case secureEnclave = "secure_enclave"
    case softwareThisDeviceOnly = "software_this_device_only"
}

public struct InstallationPublicKey: Codable, Equatable, Sendable {
    public let installationID: String
    public let algorithm: String
    public let backend: InstallationIdentityBackend
    public let x: String
    public let y: String

    public init(
        installationID: String,
        algorithm: String,
        backend: InstallationIdentityBackend,
        x: String,
        y: String
    ) {
        self.installationID = installationID
        self.algorithm = algorithm
        self.backend = backend
        self.x = x
        self.y = y
    }
}

public struct InstallationSignature: Codable, Equatable, Sendable {
    public let algorithm: String
    public let digest: String
    public let r: String
    public let s: String
    public let lowS: Bool

    public init(
        algorithm: String,
        digest: String,
        r: String,
        s: String,
        lowS: Bool
    ) {
        self.algorithm = algorithm
        self.digest = digest
        self.r = r
        self.s = s
        self.lowS = lowS
    }
}

public enum InstallationIdentityError: Error, Equatable, Sendable {
    case missingApplicationIdentifier
    case keychain(OSStatus)
    case corruptKey
    case invalidPublicKey
    case invalidSignature
}

/// One app-specific, device-local installation authority.
///
/// Generated applications call `prepare()` during first launch. No public
/// method returns a private key, Secure Enclave representation, or Keychain
/// payload.
public actor InstallationIdentity {
    public static let shared = InstallationIdentity(
        applicationIdentifier: Bundle.main.bundleIdentifier ?? ""
    )

    private let applicationIdentifier: String
    private let keyStore: any InstallationIdentityKeyStore
    private let secureEnclaveAvailable: @Sendable () -> Bool

    public init(applicationIdentifier: String) {
        self.applicationIdentifier = applicationIdentifier
        keyStore = KeychainInstallationIdentityKeyStore()
        secureEnclaveAvailable = { SecureEnclave.isAvailable }
    }

    init(
        applicationIdentifier: String,
        keyStore: any InstallationIdentityKeyStore,
        secureEnclaveAvailable: @escaping @Sendable () -> Bool
    ) {
        self.applicationIdentifier = applicationIdentifier
        self.keyStore = keyStore
        self.secureEnclaveAvailable = secureEnclaveAvailable
    }

    /// Creates the identity if this is the installation's first launch.
    @discardableResult
    public func prepare() throws -> InstallationPublicKey {
        try descriptor()
    }

    public func descriptor() throws -> InstallationPublicKey {
        let material = try loadOrCreate()
        return try Self.publicDescriptor(for: material)
    }

    /// Signs the SHA-256 digest of `message` and returns fixed-width, low-s
    /// P-256 components.
    public func sign(message: Data) throws -> InstallationSignature {
        let material = try loadOrCreate()
        let derSignature: Data
        switch material.backend {
        case .secureEnclave:
            let key = try SecureEnclave.P256.Signing.PrivateKey(
                dataRepresentation: material.representation
            )
            derSignature = try key.signature(for: message).derRepresentation
        case .softwareThisDeviceOnly:
            let key = try P256.Signing.PrivateKey(
                rawRepresentation: material.representation
            )
            derSignature = try key.signature(for: message).derRepresentation
        }
        let components = try FasciaP256.fixedWidthComponents(fromDER: derSignature)
        let r = components.r
        let s = try FasciaP256.lowS(components.s)
        let normalized = try P256.Signing.ECDSASignature(
            rawRepresentation: r + s
        )
        let valid: Bool
        switch material.backend {
        case .secureEnclave:
            let key = try SecureEnclave.P256.Signing.PrivateKey(
                dataRepresentation: material.representation
            )
            valid = key.publicKey.isValidSignature(normalized, for: message)
        case .softwareThisDeviceOnly:
            let key = try P256.Signing.PrivateKey(
                rawRepresentation: material.representation
            )
            valid = key.publicKey.isValidSignature(normalized, for: message)
        }
        guard valid else {
            throw InstallationIdentityError.invalidSignature
        }
        let digest = Data(SHA256.hash(data: message))
        return InstallationSignature(
            algorithm: "p256",
            digest: digest.fasciaHex(prefix: true),
            r: r.fasciaHex(prefix: true),
            s: s.fasciaHex(prefix: true),
            lowS: true
        )
    }

    private func loadOrCreate() throws -> InstallationKeyMaterial {
        guard !applicationIdentifier.isEmpty else {
            throw InstallationIdentityError.missingApplicationIdentifier
        }
        if let stored = try keyStore.read(account: applicationIdentifier) {
            guard let material = try? JSONDecoder().decode(
                InstallationKeyMaterial.self,
                from: stored
            ), material.schema == "tohseno.installation-key/1"
            else {
                throw InstallationIdentityError.corruptKey
            }
            _ = try Self.publicDescriptor(for: material)
            return material
        }

        let material: InstallationKeyMaterial
        if secureEnclaveAvailable() {
            let key = try SecureEnclave.P256.Signing.PrivateKey()
            material = InstallationKeyMaterial(
                schema: "tohseno.installation-key/1",
                backend: .secureEnclave,
                representation: key.dataRepresentation
            )
        } else {
            let key = P256.Signing.PrivateKey()
            material = InstallationKeyMaterial(
                schema: "tohseno.installation-key/1",
                backend: .softwareThisDeviceOnly,
                representation: key.rawRepresentation
            )
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        try keyStore.write(
            encoder.encode(material),
            account: applicationIdentifier
        )
        return material
    }

    private static func publicDescriptor(
        for material: InstallationKeyMaterial
    ) throws -> InstallationPublicKey {
        let representation: Data
        switch material.backend {
        case .secureEnclave:
            representation = try SecureEnclave.P256.Signing.PrivateKey(
                dataRepresentation: material.representation
            ).publicKey.x963Representation
        case .softwareThisDeviceOnly:
            representation = try P256.Signing.PrivateKey(
                rawRepresentation: material.representation
            ).publicKey.x963Representation
        }
        guard representation.count == 65, representation.first == 0x04 else {
            throw InstallationIdentityError.invalidPublicKey
        }
        let x = representation.subdata(in: 1 ..< 33)
        let y = representation.subdata(in: 33 ..< 65)
        var identifierInput = Data("TOHSENO-INSTALLATION-ID-V1\0".utf8)
        identifierInput.append(x)
        identifierInput.append(y)
        let installationID = Data(SHA256.hash(data: identifierInput))
            .fasciaHex(prefix: true)
        return InstallationPublicKey(
            installationID: installationID,
            algorithm: "p256",
            backend: material.backend,
            x: x.fasciaHex(prefix: true),
            y: y.fasciaHex(prefix: true)
        )
    }
}

private struct InstallationKeyMaterial: Codable {
    let schema: String
    let backend: InstallationIdentityBackend
    let representation: Data
}

protocol InstallationIdentityKeyStore: Sendable {
    func read(account: String) throws -> Data?
    func write(_ data: Data, account: String) throws
}

private struct KeychainInstallationIdentityKeyStore:
    InstallationIdentityKeyStore,
    Sendable
{
    // The protocol candidate has a device-local namespace distinct from every
    // stable TOHSENO installation. The bundle identifier remains the account,
    // so each generated application is still an independent installation.
    private let service = "org.tohseno.genesis.installation-identity.v1"

    func read(account: String) throws -> Data? {
        var item: CFTypeRef?
        let status = SecItemCopyMatching([
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrSynchronizable: kCFBooleanFalse as Any,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ] as CFDictionary, &item)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess, let data = item as? Data else {
            throw InstallationIdentityError.keychain(status)
        }
        return data
    }

    func write(_ data: Data, account: String) throws {
        let status = SecItemAdd([
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrSynchronizable: kCFBooleanFalse as Any,
            kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData: data,
            kSecAttrLabel: "TOHSENO app-specific InstallationIdentity",
        ] as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw InstallationIdentityError.keychain(status)
        }
    }
}

enum FasciaP256 {
    static let order = Data([
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
        0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
    ])
    static let halfOrder = Data([
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
        0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
    ])

    static func lowS(_ scalar: Data) throws -> Data {
        guard scalar.count == 32,
              !scalar.allSatisfy({ $0 == 0 }),
              compare(scalar, order) < 0
        else {
            throw InstallationIdentityError.invalidSignature
        }
        guard compare(scalar, halfOrder) > 0 else {
            return scalar
        }
        return subtract(order, scalar)
    }

    static func fixedWidthComponents(
        fromDER signature: Data
    ) throws -> (r: Data, s: Data) {
        var reader = FasciaDERReader(bytes: Array(signature))
        let sequenceLength = try reader.readConstructed(tag: 0x30)
        guard sequenceLength <= reader.bytes.count - reader.offset else {
            throw InstallationIdentityError.invalidSignature
        }
        let sequenceEnd = reader.offset + sequenceLength
        guard sequenceEnd == reader.bytes.count else {
            throw InstallationIdentityError.invalidSignature
        }
        let r = try reader.readPositiveInteger32()
        let s = try reader.readPositiveInteger32()
        guard reader.offset == sequenceEnd,
              !r.allSatisfy({ $0 == 0 }),
              compare(r, order) < 0,
              !s.allSatisfy({ $0 == 0 }),
              compare(s, order) < 0
        else {
            throw InstallationIdentityError.invalidSignature
        }
        return (r, s)
    }

    private static func compare(_ left: Data, _ right: Data) -> Int {
        for (a, b) in zip(left, right) where a != b {
            return a < b ? -1 : 1
        }
        return 0
    }

    private static func subtract(_ left: Data, _ right: Data) -> Data {
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
        return Data(result)
    }
}

private struct FasciaDERReader {
    let bytes: [UInt8]
    var offset = 0

    mutating func readConstructed(tag: UInt8) throws -> Int {
        guard readByte() == tag else {
            throw InstallationIdentityError.invalidSignature
        }
        return try readLength()
    }

    mutating func readPositiveInteger32() throws -> Data {
        guard readByte() == 0x02 else {
            throw InstallationIdentityError.invalidSignature
        }
        let length = try readLength()
        guard length > 0, offset + length <= bytes.count else {
            throw InstallationIdentityError.invalidSignature
        }
        var integer = Array(bytes[offset ..< offset + length])
        offset += length
        guard integer.first.map({ $0 & 0x80 == 0 }) == true else {
            throw InstallationIdentityError.invalidSignature
        }
        if integer.count > 1, integer.first == 0 {
            guard integer[1] & 0x80 != 0 else {
                throw InstallationIdentityError.invalidSignature
            }
            integer.removeFirst()
        }
        guard integer.count <= 32 else {
            throw InstallationIdentityError.invalidSignature
        }
        integer.insert(
            contentsOf: repeatElement(0, count: 32 - integer.count),
            at: 0
        )
        return Data(integer)
    }

    mutating func readLength() throws -> Int {
        guard let first = readByte() else {
            throw InstallationIdentityError.invalidSignature
        }
        if first & 0x80 == 0 {
            return Int(first)
        }
        let count = Int(first & 0x7f)
        guard count > 0, count <= MemoryLayout<Int>.size,
              offset + count <= bytes.count,
              bytes[offset] != 0
        else {
            throw InstallationIdentityError.invalidSignature
        }
        var value = 0
        for _ in 0 ..< count {
            guard let byte = readByte(), value <= (Int.max >> 8) else {
                throw InstallationIdentityError.invalidSignature
            }
            value = (value << 8) | Int(byte)
        }
        guard value >= 0x80 else {
            throw InstallationIdentityError.invalidSignature
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

extension Data {
    init?(fasciaHex value: String, expectedBytes: Int? = nil) {
        let text = value.hasPrefix("0x") ? String(value.dropFirst(2)) : value
        guard text.count % 2 == 0 else {
            return nil
        }
        var result: [UInt8] = []
        var index = text.startIndex
        while index < text.endIndex {
            let next = text.index(index, offsetBy: 2)
            guard let byte = UInt8(text[index ..< next], radix: 16) else {
                return nil
            }
            result.append(byte)
            index = next
        }
        if let expectedBytes, result.count != expectedBytes {
            return nil
        }
        self = Data(result)
    }

    func fasciaHex(prefix: Bool) -> String {
        let value = map { String(format: "%02x", $0) }.joined()
        return prefix ? "0x\(value)" : value
    }
}
