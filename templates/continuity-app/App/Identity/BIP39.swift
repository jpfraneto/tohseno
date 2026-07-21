import CryptoKit
import Foundation

/// Minimal BIP39 implementation: 128-bit entropy to a 12-word English
/// mnemonic and back, with the standard SHA-256 checksum.
///
/// Original implementation against the published BIP-0039 specification.
/// The English wordlist ships as a bundle resource (2048 words, one per
/// line, the canonical list). Everything is offline; nothing here touches
/// the network or any external service.
enum BIP39 {
    enum BIP39Error: Error, Equatable {
        case wordlistUnavailable
        case invalidEntropy
        case invalidWordCount
        case unknownWord(String)
        case checksumMismatch
    }

    /// The canonical English wordlist, loaded once from the app bundle.
    static let wordlist: [String] = {
        guard
            let url = Bundle(for: BundleToken.self).url(forResource: "bip39-english", withExtension: "txt"),
            let text = try? String(contentsOf: url, encoding: .utf8)
        else { return [] }
        return text.split(separator: "\n").map(String.init)
    }()

    /// Generates a fresh 12-word mnemonic from 128 bits of system entropy.
    static func generateMnemonic() throws -> [String] {
        var entropy = [UInt8](repeating: 0, count: 16)
        let status = SecRandomCopyBytes(kSecRandomDefault, entropy.count, &entropy)
        guard status == errSecSuccess else { throw BIP39Error.invalidEntropy }
        return try mnemonic(fromEntropy: Data(entropy))
    }

    /// Deterministically encodes 16 bytes of entropy as 12 words.
    static func mnemonic(fromEntropy entropy: Data) throws -> [String] {
        guard wordlist.count == 2048 else { throw BIP39Error.wordlistUnavailable }
        guard entropy.count == 16 else { throw BIP39Error.invalidEntropy }

        let checksum = Data(SHA256.hash(data: entropy))
        var bits = entropy.flatMap { byte in
            (0..<8).reversed().map { (byte >> $0) & 1 }
        }
        // 128 bits of entropy carry a 4-bit checksum: 132 bits = 12 * 11.
        bits.append(contentsOf: (0..<4).reversed().map { (checksum[0] >> ($0 + 4)) & 1 })

        return stride(from: 0, to: bits.count, by: 11).map { start in
            let index = bits[start..<(start + 11)].reduce(0) { ($0 << 1) | Int($1) }
            return wordlist[index]
        }
    }

    /// Decodes and checksum-verifies a 12-word mnemonic back to entropy.
    @discardableResult
    static func entropy(fromMnemonic words: [String]) throws -> Data {
        guard wordlist.count == 2048 else { throw BIP39Error.wordlistUnavailable }
        guard words.count == 12 else { throw BIP39Error.invalidWordCount }

        var bits: [UInt8] = []
        bits.reserveCapacity(132)
        for word in words {
            guard let index = wordIndex[word] else { throw BIP39Error.unknownWord(word) }
            bits.append(contentsOf: (0..<11).reversed().map { UInt8((index >> $0) & 1) })
        }

        let entropyBits = bits[0..<128]
        let entropy = Data(stride(from: 0, to: 128, by: 8).map { start in
            entropyBits[start..<(start + 8)].reduce(UInt8(0)) { ($0 << 1) | $1 }
        })

        let checksum = Data(SHA256.hash(data: entropy))
        let expected = (0..<4).reversed().map { (checksum[0] >> ($0 + 4)) & 1 }
        guard Array(bits[128..<132]) == expected else { throw BIP39Error.checksumMismatch }
        return entropy
    }

    /// Normalizes user input into candidate words: lowercase, whitespace-split.
    static func normalize(_ phrase: String) -> [String] {
        phrase.lowercased().split(whereSeparator: { $0.isWhitespace || $0.isNewline }).map(String.init)
    }

    /// Returns true when the phrase is 12 known words with a valid checksum.
    static func isValid(_ words: [String]) -> Bool {
        (try? entropy(fromMnemonic: words)) != nil
    }

    private static let wordIndex: [String: Int] = Dictionary(
        uniqueKeysWithValues: wordlist.enumerated().map { ($0.element, $0.offset) }
    )
}

private final class BundleToken {}
