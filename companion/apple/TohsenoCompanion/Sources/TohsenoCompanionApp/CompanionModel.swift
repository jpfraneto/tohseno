import Foundation
import TohsenoCompanionKit
import TohsenoWorkshopKit
#if os(iOS)
import AVFoundation
import CoreMotion
import UIKit
#endif

public protocol SoftwareClaimIdentity: Sendable {
    func claimPublicIdentity() async throws -> BuilderDevicePublicIdentity
    func claimBuilderID() async throws -> String
    func signClaimDigest(_ digestHex: String) async throws -> BuilderDeviceAuthorization
}

extension BuilderDeviceIdentity: SoftwareClaimIdentity {
    public func claimPublicIdentity() async throws -> BuilderDevicePublicIdentity {
        try ensureCreated()
    }

    public func claimBuilderID() async throws -> String {
        try builderID()
    }

    public func signClaimDigest(_ digestHex: String) async throws -> BuilderDeviceAuthorization {
        try sign(digestHex: digestHex)
    }
}

public struct ClaimedSoftwareEncounter: Codable, Equatable, Identifiable, Sendable {
    public let app: PublicAppRelease
    public let claim: PublicSoftwareClaim
    public let canonicalMarkHex: String
    public var id: String { claim.tokenID }
}

private struct PendingSoftwareClaim: Codable, Equatable, Sendable {
    let app: PublicAppRelease
    let preparation: SoftwareClaimPreparation
    let canonicalMarkHex: String
}

/// Everything the TOHSENO Companion knows.
///
/// The phone is a remote control for intent. It holds no factory state of its
/// own beyond an unsent draft: the Mac is authoritative for apps, versions, and
/// execution, and the SDK owns the durable outbox that survives the app being
/// closed.
@MainActor
@Observable
public final class CompanionModel {
    public let workshopRuntime: WorkshopClientRuntime
    public enum Screen: Equatable {
        case loading
        case firstRun
        case entitlementDecision
        case trialEnded
        case apps
        case create
        case app(String)
    }

    /// Startup begins here while persisted pairing and the last synchronized
    /// workspace are restored. This prevents a paired iPhone from briefly
    /// showing first-run setup or an empty app grid on every launch.
    public private(set) var screen: Screen = .loading
    public private(set) var apps: [ShotSummary] = []
    public private(set) var icons: [String: Data] = [:]
    public private(set) var connection: CompanionConnectionState = .disconnected
    /// Signed commands still waiting to reach the Mac.
    public private(set) var unacknowledged = 0
    /// A human sentence about the last thing that went wrong, or nil.
    public private(set) var notice: String?
    public private(set) var recoveryWords: String?
    public private(set) var busy = false
    public private(set) var syncing = false
    public private(set) var entitlement: ProductEntitlementProjection?
    public private(set) var publicApps: [PublicAppRelease] = []
    public private(set) var publicTimeline: [PublicTimelineEvent] = []
    public private(set) var followedBuilderIDs: Set<String> = []
    public private(set) var builderDevice: BuilderDevicePublicIdentity?
    public private(set) var networkNotice: String?
    public private(set) var pendingPublication: PublicationApprovalRequest?
    public var linkedPublicRelease: PublicAppRelease?
    public var profileDisplayName = ""
    public var profileHandle = ""
    public var requestedAlias = ""
    public var requestedAliasShotID = ""
    public private(set) var lastAliasRequestID: String?
    public private(set) var builderID: String?
    public private(set) var profileNotice: String?
    public private(set) var claimEditions: [String: PublicClaimEdition] = [:]
    public private(set) var claimStates: [String: String] = [:]
    public private(set) var claimedSoftware: [ClaimedSoftwareEncounter] = []
    public private(set) var privateUpdates: [PrivateUpdateItem] = []
    public var claimsActive: Bool { network.claims.active }
    public var aliasEligibleApps: [PublicAppRelease] {
        guard let builderID else { return [] }
        return publicApps.filter {
            $0.release.builderID == builderID && $0.release.permissions.installAllowed
        }
    }
    public var selectedAliasAppSlug: String? {
        aliasEligibleApps.first(where: {
            $0.release.shotID == requestedAliasShotID
        })?.release.display.appSlug
    }
    public var canRequestGlobalAlias: Bool {
        !requestedAliasShotID.isEmpty
            && (!requestedAlias.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                || selectedAliasAppSlug != nil)
    }

    /// The one text box. Nothing else is composed on the phone.
    public var intent = ""
    public var appName = ""
    public var attachments: [CompanionReferenceBlob] = []

