import Foundation

/// Claims use a separately signed activation. These values remain nil in an
/// unreleased client, keeping every write surface dark until an exact deployed
/// contract and activation digest are deliberately pinned into a release.
public enum ClaimsClientActivation {
    public static let shotRegistry = "0x3fe6508ba2660bc575080024f402c192a2e035a0"
    public static let claimsContract: String? = nil
    public static let activationSigningDigest: String? = nil
}

public struct ClaimEditionPolicy: Codable, Equatable, Sendable {
    public enum Kind: String, Codable, Equatable, Hashable, Sendable {
        case open
        case limited
        case timed
        case limitedTimed = "limited_timed"
    }

    public let maxClaims: UInt64
    public let closesAt: UInt64

    enum CodingKeys: String, CodingKey {
        case maxClaims = "max_claims"
        case closesAt = "closes_at"
    }

    public init(maxClaims: UInt64 = 0, closesAt: UInt64 = 0) throws {
        guard maxClaims <= ClaimsActionEncoding.maximumSafeInteger,
              closesAt <= ClaimsActionEncoding.maximumSafeInteger
        else { throw TohsenoCompanionError.invalidEncoding("Claim Edition policy is outside its bound") }
        self.maxClaims = maxClaims
        self.closesAt = closesAt
    }

    public var kind: Kind {
        switch (maxClaims == 0, closesAt == 0) {
        case (true, true): .open
        case (false, true): .limited
        case (true, false): .timed
        case (false, false): .limitedTimed
        }
    }
}

public struct OpenClaimEditionAction: Codable, Equatable, Sendable {
    public let shotRegistry: String
    public let shotID: String
    public let maxClaims: UInt64
    public let closesAt: UInt64
    public let controller: String
    public let nonce: UInt64
    public let deadline: UInt64

    enum CodingKeys: String, CodingKey {
        case shotRegistry = "shot_registry"
        case shotID = "shot_id"
        case maxClaims = "max_claims"
        case closesAt = "closes_at"
        case controller, nonce, deadline
    }

    public init(
        shotRegistry: String,
        shotID: String,
        maxClaims: UInt64,
        closesAt: UInt64,
        controller: String,
        nonce: UInt64,
        deadline: UInt64
    ) {
        self.shotRegistry = shotRegistry
        self.shotID = shotID
        self.maxClaims = maxClaims
        self.closesAt = closesAt
        self.controller = controller
        self.nonce = nonce
        self.deadline = deadline
    }

    public func structHash(expectedRegistry: String) throws -> Data {
        guard shotRegistry == expectedRegistry,
              let registry = ClaimsActionEncoding.addressWord(shotRegistry),
              let shot = ClaimsActionEncoding.hex32(shotID),
              let controller = ClaimsActionEncoding.addressWord(controller),
              maxClaims <= ClaimsActionEncoding.maximumSafeInteger,
              closesAt <= ClaimsActionEncoding.maximumSafeInteger,
              nonce <= ClaimsActionEncoding.maximumSafeInteger,
              deadline > 0, deadline <= ClaimsActionEncoding.maximumSafeInteger
        else { throw TohsenoCompanionError.invalidEncoding("invalid OpenClaimEdition action") }
        return Keccak256.hash(
            ClaimsActionEncoding.openTypeHash + registry + shot
                + ClaimsActionEncoding.word(maxClaims) + ClaimsActionEncoding.word(closesAt)
                + controller + ClaimsActionEncoding.word(nonce) + ClaimsActionEncoding.word(deadline)
        )
    }

    public func digest(
        chainID: UInt64 = ClaimsActionEncoding.activeChainID,
        claimsContract: String,
        expectedRegistry: String
    ) throws -> Data {
        let domain = try ClaimsActionEncoding.domainSeparator(
            chainID: chainID,
            claimsContract: claimsContract
        )
        return Keccak256.hash(Data([0x19, 0x01]) + domain + (try structHash(expectedRegistry: expectedRegistry)))
    }
}

public struct SoftwareClaimAction: Codable, Equatable, Sendable {
    public let shotRegistry: String
    public let shotID: String
    public let claimant: String
    public let releaseDigest: String
    public let checkpointDigest: String
    public let gestureCommitment: String
    public let nonce: UInt64
    public let deadline: UInt64

    enum CodingKeys: String, CodingKey {
        case shotRegistry = "shot_registry"
        case shotID = "shot_id"
        case claimant
        case releaseDigest = "release_digest"
        case checkpointDigest = "checkpoint_digest"
        case gestureCommitment = "gesture_commitment"
        case nonce, deadline
    }

    public init(
        shotRegistry: String,
        shotID: String,
        claimant: String,
        releaseDigest: String,
        checkpointDigest: String,
        gestureCommitment: String,
        nonce: UInt64,
        deadline: UInt64
    ) {
        self.shotRegistry = shotRegistry
        self.shotID = shotID
        self.claimant = claimant
        self.releaseDigest = releaseDigest
        self.checkpointDigest = checkpointDigest
        self.gestureCommitment = gestureCommitment
        self.nonce = nonce
        self.deadline = deadline
    }

