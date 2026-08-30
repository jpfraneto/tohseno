import Foundation
import TohsenoCompanionKit

/// Everything the TOHSENO Companion knows.
///
/// The phone is a remote control for intent. It holds no factory state of its
/// own beyond an unsent draft: the Mac is authoritative for apps, versions, and
/// execution, and the SDK owns the durable outbox that survives the app being
/// closed.
@MainActor
@Observable
public final class CompanionModel {
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
    public private(set) var builderDevice: BuilderDevicePublicIdentity?
    public private(set) var networkNotice: String?
    public private(set) var pendingPublication: PublicationApprovalRequest?
    public var linkedPublicRelease: PublicAppRelease?
    public var profileDisplayName = ""
    public var profileHandle = ""
    public var requestedAlias = ""
    public private(set) var builderID: String?
    public private(set) var profileNotice: String?

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
    private var profileNonce: UInt64 = 0
    private var aliasNonce: UInt64 = 0

    public init(
        backend: any CompanionBackend,
        deviceName: String,
        network: PublicNetworkClient = .production,
        builderIdentity: BuilderDeviceIdentity = BuilderDeviceIdentity()
    ) {
        self.backend = backend
        self.deviceName = deviceName
        self.network = network
        self.builderIdentity = builderIdentity
    }

    public func start() {
        Task { [backend] in
            for await connection in backend.connectionStates {
                self.connection = connection
                if connection == .revoked {
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
        Task { await refresh() }
        Task { await refreshPublicNetwork() }
        Task {
            do {
                let identity = try await builderIdentity.ensureCreated()
                builderDevice = identity
                await refreshBuilderProfile()
                let announcement = BuilderDeviceAnnouncement(publicIdentity: identity)
                _ = try await backend.announceBuilderDevice(
                    announcement,
                    commandID: "builder_announce_\(identity.keyID.suffix(24))"
                )
            }
            catch { builderDevice = nil }
        }
    }

    public func refreshPublicNetwork() async {
        do {
            publicApps = try await network.releases()
            networkNotice = nil
        } catch {
            networkNotice = "The public Registry is temporarily unavailable. Your private Mac connection is unaffected."
        }
    }

    public func refreshBuilderProfile() async {
        do {
            let id = try await builderIdentity.builderID()
            builderID = id
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
            guard let app = publicApps.first(where: { $0.release.builderID == id }) else {
                profileNotice = "Publish an installable app before requesting a global alias."
                return
            }
            let now = UInt64(Date().timeIntervalSince1970.rounded(.down))
            var random = [UInt8](repeating: 0, count: 32)
            var generator = SystemRandomNumberGenerator()
            for index in random.indices { random[index] = UInt8.random(in: .min ... .max, using: &generator) }
            let claim = try AliasClaim(
                builderID: id,
                shotID: app.release.shotID,
                alias: requestedAlias.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
                requestID: "0x\(random.map { String(format: "%02x", $0) }.joined())",
                nonce: aliasNonce + 1,
                deadline: now + 900,
                requestedAt: Self.networkTimestamp()
            )
            try await network.requestAlias(builderIdentity.sign(claim: claim))
            aliasNonce = claim.nonce
            requestedAlias = ""
            profileNotice = "Alias request signed and queued for explicit policy review."
        } catch {
            profileNotice = "Alias request failed. Check the alias and try again."
        }
    }

    private static func networkTimestamp() -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: Date())
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
        guard url.scheme?.lowercased() == "tohseno",
              let host = url.host?.lowercased(),
              host == "install" || host == "fork",
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
            } catch {
                notice = "Your Mac sent a publication request that could not be verified."
            }
        default:
            Task { await self.load() }
        }
    }

    public func dismissPublication() {
        pendingPublication = nil
    }

    public func approvePublication() async {
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
            let approvedAt = Self.timestamp(Date())
            _ = try await backend.approvePublication(
                jobID: request.jobID,
                catalog: catalog,
                registry: registry,
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
        let usableName = name.range(of: "^[a-z0-9][a-z0-9-]{0,62}$", options: .regularExpression) != nil
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
                suggestedName: name,
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

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}