    /// Apps this phone has just sent a request for, before the Mac's snapshot
    /// catches up. Cleared as soon as the Mac reports an execution.
    private var justSent: Set<String> = []
    private var pendingCableInvitation: String?
    private let backend: any CompanionBackend
    private let deviceName: String
    private let network: PublicNetworkClient
    private let builderIdentity: BuilderDeviceIdentity
    private let claimIdentity: any SoftwareClaimIdentity
    private let storage: UserDefaults
    private var hasStarted = false
    private var profileNonce: UInt64 = 0
    private var aliasNonce: UInt64 = 0
    private var pendingSoftwareClaim: PendingSoftwareClaim?
    private var isPollingClaim = false
    private static let pendingClaimKey = "tohseno.pending-software-claim.v1"
    private static let claimedSoftwareKey = "tohseno.claimed-software.v1"
    private static let followedBuildersKey = "tohseno.followed-builders.v1"

    public init(
        backend: any CompanionBackend,
        deviceName: String,
        network: PublicNetworkClient = .production,
        builderIdentity: BuilderDeviceIdentity = BuilderDeviceIdentity(),
        claimIdentity: (any SoftwareClaimIdentity)? = nil,
        storage: UserDefaults = .standard
    ) {
        self.backend = backend
        self.deviceName = deviceName
        workshopRuntime = WorkshopClientRuntime(
            authorizer: backend,
            localDeviceName: deviceName
        )
        self.network = network
        self.builderIdentity = builderIdentity
        self.claimIdentity = claimIdentity ?? builderIdentity
        self.storage = storage
    }

    public func start() {
        guard !hasStarted else { return }
        hasStarted = true
        restoreClaimMemory()
        refreshWorkshopCapabilityTruth()
        Task { [backend] in
            for await connection in backend.connectionStates {
                self.connection = connection
                if connection == .revoked {
                    self.workshopRuntime.stop()
                    self.notice = "This iPhone no longer has access to your Mac."
                    self.screen = .firstRun
                }
            }
        }
        Task { [backend] in
            for await event in backend.events {
                self.apply(event)
            }
        }
        Task {
            await workshopRuntime.start()
            await TohsenoWorkshop.current.use(session: workshopRuntime)
        }
        Task {
            for await event in workshopRuntime.events where event.event.type == "workshop.pulse" {
                self.receiveWorkshopPulse()
            }
        }
        Task { await refresh() }
        Task { await refreshPublicNetwork() }
        Task { await resumeSoftwareClaimIfNeeded() }
        Task {
            do {
                let identity = try await builderIdentity.ensureCreated()
                builderDevice = identity
                await refreshBuilderProfile()
                let announcement = BuilderDeviceAnnouncement(publicIdentity: identity)
                _ = try await backend.announceBuilderDevice(
                    announcement,
                    commandID: Self.builderAnnouncementCommandID()
                )
            }
            catch { builderDevice = nil }
        }
    }

    static func builderAnnouncementCommandID() -> String {
        "builder_announce_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
    }

    public func sendWorkshopPulse() async {
        refreshWorkshopCapabilityTruth()
        do {
            try await workshopRuntime.sendPulse()
            notice = nil
        } catch {
            notice = error.localizedDescription
        }
    }

    public func refreshWorkshopCapabilityTruth() {
#if os(iOS)
        workshopRuntime.setLocalPermissions(
            camera: Self.workshopPermission(AVCaptureDevice.authorizationStatus(for: .video)),
            microphone: Self.workshopPermission(AVCaptureDevice.authorizationStatus(for: .audio)),
            motion: Self.workshopPermission(CMMotionActivityManager.authorizationStatus())
        )
#endif
    }

#if os(iOS)
    private static func workshopPermission(_ status: AVAuthorizationStatus) -> WorkshopPermission {
        switch status {
        case .authorized: .authorized
        case .denied: .denied
        case .restricted: .restricted
        case .notDetermined: .notRequested
        @unknown default: .unknown
        }
    }

    private static func workshopPermission(_ status: CMAuthorizationStatus) -> WorkshopPermission {
        switch status {
        case .authorized: .authorized
        case .denied: .denied
        case .restricted: .restricted
        case .notDetermined: .notRequested
        @unknown default: .unknown
        }
    }
#endif

    private func receiveWorkshopPulse() {
#if os(iOS)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
#endif
    }

    public func refreshPublicNetwork() async {
        do {
            async let releases = network.releases()
            async let timeline = network.timeline()
            publicApps = try await releases
            publicTimeline = try await timeline
            selectDefaultAliasAppIfNeeded()
            await deriveClaimedAppUpdates()
            networkNotice = nil
        } catch {
            networkNotice = "The public Registry is temporarily unavailable. Your private Mac connection is unaffected."
        }
    }

    public func toggleFollow(builderID: String) {
        guard builderID.range(of: #"^eip155:4663:0x[0-9a-f]{40}$"#, options: .regularExpression) != nil else {
            return
        }
        let followed = !followedBuilderIDs.contains(builderID)
        if followed { followedBuilderIDs.insert(builderID) }
        else { followedBuilderIDs.remove(builderID) }
        storage.set(followedBuilderIDs.sorted(), forKey: Self.followedBuildersKey)
        let commandID = "follow_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
        Task {
            do {
                _ = try await backend.setBuilderFollow(
                    builderID: builderID,
                    followed: followed,
                    commandID: commandID
                )
                unacknowledged = (try? await backend.unacknowledgedCommandCount()) ?? unacknowledged
            } catch {
                if followed { followedBuilderIDs.remove(builderID) }
                else { followedBuilderIDs.insert(builderID) }
                storage.set(
                    followedBuilderIDs.sorted(),
                    forKey: Self.followedBuildersKey
                )
                notice = "That private Follow could not be sent to your Mac yet."
            }
        }
    }

