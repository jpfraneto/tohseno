import Foundation
import Observation
#if canImport(AppKit)
import AppKit
#endif

public enum ReferenceTarget: Sendable {
    case creation
    case evolution(String)
}

public enum AppRoute: Hashable, Sendable {
    case library
    case create
    case app(String)
}

@MainActor
@Observable
public final class TohsenoAppModel {
    public private(set) var workspace: WorkspaceSnapshot?
    public private(set) var defaults: FactoryDefaults?
    public private(set) var readiness: ReadinessView?
    public private(set) var receipt: ExecutionReceipt?
    public private(set) var managedStatus: ManagedStatus?
    public private(set) var managedBalance: ManagedBalance?
    public private(set) var managedCatalog: ManagedCatalog?
    public private(set) var creationEstimate: ManagedEstimate?
    public private(set) var evolutionEstimates: [String: ManagedEstimate] = [:]
    public private(set) var icons: [String: Data] = [:]
    public private(set) var previews: [String: Data] = [:]
    public private(set) var managedMessage: String?
    public private(set) var isLoading = true
    public private(set) var isSubmitting = false
    public private(set) var errorMessage: String?
    public var route: AppRoute = .library {
        didSet { persistRoute() }
    }
    public var creation = CreationDraft()
    public var evolutions: [String: EvolutionDraft] = [:]
    public var customHarness = CustomHarnessDraft()
    public var localEndpoint = LocalEndpointDraft()
    public var advancedExpanded = false

    private let client: any FactoryServing
    private let preferences: UserDefaults
    private var monitoringTask: Task<Void, Never>?

    public init(client: any FactoryServing, preferences: UserDefaults = .standard) {
        self.client = client
        self.preferences = preferences
        restoreRoute()
    }

    public var apps: [AppSummary] { workspace?.visibleApps ?? [] }
    public var archivedApps: [AppSummary] { workspace?.archivedApps ?? [] }

    public var selectedApp: AppSummary? {
        guard case let .app(id) = route else { return nil }
        return apps.first { $0.id == id }
    }

    public func start() {
        guard monitoringTask == nil else { return }
        monitoringTask = Task { [weak self] in
            guard let self else { return }
            await self.reload()
            while !Task.isCancelled {
                let stream = await self.client.events()
                do {
                    for try await _ in stream {
                        guard !Task.isCancelled else { return }
                        await self.reloadWorkspace()
                    }
                } catch {
                    if Task.isCancelled { return }
                    try? await Task.sleep(for: .seconds(1))
                    await self.reloadWorkspace()
                }
            }
        }
    }