    public func structHash(expectedRegistry: String) throws -> Data {
        guard shotRegistry == expectedRegistry,
              let registry = ClaimsActionEncoding.addressWord(shotRegistry),
              let shot = ClaimsActionEncoding.hex32(shotID),
              let claimant = ClaimsActionEncoding.addressWord(claimant),
              let release = ClaimsActionEncoding.hex32(releaseDigest),
              let checkpoint = ClaimsActionEncoding.hex32(checkpointDigest),
              let gesture = ClaimsActionEncoding.hex32(gestureCommitment),
              nonce <= ClaimsActionEncoding.maximumSafeInteger,
              deadline > 0, deadline <= ClaimsActionEncoding.maximumSafeInteger
        else { throw TohsenoCompanionError.invalidEncoding("invalid ClaimSoftware action") }
        return Keccak256.hash(
            ClaimsActionEncoding.claimTypeHash + registry + shot + claimant + release + checkpoint
                + gesture + ClaimsActionEncoding.word(nonce) + ClaimsActionEncoding.word(deadline)
        )
    }

    public func digest(
        chainID: UInt64 = ClaimsActionEncoding.activeChainID,
        claimsContract: String,
        expectedRegistry: String
    ) throws -> Data {
        let domain = try ClaimsActionEncoding.domainSeparator(
            chainID: chainID,
            claimsContract: claimsContract
        )
        return Keccak256.hash(Data([0x19, 0x01]) + domain + (try structHash(expectedRegistry: expectedRegistry)))
    }
}

public struct SoftwareClaimAuthorization: Codable, Equatable, Sendable {
    public let action: SoftwareClaimAction
    public let digest: String
    public let signature: BuilderDeviceSignature

    public init(action: SoftwareClaimAction, digest: String, signature: BuilderDeviceSignature) throws {
        guard signature.digest == digest else {
            throw TohsenoCompanionError.invalidEncoding("Software Claim signature digest differs")
        }
        self.action = action
        self.digest = digest
        self.signature = signature
    }
}

public enum ClaimsActionEncoding {
    public static let activeChainID: UInt64 = 4663
    public static let maximumSafeInteger: UInt64 = 9_007_199_254_740_991
    public static let domainName = "TOHSENO Claims"
    public static let domainVersion = "1"
    public static let openType = "OpenClaimEdition(address shotRegistry,bytes32 shotId,uint64 maxClaims,uint64 closesAt,address controller,uint64 nonce,uint64 deadline)"
    public static let claimType = "ClaimSoftware(address shotRegistry,bytes32 shotId,address claimant,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment,uint64 nonce,uint64 deadline)"

    static let openTypeHash = Keccak256.hash(Data(openType.utf8))
    static let claimTypeHash = Keccak256.hash(Data(claimType.utf8))

    public static var openTypeHashHex: String { openTypeHash.prefixedHex }
    public static var claimTypeHashHex: String { claimTypeHash.prefixedHex }

    public static func domainSeparator(chainID: UInt64, claimsContract: String) throws -> Data {
        guard chainID == activeChainID,
              let contract = addressWord(claimsContract)
        else { throw TohsenoCompanionError.invalidEncoding("Claims domain is not active") }
        let domainType = Keccak256.hash(Data(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)".utf8
        ))
        return Keccak256.hash(
            domainType + Keccak256.hash(Data(domainName.utf8))
                + Keccak256.hash(Data(domainVersion.utf8)) + word(chainID) + contract
        )
    }

    static func word(_ value: UInt64) -> Data {
        var data = Data(repeating: 0, count: 32)
        for offset in 0 ..< 8 {
            data[31 - offset] = UInt8(truncatingIfNeeded: value >> UInt64(offset * 8))
        }
        return data
    }

    static func addressWord(_ value: String) -> Data? {
        guard let address = hex(value, bytes: 20), address != Data(repeating: 0, count: 20)
        else { return nil }
        return Data(repeating: 0, count: 12) + address
    }

    static func hex32(_ value: String) -> Data? {
        guard let digest = hex(value, bytes: 32), digest != Data(repeating: 0, count: 32)
        else { return nil }
        return digest
    }

    private static func hex(_ value: String, bytes: Int) -> Data? {
        guard value.hasPrefix("0x"), value.count == 2 + bytes * 2,
              value.dropFirst(2).allSatisfy({ $0.isNumber || ("a" ... "f").contains($0) })
        else { return nil }
        var data = Data(capacity: bytes)
        var index = value.index(value.startIndex, offsetBy: 2)
        for _ in 0 ..< bytes {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index ..< next], radix: 16) else { return nil }
            data.append(byte)
            index = next
        }
        return data
    }
}

private extension Data {
    var prefixedHex: String {
        "0x" + map { String(format: "%02x", $0) }.joined()
    }
}
