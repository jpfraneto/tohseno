import CryptoKit
import Foundation
import Security

public struct RecoveryPhrase: Equatable, Sendable, CustomStringConvertible {
    private let entropy: Data

    public init(_ phrase: String) throws {
        entropy = try BIP39English.entropy(from: phrase)
    }

    init(entropy: Data) throws {
        guard entropy.count == 16 else {
            throw TohsenoCompanionError.invalidMnemonic
        }
        self.entropy = entropy
    }

    /// Recovery words are returned only through this explicit reveal operation.
    /// `description`, reflection, persistence records, and transport models never
    /// expose them.
    public func reveal() -> String {
        // The initializer already established the exact entropy width.
        try! BIP39English.phrase(from: entropy)
    }

    public var description: String { "<12-word TOHSENO companion recovery phrase>" }

    func seed(passphrase: String = "") -> Data {
        BIP39English.seed(phrase: reveal(), passphrase: passphrase)
    }

    var rawEntropy: Data { entropy }
}

public protocol CompanionEntropySource: Sendable {
    func randomBytes(count: Int) throws -> Data
}

public struct SystemCompanionEntropySource: CompanionEntropySource, Sendable {
    public init() {}

    public func randomBytes(count: Int) throws -> Data {
        guard count > 0, count <= 1024 else {
            throw TohsenoCompanionError.cryptographicFailure
        }
        var bytes = [UInt8](repeating: 0, count: count)
        guard SecRandomCopyBytes(kSecRandomDefault, count, &bytes) == errSecSuccess else {
            throw TohsenoCompanionError.cryptographicFailure
        }
        return Data(bytes)
    }
}

enum BIP39English {
    static func phrase(from entropy: Data) throws -> String {
        guard entropy.count == 16 else { throw TohsenoCompanionError.invalidMnemonic }
        let checksum = Data(SHA256.hash(data: entropy))[0] >> 4
        var indices: [Int] = []
        indices.reserveCapacity(12)
        for wordIndex in 0 ..< 12 {
            var value = 0
            for offset in 0 ..< 11 {
                let bitIndex = wordIndex * 11 + offset
                let bit: UInt8
                if bitIndex < 128 {
                    bit = (entropy[bitIndex / 8] >> UInt8(7 - bitIndex % 8)) & 1
                } else {
                    bit = (checksum >> UInt8(3 - (bitIndex - 128))) & 1
                }
                value = (value << 1) | Int(bit)
            }
            indices.append(value)
        }
        return indices.map { BIP39EnglishWords.words[$0] }.joined(separator: " ")
    }

    static func entropy(from phraseText: String) throws -> Data {
        let normalized = phraseText.decomposedStringWithCompatibilityMapping
        let words = normalized.split(whereSeparator: \.isWhitespace).map(String.init)
        guard words.count == 12 else { throw TohsenoCompanionError.invalidMnemonic }
        let byWord = Dictionary(uniqueKeysWithValues: BIP39EnglishWords.words.enumerated().map {
            ($0.element, $0.offset)
        })
        var bits = [UInt8]()
        bits.reserveCapacity(132)
        for word in words {
            guard let index = byWord[word] else { throw TohsenoCompanionError.invalidMnemonic }
            for shift in stride(from: 10, through: 0, by: -1) {
                bits.append(UInt8((index >> shift) & 1))
            }
        }
        var entropy = Data(repeating: 0, count: 16)
        for bitIndex in 0 ..< 128 where bits[bitIndex] == 1 {
            entropy[bitIndex / 8] |= UInt8(1 << (7 - bitIndex % 8))
        }
        let checksum = Data(SHA256.hash(data: entropy))[0] >> 4
        var observed: UInt8 = 0
        for index in 128 ..< 132 {
            observed = (observed << 1) | bits[index]
        }
        guard observed == checksum,
              try phrase(from: entropy) == words.joined(separator: " ")
        else {
            throw TohsenoCompanionError.invalidMnemonic
        }
        return entropy
    }

    static func seed(phrase: String, passphrase: String) -> Data {
        let password = Data(phrase.decomposedStringWithCompatibilityMapping.utf8)
        let salt = Data(("mnemonic" + passphrase.decomposedStringWithCompatibilityMapping).utf8)
        return pbkdf2SHA512(password: password, salt: salt, iterations: 2048)
    }

    private static func pbkdf2SHA512(password: Data, salt: Data, iterations: Int) -> Data {
        precondition(iterations > 0)
        let key = SymmetricKey(data: password)
        var initial = salt
        initial.append(contentsOf: [0, 0, 0, 1])
        var current = Data(HMAC<SHA512>.authenticationCode(for: initial, using: key))
        var result = current
        if iterations > 1 {
            for _ in 2 ... iterations {
                current = Data(HMAC<SHA512>.authenticationCode(for: current, using: key))
                for index in result.indices {
                    result[index] ^= current[index]
                }
            }
        }
        return result
    }
}