    public func refreshClaimState(for app: PublicAppRelease) async {
        guard network.claims.active else { return }
        do {
            async let edition = network.claimEdition(shotID: app.release.shotID)
            let identity = try await builderIdentity.builderID()
            guard let claimant = identity.split(separator: ":").last.map(String.init) else { return }
            async let existing = network.softwareClaim(shotID: app.release.shotID, claimant: claimant)
            claimEditions[app.release.shotID] = try await edition
            if let receipt = try await existing {
                claimStates[app.release.shotID] = "Claimed #\(receipt.claimNumber)"
            }
        } catch {
            // Claims remain deliberately absent when the separately activated
            // contract, indexer, or this client's activation pin is unavailable.
        }
    }

    public func claim(_ app: PublicAppRelease, mark: ClaimMark) async {
        guard !busy, pendingSoftwareClaim == nil else { return }
        busy = true
        claimStates[app.release.shotID] = "Claiming…"
        defer { busy = false }
        do {
            let publicIdentity = try await claimIdentity.claimPublicIdentity()
            let announcement = BuilderDeviceAnnouncement(publicIdentity: publicIdentity)
            let claimantID = try await claimIdentity.claimBuilderID()
            guard let claimant = claimantID.split(separator: ":").last.map(String.init) else {
                throw TohsenoCompanionError.invalidEncoding("Tohseno address is invalid")
            }
            let preparation = try await network.prepareSoftwareClaim(
                app: app, claimant: claimant, mark: mark, builderDevice: announcement
            )
            let action = SoftwareClaimAction(
                shotRegistry: preparation.shotRegistry,
                shotID: preparation.shotID,
                claimant: preparation.claimant,
                releaseDigest: preparation.releaseDigest,
                checkpointDigest: preparation.checkpointDigest,
                gestureCommitment: preparation.gestureCommitment,
                nonce: preparation.nonce,
                deadline: preparation.deadline
            )
            let digest = try action.digest(
                chainID: preparation.chainID,
                claimsContract: preparation.claimsContract,
                expectedRegistry: preparation.shotRegistry
            ).prefixedHex
            let signed = BuilderDeviceSignature(try await claimIdentity.signClaimDigest(digest))
            let authorization = try SoftwareClaimAuthorization(
                action: action, digest: digest, signature: signed
            )
            _ = try await network.submitSoftwareClaim(authorization, preparation: preparation)
            let pending = PendingSoftwareClaim(app: app, preparation: preparation,
                canonicalMarkHex: mark.canonicalBytes.prefixedHex)
            pendingSoftwareClaim = pending
            try persist(pending, key: Self.pendingClaimKey)
            await resumeSoftwareClaimIfNeeded()
        } catch {
            claimStates[app.release.shotID] = "Claim unavailable"
            networkNotice = !network.claims.active
                ? "Claims will open after the signed Claims contract and released Companion are activated."
                : "This Claim could not be authorized safely. Refresh the app and try again."
        }
    }

    public func resumeSoftwareClaimIfNeeded() async {
        guard !isPollingClaim,
              network.claims.active else { return }
        isPollingClaim = true
        defer { isPollingClaim = false }
        while let pending = pendingSoftwareClaim, !Task.isCancelled {
            do {
                let status = try await network.softwareClaimStatus(pending.preparation)
                if status.status == "failed" {
                    claimStates[pending.app.release.shotID] = "Closed"
                    networkNotice = status.failure ?? "This edition closed before your Claim confirmed."
                    clearPendingClaim()
                    return
                }
                guard status.status == "complete", let receipt = status.claim else {
                    claimStates[pending.app.release.shotID] = "Claiming…"
                    try? await Task.sleep(for: .seconds(2))
                    continue
                }
                let encounter = ClaimedSoftwareEncounter(app: pending.app, claim: receipt,
                    canonicalMarkHex: pending.canonicalMarkHex)
                if !claimedSoftware.contains(where: { $0.claim.tokenID == receipt.tokenID }) {
                    claimedSoftware.append(encounter)
                    try persist(claimedSoftware, key: Self.claimedSoftwareKey)
                }
                await recordPrivateUpdate(PrivateUpdateItem(
                    kind: .claimed,
                    subjectID: receipt.shotID,
                    evidenceID: receipt.transactionHash ?? receipt.tokenID,
                    title: "You claimed (pending.app.release.display.name)",
                    detail: "Claim #\(receipt.claimNumber) is canonical. This exact release is preparing on your Mac.",
                    occurredAt: Self.canonicalNow()
                ))
                claimStates[pending.app.release.shotID] = "Claimed #\(receipt.claimNumber)"
                networkNotice = connection == .connected
                    ? "Claimed #\(receipt.claimNumber). Preparing this exact release on your Mac."
                    : "Claimed #\(receipt.claimNumber). Waiting for your Mac."
                do {
                    _ = try await backend.requestNetworkRelease(
                        action: .install,
                        shotID: pending.app.release.shotID,
                        releaseDigest: receipt.releaseDigest,
                        commandID: "claim_install_\(receipt.tokenID)"
                    )
                    clearPendingClaim()
                } catch {
                    // The canonical Claim is complete. Keep the durable preparation
                    // token and retry only the exact post-Claim Mac intention later.
                }
                return
            } catch {
                claimStates[pending.app.release.shotID] = "Claiming…"
                return
            }
        }
    }

