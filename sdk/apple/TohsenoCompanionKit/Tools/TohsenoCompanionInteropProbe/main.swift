import Foundation
import TohsenoCompanionKit

private struct ProbeResult: Encodable {
    let schema = "tohseno.companion-swift-interop-probe/1"
    let operation: String
    let invitationValid: Bool?
    let canonicalSchema: Bool?
    let officialRelay: Bool?
    let signedInvitation: Bool?
    let workspaceSnapshotReceived: Bool?
    let shotCount: Int?
    let exactVersionCount: Int?
    let iconCount: Int?
    let identityRestored: Bool?
    let commandReceived: Bool?
    let revocationObserved: Bool?
}

private func emit(_ result: ProbeResult) throws {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys]
    FileHandle.standardOutput.write(try encoder.encode(result))
    FileHandle.standardOutput.write(Data("\n".utf8))
}

private func readInvitation(_ path: String) throws -> String {
    let value = try String(contentsOfFile: path, encoding: .utf8)
        .trimmingCharacters(in: .whitespacesAndNewlines)
    guard value.hasPrefix(PairingInvitation.uriPrefix) else {
        throw TohsenoCompanionError.invalidInvitation("unexpected URI prefix")
    }
    return value
}

private func endpoint(_ origin: String) throws -> RelayEndpoint {
    guard let url = URL(string: origin) else {
        throw TohsenoCompanionError.relayNotAllowed
    }
    return try RelayEndpoint(
        id: "official-v1",
        baseURL: url,
        allowLoopbackHTTP: true
    )
}

private func client(stateDirectory: String, relayOrigin: String) throws -> TohsenoCompanionClient {
    let root = URL(fileURLWithPath: stateDirectory, isDirectory: true)
    let stateStore = try FileCompanionStateStore(fileURL: root.appendingPathComponent("state.bin"))
    let payloadStore = try FileCompanionPayloadStore(directoryURL: root.appendingPathComponent("outbox"))
    let service = ProcessInfo.processInfo.environment["TOHSENO_SWIFT_PROBE_KEYCHAIN_SERVICE"]
        ?? "org.tohseno.companion.identity.verification"
    let account = ProcessInfo.processInfo.environment["TOHSENO_SWIFT_PROBE_KEYCHAIN_ACCOUNT"]
        ?? "probe"
    return TohsenoCompanionClient(
        identityStore: KeychainCompanionSecretStore(service: service, account: account),
        stateStore: stateStore,
        payloadStore: payloadStore,
        relayAllowlist: try RelayAllowlist([endpoint(relayOrigin)])
    )
}