    public func reload() async {
        isLoading = true
        defer { isLoading = false }
        do {
            async let readiness = client.readiness()
            async let workspace = client.workspace()
            async let defaults = client.factoryDefaults()
            self.readiness = try await readiness
            self.workspace = try await workspace
            self.defaults = try await defaults
            await refreshAssets()
            await refreshManaged()
            repairRoute()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func reloadWorkspace() async {
        do {
            workspace = try await client.workspace()
            repairRoute()
            await refreshAssets()
            if let selectedApp {
                receipt = try await client.receipt(for: selectedApp.id)
            }
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func refreshAssets() async {
        guard let workspace else { return }
        for app in workspace.visibleApps {
            if icons[app.id] == nil, let data = try? await client.icon(for: app.id) {
                icons[app.id] = data
            }
            if previews[app.id] == nil, let data = try? await client.preview(for: app.id) {
                previews[app.id] = data
            }
        }
    }

    public func openOnPhone(for app: AppSummary) async {
        do { try await client.openOnPhone(for: app.id) }
        catch { errorMessage = error.localizedDescription }
    }

    public func refreshManaged() async {
        do {
            managedStatus = try await client.managedStatus()
            async let balance = client.managedBalance()
            async let catalog = client.managedCatalog()
            managedBalance = try await balance
            managedCatalog = try await catalog
            managedMessage = nil
        } catch {
            managedBalance = nil
            managedCatalog = nil
            managedMessage = error.localizedDescription
        }
    }

    public func estimateCreation() async {
        guard creation.harness == "tohseno-managed", let selected = creation.model else {
            creationEstimate = nil
            return
        }
        do {
            let value = try await client.managedEstimate(
                model: selected,
                privacy: creation.managedPrivacy,
                intentionBytes: UInt64(creation.intention.utf8.count),
                referenceBytes: UInt64(creation.references.reduce(0) { $0 + $1.data.count }),
                appID: nil
            )
            guard creation.harness == "tohseno-managed",
                  creation.model == value.model,
                  creation.managedPrivacy == value.privacy else { return }
            creationEstimate = value
            creation.managedMaximumMicrousd = value.recommendedMaximumMicrousd
            creation.managedConsent = false
        } catch {
            creationEstimate = nil
            managedMessage = error.localizedDescription
        }
    }

    public func chooseManagedForCreation() async {
        guard let model = managedCatalog?.models.first else {
            errorMessage = managedMessage ?? "Managed intelligence is not available right now."
            return
        }
        creation.harness = "tohseno-managed"
        creation.model = model.model
        creation.managedPrivacy = model.privacyTiers.contains("zdr") ? "zdr" : (model.privacyTiers.first ?? "standard")
        creation.managedConsent = false
        await estimateCreation()
    }

    public func estimateEvolution(for app: AppSummary) async {
        guard let draft = evolutions[app.id], draft.harness == "tohseno-managed",
              let selected = draft.model else {
            evolutionEstimates[app.id] = nil
            return
        }
        do {
            let value = try await client.managedEstimate(
                model: selected,
                privacy: draft.managedPrivacy,
                intentionBytes: UInt64(draft.intention.utf8.count),
                referenceBytes: UInt64(draft.references.reduce(0) { $0 + $1.data.count }),
                appID: app.id
            )
            guard let current = evolutions[app.id], current.harness == "tohseno-managed",
                  current.model == value.model, current.managedPrivacy == value.privacy else { return }
            evolutionEstimates[app.id] = value
            var updated = current
            updated.managedMaximumMicrousd = value.recommendedMaximumMicrousd
            updated.managedConsent = false
            evolutions[app.id] = updated
        } catch {
            evolutionEstimates[app.id] = nil
            managedMessage = error.localizedDescription
        }
    }

    public func buyBalance(packID: String) async {
        guard !isSubmitting else { return }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            let checkout = try await client.managedCheckout(packID: packID)
            #if canImport(AppKit)
            guard let url = URL(string: checkout.checkoutURL), url.scheme == "https",
                  url.host == "checkout.stripe.com", NSWorkspace.shared.open(url) else {
                throw FactoryClientError.invalidResponse("Stripe Checkout could not be opened safely.")
            }
            #endif
        } catch { errorMessage = error.localizedDescription }
    }

    public func requestWelcomeCompute() {
        #if canImport(AppKit)
        guard let value = managedStatus?.welcomeContactURL, let url = URL(string: value),
              ["https", "mailto"].contains(url.scheme ?? ""), NSWorkspace.shared.open(url) else {
            errorMessage = "This release has not configured a welcome-compute contact yet."
            return
        }
        #endif
    }

    public func managedEstimateDescription(_ estimate: ManagedEstimate?) -> String {
        guard let estimate else { return "Waiting for a current server-priced estimate." }
        return "Estimated \(currency(estimate.lowMicrousd))–\(currency(estimate.highMicrousd)); maximum \(currency(estimate.recommendedMaximumMicrousd))."
    }

    public func currency(_ microusd: UInt64) -> String {
        (Double(microusd) / 1_000_000).formatted(.currency(code: "USD"))
    }

    public func signedCurrency(_ microusd: Int64) -> String {
        (Double(microusd) / 1_000_000).formatted(.currency(code: "USD"))
    }

    public func submitCreation() async {
        guard !isSubmitting else { return }
        let intention = creation.intention.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !intention.isEmpty else {
            errorMessage = "Describe what would make your life easier."
            return
        }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            let receipt = try await client.create(creation, commandID: Self.commandID())
            creation = CreationDraft()
            route = .app(receipt.shotID)
            await reloadWorkspace()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func submitEvolution(for app: AppSummary) async {
        guard !isSubmitting else { return }
        let draft = evolutions[app.id] ?? EvolutionDraft()
        guard !draft.intention.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            errorMessage = "Describe what should change."
            return
        }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            _ = try await client.evolve(app, draft: draft, commandID: Self.commandID())
            evolutions[app.id] = EvolutionDraft()
            await reloadWorkspace()
        } catch let error as FactoryClientError {
            if case .rejected(code: "stale_base", message: _) = error {
                errorMessage = "This app changed while your request was waiting. Review your request and try again."
            } else {
                errorMessage = error.localizedDescription
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func performReadinessAction() async {
        guard let action = readiness?.primaryAction, !isSubmitting else { return }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            readiness = try await client.performReadinessAction(action)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func showReceipt(for app: AppSummary) async {
        do { receipt = try await client.receipt(for: app.id) }
        catch { errorMessage = error.localizedDescription }
    }

    /// Seeds a new evolution from the app's last durable execution route.
    /// The owner can restore Automatic under Advanced, and managed work still
    /// requires a fresh estimate, maximum, and consent for every request.
    public func prepareEvolution(for app: AppSummary) async {
        guard evolutions[app.id] == nil else { return }
        do {
            let latest = try await client.receipt(for: app.id)
            receipt = latest
            guard let latest else {
                evolutions[app.id] = EvolutionDraft()
                return
            }
            var draft = EvolutionDraft(harness: latest.harnessID, model: latest.model)
            if latest.harnessID == "tohseno-managed" {
                draft.managedPrivacy = latest.route.replacingOccurrences(of: "managed-", with: "")
                draft.managedMaximumMicrousd = nil
                draft.managedConsent = false
            }
            evolutions[app.id] = draft
        } catch {
            evolutions[app.id] = EvolutionDraft()
            errorMessage = error.localizedDescription
        }
    }

    public func openSource(for app: AppSummary) async {
        do { try await client.openSource(for: app.id) }
        catch { errorMessage = error.localizedDescription }
    }

    public func retire(_ app: AppSummary) async {
        guard !isSubmitting else { return }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            try await client.retire(appID: app.id)
            route = .library
            await reloadWorkspace()
        } catch { errorMessage = error.localizedDescription }
    }

    public func restore(_ app: AppSummary) async {
        guard !isSubmitting else { return }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            try await client.restore(appID: app.id)
            await reloadWorkspace()
            route = .app(app.id)
        } catch { errorMessage = error.localizedDescription }
    }

    public func dismissError() { errorMessage = nil }

    public func report(_ error: Error) { errorMessage = error.localizedDescription }

    public func addReferences(
        _ result: Result<[URL], Error>,
        to target: ReferenceTarget
    ) {
        do {
            let urls = try result.get()
            var destination: [ReferenceDraft]
            switch target {
            case .creation: destination = creation.references
            case let .evolution(id): destination = evolutions[id]?.references ?? []
            }
            guard destination.count + urls.count <= 8 else {
                throw FactoryClientError.invalidConfiguration("A request accepts at most eight reference images.")
            }
            for url in urls {
                let accessed = url.startAccessingSecurityScopedResource()
                defer { if accessed { url.stopAccessingSecurityScopedResource() } }
                let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
                guard values.isRegularFile == true, values.isSymbolicLink != true,
                      let size = values.fileSize, size <= 64 * 1024 * 1024 else {
                    throw FactoryClientError.invalidConfiguration("Each reference must be a regular image no larger than 64 MB.")
                }
                let ext = url.pathExtension.lowercased()
                let mediaType: String
                switch ext {
                case "png": mediaType = "image/png"
                case "jpg", "jpeg": mediaType = "image/jpeg"
                default: throw FactoryClientError.invalidConfiguration("References must be PNG or JPEG images.")
                }
                let data = try Data(contentsOf: url, options: [.mappedIfSafe, .uncached])
                guard data.count == size else {
                    throw FactoryClientError.invalidConfiguration("A reference changed while it was being attached.")
                }
                destination.append(ReferenceDraft(
                    filename: uniqueFilename(url.lastPathComponent, among: destination),
                    mediaType: mediaType,
                    data: data,
                    origin: "native_file_picker"
                ))
            }
            switch target {
            case .creation: creation.references = destination
            case let .evolution(id):
                var draft = evolutions[id] ?? EvolutionDraft()
                draft.references = destination
                evolutions[id] = draft
            }
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func costDescription(for harnessID: String?) -> String {
        let effectiveHarnessID = harnessID ?? defaults?.harnessID
        let selected = effectiveHarnessID.flatMap { id in defaults?.harnesses.first { $0.id == id } }
        let route = selected?.routes.first(where: \.available)
        switch route?.billing {
        case "none", "local": return "$0 paid to TOHSENO; your Mac still uses electricity and hardware."
        case "subscription": return "Incremental provider cost is unknown or covered by your provider plan."
        case "api": return "Provider usage may be billed. A reliable estimate is unavailable for this route."
        case "managed":
            if let estimate = route?.estimatedAdditionalCostUSD {
                return "Estimated managed cost: up to \(estimate.formatted(.currency(code: "USD")))."
            }
            return "TOHSENO will show an estimate and maximum before managed work starts."
        default:
            return defaults?.ready == true
                ? "Automatic uses your best available configured route."
                : "No usable intelligence route is configured yet."
        }
    }

    public func restartService() async {
        do { try await client.restartService(); await reload() }
        catch { errorMessage = error.localizedDescription }
    }

    public func openLegacyStudio() async {
        do { try await client.openLegacyStudio() }
        catch { errorMessage = error.localizedDescription }
    }

    public func saveCustomHarness() async {
        guard !isSubmitting else { return }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            try await client.configureCustomHarness(customHarness)
            customHarness = CustomHarnessDraft()
            await reload()
        } catch { errorMessage = error.localizedDescription }
    }

    public func saveLocalEndpoint() async {
        guard !isSubmitting else { return }
        isSubmitting = true
        defer { isSubmitting = false }
        do {
            try await client.configureLocalEndpoint(localEndpoint)
            localEndpoint.credential = ""
            await reload()
        } catch { errorMessage = error.localizedDescription }
    }

    public func exportSupportReport() {
        #if canImport(AppKit)
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "TOHSENO Support Report.txt"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let lines = [
            "TOHSENO support report",
            "Generated: \(ISO8601DateFormatter().string(from: Date()))",
            "Service: \(workspace?.serviceVersion ?? "unavailable")",
            "Apps: \(apps.count)",
            "Readiness: \(readiness?.step ?? "unavailable")",
            "No intentions, source, credentials, or raw harness output are included."
        ]
        do { try lines.joined(separator: "\n").write(to: url, atomically: true, encoding: .utf8) }
        catch { errorMessage = error.localizedDescription }
        #endif
    }

    private func repairRoute() {
        if case let .app(id) = route, !apps.contains(where: { $0.id == id }) {
            route = .library
        }
    }

    private func restoreRoute() {
        if let id = preferences.string(forKey: "tohseno.selected-app-id") {
            route = .app(id)
        }
    }

    private func persistRoute() {
        if case let .app(id) = route {
            preferences.set(id, forKey: "tohseno.selected-app-id")
        } else {
            preferences.removeObject(forKey: "tohseno.selected-app-id")
        }
    }

    private static func commandID() -> String {
        "native_\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: ""))"
    }

    private func uniqueFilename(_ candidate: String, among references: [ReferenceDraft]) -> String {
        let existing = Set(references.map(\.filename))
        if !existing.contains(candidate) { return candidate }
        let url = URL(fileURLWithPath: candidate)
        let stem = url.deletingPathExtension().lastPathComponent
        let ext = url.pathExtension
        for index in 2...9 {
            let name = ext.isEmpty ? "\(stem)-\(index)" : "\(stem)-\(index).\(ext)"
            if !existing.contains(name) { return name }
        }
        return "reference-\(UUID().uuidString.lowercased()).\(ext)"
    }
}