    private func restoreClaimMemory() {
        let decoder = JSONDecoder()
        if let data = storage.data(forKey: Self.pendingClaimKey) {
            pendingSoftwareClaim = try? decoder.decode(PendingSoftwareClaim.self, from: data)
        }
        if let data = storage.data(forKey: Self.claimedSoftwareKey) {
            claimedSoftware = (try? decoder.decode([ClaimedSoftwareEncounter].self, from: data)) ?? []
        }
        followedBuilderIDs = Set(storage.stringArray(forKey: Self.followedBuildersKey) ?? [])
    }

    private func persist<Value: Encodable>(_ value: Value, key: String) throws {
        storage.set(try JSONEncoder().encode(value), forKey: key)
    }

    private func clearPendingClaim() {
        pendingSoftwareClaim = nil
        storage.removeObject(forKey: Self.pendingClaimKey)
    }

    public func refreshBuilderProfile() async {
        do {
            let id = try await builderIdentity.builderID()
            builderID = id
            selectDefaultAliasAppIfNeeded()
            if let profile = try await network.builderProfile(builderID: id) {
                profileDisplayName = profile.displayName
                profileHandle = profile.handle ?? ""
                profileNonce = profile.nonce
            }
        } catch {
            // A BuilderAccount and public profile need not exist before the
            // first explicitly approved publication.
        }
    }