@main
private enum Main {
    static func main() async throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        guard let operation = arguments.first else {
            throw TohsenoCompanionError.invalidEncoding("probe operation is required")
        }
        switch operation {
        case "validate":
            guard arguments.count == 3 else {
                throw TohsenoCompanionError.invalidEncoding("validate requires URI file and relay origin")
            }
            let invitationURI = try readInvitation(arguments[1])
            let parsed = try PairingInvitation.parse(
                uri: invitationURI,
                allowlist: try RelayAllowlist([endpoint(arguments[2])])
            ).0
            try emit(ProbeResult(
                operation: operation,
                invitationValid: true,
                canonicalSchema: parsed.schema == PairingInvitation.schemaV1,
                officialRelay: parsed.relayID == "official-v1",
                signedInvitation: !parsed.signature.isEmpty,
                workspaceSnapshotReceived: nil,
                shotCount: nil,
                exactVersionCount: nil,
                iconCount: nil,
                identityRestored: nil,
                commandReceived: nil,
                revocationObserved: nil
            ))
        case "pair":
            guard arguments.count == 4 else {
                throw TohsenoCompanionError.invalidEncoding("pair requires state directory, URI file, and relay origin")
            }
            let companion = try client(stateDirectory: arguments[1], relayOrigin: arguments[3])
            let existingIdentity = try await companion.publicIdentity()
            let invitationURI = try readInvitation(arguments[2])
            try await companion.pair(with: invitationURI, displayName: "Swift interoperability probe")
            let snapshot = try await companion.currentWorkspace()
            var icons = 0
            for shot in snapshot.shots {
                if let descriptor = shot.icon,
                   try await companion.iconBlob(for: descriptor) != nil {
                    icons += 1
                }
            }
            try emit(ProbeResult(
                operation: operation,
                invitationValid: true,
                canonicalSchema: true,
                officialRelay: true,
                signedInvitation: true,
                workspaceSnapshotReceived: true,
                shotCount: snapshot.shots.count,
                exactVersionCount: snapshot.shots.filter { $0.latestVersionID != nil }.count,
                iconCount: icons,
                identityRestored: !existingIdentity.deviceID.isEmpty,
                commandReceived: nil,
                revocationObserved: nil
            ))
        case "create-identity":
            guard arguments.count == 3 else {
                throw TohsenoCompanionError.invalidEncoding("create-identity requires state directory and relay origin")
            }
            let companion = try client(stateDirectory: arguments[1], relayOrigin: arguments[2])
            _ = try await companion.createIdentity()
            try emit(ProbeResult(
                operation: operation,
                invitationValid: nil,
                canonicalSchema: nil,
                officialRelay: nil,
                signedInvitation: nil,
                workspaceSnapshotReceived: nil,
                shotCount: nil,
                exactVersionCount: nil,
                iconCount: nil,
                identityRestored: true,
                commandReceived: nil,
                revocationObserved: nil
            ))
        case "reconcile":
            guard arguments.count == 3 else {
                throw TohsenoCompanionError.invalidEncoding("reconcile requires state directory and relay origin")
            }
            let companion = try client(stateDirectory: arguments[1], relayOrigin: arguments[2])
            _ = try await companion.publicIdentity()
            try await companion.reconcile()
            let snapshot = try await companion.currentWorkspace()
            try emit(ProbeResult(
                operation: operation,
                invitationValid: nil,
                canonicalSchema: nil,
                officialRelay: true,
                signedInvitation: nil,
                workspaceSnapshotReceived: true,
                shotCount: snapshot.shots.count,
                exactVersionCount: snapshot.shots.filter { $0.latestVersionID != nil }.count,
                iconCount: snapshot.shots.filter { $0.icon != nil }.count,
                identityRestored: true,
                commandReceived: nil,
                revocationObserved: nil
            ))
        case "request-snapshot":
            guard arguments.count == 4 else {
                throw TohsenoCompanionError.invalidEncoding(
                    "request-snapshot requires state directory, relay origin, and command ID"
                )
            }
            let companion = try client(stateDirectory: arguments[1], relayOrigin: arguments[2])
            let receipt = try await companion.requestWorkspaceSnapshot(commandID: arguments[3])
            try emit(ProbeResult(
                operation: operation,
                invitationValid: nil,
                canonicalSchema: nil,
                officialRelay: true,
                signedInvitation: nil,
                workspaceSnapshotReceived: nil,
                shotCount: nil,
                exactVersionCount: nil,
                iconCount: nil,
                identityRestored: true,
                commandReceived: receipt.state == .received,
                revocationObserved: nil
            ))
        case "expect-revoked":
            guard arguments.count == 3 else {
                throw TohsenoCompanionError.invalidEncoding(
                    "expect-revoked requires state directory and relay origin"
                )
            }
            let companion = try client(stateDirectory: arguments[1], relayOrigin: arguments[2])
            do {
                try await companion.reconcile()
                try emit(ProbeResult(
                    operation: operation,
                    invitationValid: nil,
                    canonicalSchema: nil,
                    officialRelay: true,
                    signedInvitation: nil,
                    workspaceSnapshotReceived: nil,
                    shotCount: nil,
                    exactVersionCount: nil,
                    iconCount: nil,
                    identityRestored: true,
                    commandReceived: nil,
                    revocationObserved: false
                ))
            } catch TohsenoCompanionError.capabilityRevoked {
                try emit(ProbeResult(
                    operation: operation,
                    invitationValid: nil,
                    canonicalSchema: nil,
                    officialRelay: true,
                    signedInvitation: nil,
                    workspaceSnapshotReceived: nil,
                    shotCount: nil,
                    exactVersionCount: nil,
                    iconCount: nil,
                    identityRestored: true,
                    commandReceived: nil,
                    revocationObserved: true
                ))
            }
        default:
            throw TohsenoCompanionError.invalidEncoding("unsupported probe operation")
        }
    }
}
