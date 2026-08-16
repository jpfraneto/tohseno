import Foundation

public enum TohsenoCompanionError: Error, Equatable, Sendable {
    case invalidMnemonic
    case cryptographicFailure
    case invalidEncoding(String)
    case invalidInvitation(String)
    case invitationExpired
    case invitationNotYetValid
    case relayNotAllowed
    case invalidCapability(String)
    case capabilityDenied(CompanionCapability)
    case capabilityRevoked
    case invalidEnvelope(String)
    case envelopeExpired
    case replayDetected
    case identityAlreadyExists
    case identityMissing
    case notPaired
    case workspaceUnavailable
    case commandRejected(String)
    case unsafeStorage
    case responseTooLarge
    case cursorResetRequired(resetBefore: UInt64, head: UInt64)
    case relayFailure(Int)
    case transportUnavailable
}

extension TohsenoCompanionError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .invalidMnemonic:
            "The recovery phrase is not a valid 12-word BIP-39 English phrase."
        case .cryptographicFailure:
            "A companion cryptographic operation failed."
        case let .invalidEncoding(reason):
            "The companion payload has an invalid encoding: \(reason)."
        case let .invalidInvitation(reason):
            "The pairing invitation is invalid: \(reason)."
        case .invitationExpired:
            "The pairing invitation has expired."
        case .invitationNotYetValid:
            "The pairing invitation is not valid yet."
        case .relayNotAllowed:
            "The pairing invitation names an unrecognized relay."
        case let .invalidCapability(reason):
            "The companion capability is invalid: \(reason)."
        case let .capabilityDenied(capability):
            "The paired device was not granted \(capability.rawValue)."
        case .capabilityRevoked:
            "This paired device has been revoked."
        case let .invalidEnvelope(reason):
            "The encrypted companion envelope is invalid: \(reason)."
        case .envelopeExpired:
            "The encrypted companion envelope has expired."
        case .replayDetected:
            "The encrypted companion envelope has already been processed."
        case .identityAlreadyExists:
            "A companion identity already exists on this device."
        case .identityMissing:
            "No companion identity exists on this device."
        case .notPaired:
            "This companion is not paired with a workspace."
        case .workspaceUnavailable:
            "No synchronized workspace snapshot is available."
        case let .commandRejected(reason):
            "The Mac rejected the companion command: \(reason)."
        case .unsafeStorage:
            "The companion storage location is unsafe."
        case .responseTooLarge:
            "The relay response exceeded its size limit."
        case .cursorResetRequired:
            "The relay retention window advanced; a full workspace snapshot is required."
        case let .relayFailure(status):
            "The companion relay returned HTTP \(status)."
        case .transportUnavailable:
            "The companion relay is currently unavailable."
        }
    }
}