    public func savePublicProfile() async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do {
            let id = try await builderIdentity.builderID()
            let profile = try BuilderProfile(
                builderID: id,
                displayName: profileDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
                handle: profileHandle.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().nilIfEmpty,
                updatedAt: Self.networkTimestamp(),
                nonce: profileNonce + 1
            )
            let envelope = try await builderIdentity.sign(profile: profile)
            try await network.updateProfile(envelope)
            builderID = id
            profileNonce = profile.nonce
            profileNotice = "Public profile updated with this iPhone’s Builder DeviceKey."
        } catch {
            profileNotice = "Profile update failed. Publish once first, then check the name and handle."
        }
    }

    public func requestGlobalAlias() async {
        guard !busy else { return }
        busy = true
        defer { busy = false }
        do {
            let id = try await builderIdentity.builderID()
            guard let app = publicApps.first(where: {
                $0.release.builderID == id && $0.release.shotID == requestedAliasShotID
            }) else {
                selectDefaultAliasAppIfNeeded()
                profileNotice = "Choose the exact published app for this global alias."
                return
            }
            let alias = requestedAlias.trimmingCharacters(in: .whitespacesAndNewlines)
                .lowercased().nilIfEmpty ?? app.release.display.appSlug
            guard let alias else {
                profileNotice = "Enter an alias for this published app."
                return
            }
            let now = UInt64(Date().timeIntervalSince1970.rounded(.down))
            var random = [UInt8](repeating: 0, count: 32)
            var generator = SystemRandomNumberGenerator()
            for index in random.indices { random[index] = UInt8.random(in: .min ... .max, using: &generator) }
            let claim = try AliasClaim(
                builderID: id,
                shotID: app.release.shotID,
                alias: alias,
                requestID: "0x\(random.map { String(format: "%02x", $0) }.joined())",
                nonce: aliasNonce + 1,
                deadline: now + 900,
                requestedAt: Self.networkTimestamp()
            )
            let receipt = try await network.requestAlias(builderIdentity.sign(claim: claim))
            aliasNonce = claim.nonce
            requestedAlias = ""
            lastAliasRequestID = receipt.requestID
            profileNotice = "Alias /\(receipt.alias) is signed and queued for explicit policy review."
        } catch {
            profileNotice = "Alias request failed. Check the alias and try again."
        }
    }

    private static func networkTimestamp() -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: Date())
    }

    private func selectDefaultAliasAppIfNeeded() {
        let eligible = aliasEligibleApps
        if !eligible.contains(where: { $0.release.shotID == requestedAliasShotID }) {
            requestedAliasShotID = eligible.first?.release.shotID ?? ""
        }
    }

    /// Accepts only the existing signed, expiring, single-use pairing
    /// invitation delivered by CoreDevice's verified `--payload-url` launch.
    /// Recovery words are created and shown only on this iPhone; they are
    /// never returned to the Mac or placed in the URL.
    public func bootstrapFromCable(_ url: URL) async {
        guard url.scheme?.lowercased() == "tohseno",
              url.host?.lowercased() == "pair",
              url.path.hasPrefix("/v1/")
        else {
            notice = "Reconnect this iPhone to your Mac and open Tohseno there."
            return
        }
        pendingCableInvitation = url.absoluteString
        await createIdentity()
        if recoveryWords == nil {
            await pair(scanned: url.absoluteString)
            pendingCableInvitation = nil
        }
    }

    public func handleIncomingURL(_ url: URL) async {
        if url.host?.lowercased() == "pair" {
            await bootstrapFromCable(url)
            return
        }
        if url.scheme?.lowercased() == "tohseno", url.host?.lowercased() == "follow",
           let address = url.path.split(separator: "/").first.map(String.init),
           address.range(of: #"^0x[0-9a-f]{40}$"#, options: .regularExpression) != nil {
            let builderID = "eip155:4663:\(address)"
            if !followedBuilderIDs.contains(builderID) { toggleFollow(builderID: builderID) }
            return
        }
        guard url.scheme?.lowercased() == "tohseno",
              let host = url.host?.lowercased(),
              host == "claim" || host == "install" || host == "fork",
              let shot = url.path.split(separator: "/").first.map(String.init),
              shot.count == 64,
              let release = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                .queryItems?.first(where: { $0.name == "release" })?.value
        else {
            networkNotice = "This Tohseno app link is incomplete or invalid."
            return
        }
        do {
            linkedPublicRelease = try await network.release(
                shotID: "0x\(shot)",
                releaseDigest: release
            )
        } catch {
            networkNotice = "That exact public release is not currently discoverable."
            return
        }
    }

    public func requestNetworkRelease(
        _ app: PublicAppRelease,
        action: NetworkReleaseAction
    ) async {
        guard !busy else { return }
        if action == .fork && !app.release.permissions.forkAllowed {
            networkNotice = "The Builder did not authorize forks of this release."
            return
        }
        busy = true
        defer { busy = false }
        do {
            _ = try await backend.requestNetworkRelease(
                action: action,
                shotID: app.release.shotID,
                releaseDigest: app.releaseDigest,
                commandID: "network_\(action.rawValue)_\(UUID().uuidString.lowercased())"
            )
            linkedPublicRelease = nil
            networkNotice = connection == .connected
                ? (action == .install
                    ? "Queued. Your Mac is verifying and preparing this exact release."
                    : "Queued. Your Mac is verifying and materializing a new fork.")
                : "Queued for your Mac. It will resume when the private connection returns."
            unacknowledged = (try? await backend.unacknowledgedCommandCount()) ?? unacknowledged
        } catch {
            networkNotice = "The request is still on this iPhone and will retry when your Mac is reachable."
        }
    }

    public func confirmRecoveryWords() async {
        guard let invitation = pendingCableInvitation else { return }
        pendingCableInvitation = nil
        await pair(scanned: invitation)
    }

    /// Pull the Mac's current truth. Safe to call on every launch and
    /// foreground; the SDK reconciles and replays anything that was missed.
    public func refresh() async {
        await refresh(reportFailure: false)
    }

    /// A person-requested refresh. Unlike background reconciliation, failure
    /// is visible so the Sync button never pretends stale data is current.
    public func syncNow() async {
        await refresh(reportFailure: true)
    }

    private func refresh(reportFailure: Bool) async {
        guard !syncing else { return }
        syncing = true
        defer { syncing = false }
        do {
            try await backend.reconcile()
            // A process relaunch restores the persisted pairing before this
            // model exists. Rejoin the live encrypted event stream after the
            // first successful reconciliation so reopening the Companion is
            // the same connected state as completing a fresh pairing.
            try await backend.startSynchronization()
            if reportFailure { notice = nil }
        } catch TohsenoCompanionError.notPaired {
            screen = .firstRun
            return
        } catch {
            // Offline is not an error the person needs to read. The durable
            // outbox keeps the request; `unacknowledged` says the honest thing.
            if reportFailure { notice = "Couldn’t sync with your Mac." }
        }
        await load()
    }

    private func load() async {
        do {
            let workspace = try await backend.synchronizedWorkspace()
            adopt(workspace.shots)
            switch screen {
            case .loading, .firstRun:
                screen = .apps
            case .entitlementDecision, .trialEnded, .apps, .create, .app:
                break
            }
            try await loadIcons()
        } catch TohsenoCompanionError.notPaired {
            screen = .firstRun
        } catch {
            // A missing snapshot means nothing has synchronized yet.
            if case .loading = screen { screen = .apps }
        }
        unacknowledged = (try? await backend.unacknowledgedCommandCount()) ?? unacknowledged
    }

    private func loadIcons() async throws {
        for shot in apps {
            guard let descriptor = shot.icon, icons[descriptor.blobID] == nil else { continue }
            if let bytes = try await backend.iconBytes(for: descriptor) {
                icons[descriptor.blobID] = bytes
            }
        }
    }

    private func adopt(_ shots: [ShotSummary]) {
        apps = shots
            .filter {
                ($0.kind == .factoryShot || $0.kind == .adoptedProject)
                    && !$0.retired && !$0.archived
            }
            .sorted { left, right in
                left.sortIndex == right.sortIndex
                    ? left.displayName < right.displayName
                    : left.sortIndex < right.sortIndex
            }
        for shot in apps where shot.execution != nil {
            justSent.remove(shot.shotID)
        }
    }

    func apply(_ event: WorkspaceEvent) {
        switch event.payload {
        case let .workspaceSnapshot(snapshot):
            adopt(snapshot.shots)
        case let .productEntitlement(projection):
            entitlement = projection
            if screen == .entitlementDecision || screen == .trialEnded { screen = .apps }
        case let .builderFollows(projection):
            followedBuilderIDs = Set(projection.builderIDs)
            storage.set(
                projection.builderIDs,
                forKey: Self.followedBuildersKey
            )
        case let .privateUpdates(projection):
            privateUpdates = projection.items
        case let .iconBlob(blob):
            icons[blob.blobID] = blob.bytes
        case let .commandRejected(receipt):
            notice = Self.humanRejection(receipt.rejectionCode)
            if let shotID = receipt.shotID { justSent.remove(shotID) }
        case .commandAcknowledged:
            Task { self.unacknowledged = (try? await self.backend.unacknowledgedCommandCount()) ?? 0 }
        case let .publicationApprovalRequested(request):
            do {
                try request.validate()
                guard builderDevice.map({ BuilderDeviceAnnouncement(publicIdentity: $0) }) == request.builderDevice else {
                    notice = "This publication targets a different Builder identity."
                    return
                }
                pendingPublication = request
                Task {
                    await self.recordPrivateUpdate(PrivateUpdateItem(
                        kind: .publicationApproval,
                        subjectID: request.jobID,
                        evidenceID: event.eventID,
                        title: "Publication needs your approval",
                        detail: "Review the signed release and Claim Edition on this iPhone.",
                        occurredAt: event.emittedAt
                    ))
                }
            } catch {
                notice = "Your Mac sent a publication request that could not be verified."
            }
        case let .executionCompleted(execution):
            Task {
                await self.recordPrivateUpdate(PrivateUpdateItem(
                    kind: .evolutionFinished,
                    subjectID: execution.shotID,
                    evidenceID: execution.executionID,
                    title: "Evolution finished",
                    detail: "Your app is ready on your Mac.",
                    occurredAt: event.emittedAt
                ))
                await self.load()
            }
        default:
            Task { await self.load() }
        }
    }

    public func setPrivateUpdateRead(_ update: PrivateUpdateItem, read: Bool = true) {
        guard let index = privateUpdates.firstIndex(where: { $0.updateID == update.updateID }),
              (privateUpdates[index].readAt != nil) != read else { return }
        let previous = privateUpdates[index]
        privateUpdates[index] = Self.copy(previous, readAt: read ? Self.canonicalNow() : nil)
        Task {
            do {
                _ = try await backend.setPrivateUpdateRead(
                    updateID: update.updateID,
                    read: read,
                    commandID: "update_read_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
                )
            } catch {
                if let current = privateUpdates.firstIndex(where: { $0.updateID == update.updateID }) {
                    privateUpdates[current] = previous
                }
                notice = "That Update could not be synchronized with your Mac yet."
            }
        }
    }

    private func deriveClaimedAppUpdates() async {
        let claimedShotIDs = Set(claimedSoftware.map(\.app.release.shotID))
        for event in publicTimeline where event.kind == "shot.updated"
            && claimedShotIDs.contains(event.shotID) {
            let name = publicApps.first(where: { $0.release.shotID == event.shotID })?
                .release.display.name ?? "A claimed app"
            await recordPrivateUpdate(PrivateUpdateItem(
                kind: .claimedAppUpdated,
                subjectID: event.shotID,
                evidenceID: event.eventID,
                title: "\(name) was updated",
                detail: "The Builder shipped a new verified release. Your original Claim remains unchanged.",
                occurredAt: event.occurredAt
            ))
        }
    }

    private func recordPrivateUpdate(_ update: PrivateUpdateItem) async {
        guard !privateUpdates.contains(where: { $0.updateID == update.updateID }) else { return }
        privateUpdates.append(update)
        privateUpdates.sort {
            $0.occurredAt == $1.occurredAt
                ? $0.updateID < $1.updateID
                : $0.occurredAt > $1.occurredAt
        }
        privateUpdates = Array(privateUpdates.prefix(1_000))
        do {
            _ = try await backend.upsertPrivateUpdate(
                update,
                commandID: "private_\(update.updateID)"
            )
        } catch {
            privateUpdates.removeAll { $0.updateID == update.updateID }
        }
    }

    private static func copy(_ update: PrivateUpdateItem, readAt: String?) -> PrivateUpdateItem {
        PrivateUpdateItem(
            updateID: update.updateID,
            kind: update.kind,
            subjectID: update.subjectID,
            evidenceID: update.evidenceID,
            title: update.title,
            detail: update.detail,
            occurredAt: update.occurredAt,
            readAt: readAt
        )
    }

    private static func canonicalNow() -> String {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"
        return formatter.string(from: Date())
    }

    public func dismissPublication() {
        pendingPublication = nil
    }

    public func approvePublication(policy selectedPolicy: ClaimEditionPolicy? = nil) async {
        guard let request = pendingPublication else { return }
        busy = true
        defer { busy = false }
        do {
            try request.validate()
            let catalog = BuilderDeviceSignature(
                try await builderIdentity.sign(digestHex: request.catalogDigest)
            )
            let registry = BuilderDeviceSignature(
                try await builderIdentity.sign(digestHex: request.registryDigest)
            )
            let claimEdition: ApprovedClaimEdition?
            if let context = request.claimEdition {
                let policy = if let required = context.requestedPolicy {
                    try required.policy()
                } else if let selectedPolicy {
                    selectedPolicy
                } else {
                    try ClaimEditionPolicy()
                }
                let action = try context.action(request: request, policy: policy)
                let digest = try action.digest(
                    claimsContract: context.claimsContract,
                    expectedRegistry: request.shotRegistry
                )
                let digestHex = "0x" + digest.map { String(format: "%02x", $0) }.joined()
                let signature = BuilderDeviceSignature(
                    try await builderIdentity.sign(digestHex: digestHex)
                )
                claimEdition = try ApprovedClaimEdition(
                    policy: policy,
                    action: action,
                    digest: digestHex,
                    signature: signature
                )
            } else {
                guard selectedPolicy == nil else {
                    throw TohsenoCompanionError.invalidEncoding("an Update cannot change its Claim Edition")
                }
                claimEdition = nil
            }
            let approvedAt = Self.timestamp(Date())
            _ = try await backend.approvePublication(
                jobID: request.jobID,
                catalog: catalog,
                registry: registry,
                claimEdition: claimEdition,
                approvedAt: approvedAt,
                commandID: "publication_approve_\(request.jobID)"
            )
            pendingPublication = nil
            notice = "Approved. Your Mac is publishing \(request.appName)."
        } catch {
            notice = "This publication could not be approved safely."
        }
    }

    private static func timestamp(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: date)
    }

    /// The Mac's protocol reasons, said the way a person would say them.
    static func humanRejection(_ code: String?) -> String {
        switch code {
        case "stale_base_version", "stale_project_source_state":
            "This app changed while your request was waiting. Review it and try again."
        case "project_busy":
            "This app is already being changed. Wait for it to finish, then send the next request."
        case "device_revoked", "device_not_paired", "capability_rejected":
            "This iPhone no longer has access to your Mac."
        case "unknown_shot":
            "That app is no longer on your Mac."
        default:
            "Your Mac couldn’t accept that request."
        }
    }

    // MARK: - The one action

    public func open(_ shot: ShotSummary) {
        intent = ""
        attachments = []
        notice = nil
        screen = .app(shot.shotID)
    }

    public func openCreate() {
        appName = ""
        intent = ""
        attachments = []
        notice = nil
        screen = .create
    }

    public func openApps() {
        appName = ""
        intent = ""
        attachments = []
        screen = .apps
    }

    public var canCreate: Bool {
        let name = appName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let usableName = name.isEmpty
            || name.range(of: "^[a-z0-9][a-z0-9-]{0,62}$", options: .regularExpression) != nil
        return usableName
            && !intent.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !busy
    }

    public func create() async {
        guard case .create = screen, canCreate else { return }
        busy = true
        defer { busy = false }
        let name = appName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        do {
            _ = try await backend.requestShotCreation(CreateShotRequest(
                suggestedName: name.isEmpty ? nil : name,
                intention: intent,
                references: attachments
            ))
            appName = ""
            intent = ""
            attachments = []
            notice = nil
            unacknowledged = (try? await backend.unacknowledgedCommandCount()) ?? unacknowledged
            screen = .apps
        } catch let error as TohsenoCompanionError {
            notice = Self.humanFailure(error)
        } catch {
            notice = "Your Mac couldn’t accept that request."
        }
    }

    public func app(_ shotID: String) -> ShotSummary? {
        apps.first { $0.shotID == shotID }
    }

    public func icon(for shot: ShotSummary) -> Data? {
        guard let blobID = shot.icon?.blobID else { return nil }
        return icons[blobID]
    }

    public var canEvolve: Bool {
        guard case let .app(shotID) = screen, let shot = app(shotID) else { return false }
        guard !presentation(for: shot).state.inFlight else { return false }
        return !intent.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && (shot.kind == .adoptedProject
                ? shot.sourceState != nil
                : shot.expressionID != nil
                    && shot.latestVersionID != nil
                    && shot.latestVersionOrdinal != nil)
            && !busy
    }

    /// Evolve App.
    ///
    /// One tap. No confirmation, no version picker, no separate feedback step.
    /// The SDK persists the signed command and its images before this returns,
    /// so the person can close TOHSENO immediately — a Mac that is offline or
    /// busy receives it later without another tap.
    public func evolve() async {
        guard case let .app(shotID) = screen, let shot = app(shotID), canEvolve else { return }
        busy = true
        defer { busy = false }
        do {
            if shot.kind == .adoptedProject, let sourceState = shot.sourceState {
                _ = try await backend.requestProjectEvolution(ProjectEvolutionRequest(
                    projectID: shot.shotID,
                    baseSourceState: sourceState,
                    intention: intent,
                    references: attachments
                ))
            } else if let expression = shot.expressionID,
                      let version = shot.latestVersionID,
                      let ordinal = shot.latestVersionOrdinal {
                _ = try await backend.requestEvolution(EvolutionRequest(
                    // The exact accepted base is bound here, at submission. A
                    // moved base is refused by the Mac, never silently rebased.
                    shotID: shot.shotID,
                    baseExpressionID: expression,
                    baseVersionID: version,
                    baseVersionOrdinal: ordinal,
                    intention: intent,
                    references: attachments
                ))
            } else {
                return
            }
            intent = ""
            attachments = []
            notice = nil
            justSent.insert(shot.shotID)
            unacknowledged = (try? await backend.unacknowledgedCommandCount()) ?? unacknowledged
        } catch let error as TohsenoCompanionError {
            notice = Self.humanFailure(error)
        } catch {
            notice = "Your Mac couldn’t accept that request."
        }
    }

    static func humanFailure(_ error: TohsenoCompanionError) -> String {
        return switch error {
        case .capabilityRevoked, .capabilityDenied, .notPaired:
            "This iPhone no longer has access to your Mac."
        case let .commandRejected(reason) where reason.contains("outbox is full"):
            "Too many requests are already waiting for your Mac."
        default:
            "Your Mac couldn’t accept that request."
        }
    }

    // MARK: - What the person sees

    /// The one state line for an app, phone-side.
    public func presentation(for shot: ShotSummary) -> TohsenoPresentation {
        // Nothing this phone wrote has reached the Mac, so nothing about the
        // build can be claimed yet.
        if justSent.contains(shot.shotID) {
            return unacknowledged > 0
                ? .waitingForMac(appName: shot.displayName)
                : TohsenoPresentation.forState(.building, appName: shot.displayName)
        }
        return TohsenoPresentation.of(shot)
    }

    // MARK: - First run

    public func createIdentity() async {
        do {
            recoveryWords = try await backend.createIdentity().reveal()
        } catch TohsenoCompanionError.identityAlreadyExists {
            recoveryWords = nil
        } catch {
            notice = "Tohseno could not create this iPhone's identity."
        }
    }

    public func pair(scanned invitation: String) async {
        busy = true
        defer { busy = false }
        do {
            try await backend.pair(invitation: invitation, displayName: deviceName)
            try? await backend.startSynchronization()
            recoveryWords = nil
            notice = nil
            await refresh()
            await workshopRuntime.start()
            screen = .apps
        } catch {
#if DEBUG
            // Pairing material is deliberately omitted. A physical-device
            // development build still needs one factual boundary error so a
            // QR parse failure can be distinguished from relay transport.
            NSLog("TOHSENO Companion pairing failed: %@", String(describing: error))
#endif
            screen = .firstRun
            notice = Self.humanPairingFailure(error)
        }
    }

    static func humanPairingFailure(_ error: Error) -> String {
        guard let error = error as? TohsenoCompanionError else {
            return "The private connection stopped before it completed. Reconnect this iPhone to your Mac."
        }
        return switch error {
        case .transportUnavailable:
#if DEBUG
            "Tohseno couldn’t reach the development relay on your Mac. The USB cable installs and debugs the app, but it doesn’t carry Companion messages. Connect both devices to the same Wi‑Fi, then try again."
#else
            "Tohseno couldn’t reach your Mac. Check this iPhone’s internet connection, then try again."
#endif
        case .invitationExpired:
            "The private connection expired. Reconnect this iPhone to your Mac."
        case .invitationNotYetValid:
            "Your iPhone and Mac clocks don’t agree. Set Date & Time to automatic on both devices, then try again."
        case .invalidInvitation, .invalidEncoding, .relayNotAllowed:
            "The private connection was invalid. Reconnect this iPhone to your Mac."
        case let .relayFailure(status) where [404, 409, 410].contains(status):
            "The private connection expired or was already used. Reconnect this iPhone to your Mac."
        case .relayFailure:
            "Your Mac refused the private connection. Reconnect this iPhone and try again."
        default:
            "The private connection stopped before it completed. Reconnect this iPhone to your Mac."
        }
    }
}

private extension Data {
    var prefixedHex: String { "0x" + map { String(format: "%02x", $0) }.joined() }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}
