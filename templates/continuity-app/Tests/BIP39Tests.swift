import CryptoKit
import XCTest
@testable import Writing

/// Invariants for the vendored BIP39 implementation. The wordlist must be
/// byte-identical to the canonical English list, and encoding must match
/// the published test vectors — a phrase generated here must be recoverable
/// by any standard BIP39 implementation.
final class BIP39Tests: XCTestCase {
    func testWordlistIsCanonical() throws {
        XCTAssertEqual(BIP39.wordlist.count, 2048)
        XCTAssertEqual(BIP39.wordlist.first, "abandon")
        XCTAssertEqual(BIP39.wordlist.last, "zoo")

        // SHA-256 of the canonical bip-0039 english.txt (one word per line).
        let joined = BIP39.wordlist.joined(separator: "\n") + "\n"
        let digest = SHA256.hash(data: Data(joined.utf8))
            .map { String(format: "%02x", $0) }
            .joined()
        XCTAssertEqual(digest, "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda")
    }

    func testPublishedVectors() throws {
        let vectors: [(entropy: String, mnemonic: String)] = [
            (
                "00000000000000000000000000000000",
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ),
            (
                "7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f",
                "legal winner thank year wave sausage worth useful legal winner thank yellow"
            ),
            (
                "80808080808080808080808080808080",
                "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"
            ),
            (
                "ffffffffffffffffffffffffffffffff",
                "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"
            ),
        ]
        for vector in vectors {
            let entropy = Data(hex: vector.entropy)
            let words = try BIP39.mnemonic(fromEntropy: entropy)
            XCTAssertEqual(words.joined(separator: " "), vector.mnemonic)
            XCTAssertEqual(try BIP39.entropy(fromMnemonic: words), entropy)
        }
    }

    func testGeneratedMnemonicsRoundTripAndVary() throws {
        let first = try BIP39.generateMnemonic()
        let second = try BIP39.generateMnemonic()
        XCTAssertEqual(first.count, 12)
        XCTAssertTrue(BIP39.isValid(first))
        XCTAssertNotEqual(first, second, "two generated phrases colliding means entropy is broken")
    }

    func testChecksumRejectsTamperedPhrase() throws {
        var words = try BIP39.mnemonic(fromEntropy: Data(count: 16))
        words[0] = "zoo"
        XCTAssertFalse(BIP39.isValid(words))
    }

    func testRejectsUnknownWordsAndWrongCounts() {
        XCTAssertThrowsError(try BIP39.entropy(fromMnemonic: ["notaword"] + Array(repeating: "abandon", count: 11)))
        XCTAssertThrowsError(try BIP39.entropy(fromMnemonic: Array(repeating: "abandon", count: 11)))
    }

    func testNormalizeHandlesCaseAndWhitespace() {
        XCTAssertEqual(
            BIP39.normalize("  Legal\twinner\nTHANK  year "),
            ["legal", "winner", "thank", "year"]
        )
    }
}

private extension Data {
    init(hex: String) {
        self.init(stride(from: 0, to: hex.count, by: 2).map { offset in
            let start = hex.index(hex.startIndex, offsetBy: offset)
            let end = hex.index(start, offsetBy: 2)
            return UInt8(hex[start..<end], radix: 16)!
        })
    }
}
