import SwiftUI
import UniformTypeIdentifiers
#if canImport(AppKit)
import AppKit
#endif

public struct TohsenoRootView: View {
    @Bindable private var model: TohsenoAppModel

    public init(model: TohsenoAppModel) {
        self.model = model
    }

    public var body: some View {
        Group {
            if model.isLoading, model.workspace == nil {
                VStack(spacing: 18) {
                    TohsenoLivingMark(size: 54)
                    VStack(spacing: 6) {
                        Text("Opening Tohseno")
                            .font(.title2.weight(.semibold))
                        Text("Your workshop is waking up on this Mac.")
                            .foregroundStyle(TohsenoTheme.silver)
                    }
                }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let readiness = model.readiness, !readiness.ready {
                ReadinessScreen(model: model, readiness: readiness)
            } else if let defaults = model.defaults, !defaults.ready {
                HarnessReadinessScreen(model: model, defaults: defaults)
            } else {
                factory
            }
        }
        .background(TohsenoTheme.void)
        .foregroundStyle(TohsenoTheme.bone)
        .tint(TohsenoTheme.amber)
        .task { model.start() }
        .alert("Tohseno", isPresented: errorBinding) {
            Button("OK") { model.dismissError() }
        } message: {
            Text(model.errorMessage ?? "Something stopped safely.")
        }
        .confirmationDialog(
            "Choose the app scheme",
            isPresented: schemeChoiceBinding,
            titleVisibility: .visible
        ) {
            ForEach(model.adoptionSchemeCandidates, id: \.self) { scheme in
                Button(scheme) {
                    guard let path = model.pendingAdoptionPath else { return }
                    Task { await model.adoptProject(at: path, scheme: scheme) }
                }
            }
            Button("Cancel", role: .cancel) { model.cancelSchemeChoice() }
        } message: {
            Text("Tohseno found more than one iOS app scheme. Choose the one installed on your iPhone.")
        }
    }

    private var factory: some View {
        NavigationSplitView {
            List(selection: routeBinding) {
                Section {
                    Button(action: chooseProject) {
                        Label("Adopt Existing App", systemImage: "folder.badge.plus")
                    }
                    .buttonStyle(.plain)
                    .disabled(model.isSubmitting)
                    .accessibilityIdentifier("adopt-app.sidebar")
                }
                Section("Your Apps") {
                    ForEach(model.apps) { app in
                        Label {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(app.displayName).foregroundStyle(TohsenoTheme.bone)
                                Text(app.presentation.headline)
                                    .font(.caption)
                                    .foregroundStyle(TohsenoTheme.silver)
                                    .lineLimit(1)
                            }
                        } icon: {
                            AppArtwork(data: model.icons[app.id], size: 28, cornerRadius: 6)
                        }
                        .tag(AppRoute.app(app.id))
                        .accessibilityIdentifier("app.\(app.id)")
                    }
                }
                Section {
                    Label("Create App", systemImage: "plus.circle")
                        .tag(AppRoute.create)
                        .accessibilityIdentifier("create-app.sidebar")
                    Label("Registry", systemImage: "point.3.connected.trianglepath.dotted")
                        .tag(AppRoute.registry)
                        .accessibilityIdentifier("registry.sidebar")
                    Label("Profile", systemImage: "person.crop.circle")
                        .tag(AppRoute.profile)
                        .accessibilityIdentifier("profile.sidebar")
                }
            }
            .scrollContentBackground(.hidden)
            .background(TohsenoTheme.carbon)
            .navigationSplitViewColumnWidth(min: 210, ideal: 250)
            .safeAreaInset(edge: .top) {
                HStack(spacing: 10) {
                    TohsenoMark().frame(width: 28, height: 28)
                    Text("Tohseno").font(.headline).tracking(1.2)
                    Spacer()
                }
                .padding(14)
                .background(TohsenoTheme.carbon)
            }
        } detail: {
            switch model.route {
            case .library:
                LibraryEmptyView(adopt: chooseProject) { model.route = .create }
            case .registry:
                RegistryView(model: model)
            case .profile:
                ProfileView(model: model)
            case .create:
                CreationView(model: model)
            case .app:
                if let app = model.selectedApp {
                    AppDetailView(model: model, app: app)
                } else {
                    LibraryEmptyView(adopt: chooseProject) { model.route = .create }
                }
            }
        }
        .navigationSplitViewStyle(.balanced)
    }

    private var routeBinding: Binding<AppRoute?> {
        Binding(get: { model.route }, set: { if let value = $0 { model.route = value } })
    }

    private var errorBinding: Binding<Bool> {
        Binding(get: { model.errorMessage != nil }, set: { if !$0 { model.dismissError() } })
    }

    private var schemeChoiceBinding: Binding<Bool> {
        Binding(
            get: { !model.adoptionSchemeCandidates.isEmpty },
            set: { if !$0 { model.cancelSchemeChoice() } }
        )
    }

    private func chooseProject() {
        #if canImport(AppKit)
        let panel = NSOpenPanel()
        panel.title = "Adopt an iPhone app"
        panel.message = "Choose one Xcode project or workspace. Tohseno will inspect it without restructuring it."
        panel.prompt = "Adopt"
        panel.allowsMultipleSelection = false
        panel.canChooseFiles = true
        panel.canChooseDirectories = true
        panel.treatsFilePackagesAsDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        Task { await model.adoptProject(at: url.path) }
        #endif
    }

    private func stateSymbol(_ state: PresentedState) -> String {
        switch state {
        case .waiting: "clock"
        case .building: "hammer"
        case .readyForPhone: "iphone.gen3"
        case .installing: "arrow.down.to.line"
        case .installed: "checkmark.circle.fill"
        case .failed: "exclamationmark.triangle.fill"
        }
    }
}

#if DEBUG
/// Offscreen visual-QA projection. It renders the exact production app-detail
/// view without asking NavigationSplitView or the owner's live service to
/// participate in a test image.
public struct TohsenoBuildWorkspaceFixtureView: View {
    @Bindable private var model: TohsenoAppModel

    public init(model: TohsenoAppModel) {
        self.model = model
    }

    public var body: some View {
        if let app = model.apps.first {
            AppDetailView(model: model, app: app)
        } else {
            Color.clear
        }
    }
}

/// Offscreen first-open projection used to keep the welcome composition calm
/// at the same size as the shipping window.
public struct TohsenoWelcomeFixtureView: View {
    @Bindable private var model: TohsenoAppModel

    public init(model: TohsenoAppModel) {
        self.model = model
    }

    public var body: some View {
        ReadinessScreen(
            model: model,
            readiness: ReadinessView(
                schema: "tohseno.native-onboarding-view/1",
                ready: false,
                step: "welcome",
                headline: "Bring your iPhone close",
                detail: "Tohseno will check each local connection before it builds anything.",
                primaryAction: "begin",
                primaryLabel: "Begin"
            ),
            introductionInitiallyRevealed: true,
            introductionMotionEnabled: false
        )
            .background(TohsenoTheme.void)
            .foregroundStyle(TohsenoTheme.bone)
            .tint(TohsenoTheme.amber)
    }
}

public struct TohsenoReadinessFixtureView: View {
    private let model: TohsenoAppModel
    private let readiness: ReadinessView

    public init(model: TohsenoAppModel, readiness: ReadinessView) {
        self.model = model
        self.readiness = readiness
    }

    public var body: some View {
        ReadinessScreen(model: model, readiness: readiness, setupInitiallyVisible: true)
            .background(TohsenoTheme.void)
            .foregroundStyle(TohsenoTheme.bone)
            .tint(TohsenoTheme.amber)
    }
}

/// Offscreen Registry projection used to verify the CLI and Companion on-ramp
/// without touching the owner's shell profile, Registry, or paired devices.
public struct TohsenoRegistryFixtureView: View {
    @Bindable private var model: TohsenoAppModel

    public init(model: TohsenoAppModel) {
        self.model = model
    }

    public var body: some View {
        RegistryView(model: model)
            .background(TohsenoTheme.void)
            .foregroundStyle(TohsenoTheme.bone)
            .tint(TohsenoTheme.amber)
    }
}
#endif

private struct StarterCapability: Identifiable, Sendable {
    let id: String
    let title: String
    let systemImage: String
    let requirement: String
}

private struct StarterCapabilitiesView: View {
    @Binding var intention: String
    let deviceDescription: String?
    @State private var selected: Set<String> = []

    private static let marker = "\n\nFeatures I want:\n"
    private static let options = [
        StarterCapability(
            id: "quick-capture", title: "Quick capture", systemImage: "bolt.fill",
            requirement: "Let me capture something in a few seconds."
        ),
        StarterCapability(
            id: "photos", title: "Photos & camera", systemImage: "camera.fill",
            requirement: "Let me take or choose photos when they add useful context."
        ),
        StarterCapability(
            id: "reminders", title: "Reminders", systemImage: "checklist",
            requirement: "Help me remember what matters and mark it complete."
        ),
        StarterCapability(
            id: "notifications", title: "Notifications", systemImage: "bell.fill",
            requirement: "Notify me only at useful moments I choose."
        ),
        StarterCapability(
            id: "location", title: "Location", systemImage: "location.fill",
            requirement: "Use my location only when it is necessary for the feature."
        ),
        StarterCapability(
            id: "offline", title: "Works offline", systemImage: "iphone.gen3",
            requirement: "Keep the core experience useful without an internet connection."
        ),
        StarterCapability(
            id: "share", title: "Share from apps", systemImage: "square.and.arrow.up",
            requirement: "Accept useful items shared from other iPhone apps."
        ),
        StarterCapability(
            id: "private", title: "Private by default", systemImage: "lock.fill",
            requirement: "Keep personal data on my devices unless I explicitly export it."
        ),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Start with what it should do")
                    .font(.headline)
                Text(deviceDescription.map { "Choose useful building blocks for \($0), then edit the description below." }
                    ?? "Choose a few useful building blocks, then edit the description below.")
                    .font(.caption)
                    .foregroundStyle(TohsenoTheme.silver)
            }
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 138), spacing: 9)], spacing: 9) {
                ForEach(Self.options) { option in
                    let isSelected = selected.contains(option.id)
                    Button {
                        if isSelected {
                            selected.remove(option.id)
                        } else {
                            selected.insert(option.id)
                        }
                        updateIntention()
                    } label: {
                        HStack(spacing: 7) {
                            Image(systemName: isSelected ? "checkmark.circle.fill" : option.systemImage)
                                .foregroundStyle(isSelected ? TohsenoTheme.void : TohsenoTheme.amber)
                            Text(option.title)
                                .font(.caption.weight(.semibold))
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 11)
                        .padding(.vertical, 9)
                        .frame(maxWidth: .infinity)
                        .background(isSelected ? TohsenoTheme.amber : TohsenoTheme.graphite)
                        .foregroundStyle(isSelected ? TohsenoTheme.void : TohsenoTheme.bone)
                        .overlay(RoundedRectangle(cornerRadius: 9).stroke(TohsenoTheme.iron))
                        .clipShape(RoundedRectangle(cornerRadius: 9))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("creation.starter.\(option.id)")
                }
            }
        }
        .padding(16)
        .background(TohsenoTheme.carbon)
        .clipShape(RoundedRectangle(cornerRadius: 13))
        .accessibilityIdentifier("creation.starters")
    }

    private func updateIntention() {
        let base = intention.components(separatedBy: Self.marker).first?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let chosen = Self.options.filter { selected.contains($0.id) }
        guard !chosen.isEmpty else {
            intention = base
            return
        }
        let opening = base.isEmpty
            ? "Build a simple personal iPhone app that makes one part of my daily life easier."
            : base
        let requirements = chosen.map { "• \($0.requirement)" }.joined(separator: "\n")
        intention = opening + Self.marker + requirements
    }
}

private struct LibraryEmptyView: View {
    let adopt: () -> Void
    let create: () -> Void

    var body: some View {
        ContentUnavailableView {
            Label("Keep an iPhone app connected", systemImage: "iphone.and.arrow.forward")
        } description: {
            Text("Tohseno connects the app you use on your iPhone to the source and coding harness on this Mac.")
        } actions: {
            Button("Adopt Existing App", action: adopt)
                .buttonStyle(PrimaryActionStyle())
                .accessibilityIdentifier("adopt-app.empty")
            Button("Create a First App", action: create)
                .buttonStyle(.plain)
                .accessibilityIdentifier("create-app.empty")
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct RegistryView: View {
    @Bindable var model: TohsenoAppModel
    @State private var mode = RegistryMode.discover
    @State private var query = ""

    private enum RegistryMode: String, CaseIterable, Identifiable {
        case discover = "Discover"
        case following = "Following"
        case updates = "Updates"
        var id: String { rawValue }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Registry").font(.largeTitle.bold())
                    Text("The world outside your workshop. Software enters once, then changes through Updates.")
                        .foregroundStyle(TohsenoTheme.silver)
                }

                RegistryOnRampCard(model: model)

                HStack(spacing: 18) {
                    Picker("Registry mode", selection: $mode) {
                        ForEach(RegistryMode.allCases) { Text($0.rawValue).tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("registry.modes")
                    TextField("Search apps, Builders, or ShotID", text: $query)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 330)
                        .accessibilityIdentifier("registry.search")
                }

                if model.isLoadingRegistry, model.registrySnapshot == nil {
                    HStack(spacing: 10) {
                        TohsenoSpinner(size: 20)
                        Text("Verifying your local Registry…")
                            .foregroundStyle(TohsenoTheme.silver)
                    }
                } else if let snapshot = model.registrySnapshot {
                    registryWorld(snapshot)
                } else {
                    ContentUnavailableView {
                        Label("Registry unavailable", systemImage: "exclamationmark.triangle")
                    } description: {
                        Text(model.registryMessage ?? "The local Registry could not be inspected.")
                    } actions: {
                        Button("Try Again") { Task { await model.refreshRegistry() } }
                    }
                }
            }
            .frame(maxWidth: 900, alignment: .leading)
            .padding(40)
        }
        .background(TohsenoTheme.void)
        .task {
            await model.refreshRegistry()
        }
    }

    @ViewBuilder private func registryWorld(_ snapshot: RegistrySnapshot) -> some View {
        if mode == .updates {
            updates(snapshot)
        } else {
            let events = visibleEvents(snapshot)
            if events.isEmpty {
                ContentUnavailableView(
                    mode == .following ? "Follow a Builder" : "The network is quiet",
                    systemImage: mode == .following ? "person.crop.circle.badge.plus" : "network",
                    description: Text(mode == .following
                        ? "Following stays private on this Mac and paired Companion. No follower count exists."
                        : snapshot.network.reason)
                )
            } else {
                LazyVStack(alignment: .leading, spacing: 1) {
                    ForEach(events) { event in
                        eventCard(event, snapshot: snapshot)
                    }
                }
                .background(TohsenoTheme.iron)
                .clipShape(RoundedRectangle(cornerRadius: 16))
                .accessibilityIdentifier("registry.timeline")
            }
        }
    }

    private func visibleEvents(_ snapshot: RegistrySnapshot) -> [PublicTimelineEvent] {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return snapshot.timeline.filter { event in
            guard mode != .following || model.followedBuilderIDs.contains(event.builderID) else { return false }
            guard !normalized.isEmpty else { return true }
            let app = snapshot.published.first { $0.release.shotID == event.shotID }
            return [event.shotID, event.builderID, app?.release.display.name ?? "",
                    app?.release.display.description ?? "", app?.release.display.builderHandle ?? ""]
                .joined(separator: " ").lowercased().contains(normalized)
        }
    }

    private func eventCard(_ event: PublicTimelineEvent, snapshot: RegistrySnapshot) -> some View {
        let app = snapshot.published.first { $0.release.shotID == event.shotID }
        return HStack(alignment: .top, spacing: 18) {
            Image(systemName: event.kind == "shot.updated" ? "arrow.triangle.2.circlepath" :
                event.kind == "shot.forked" ? "arrow.triangle.branch" :
                event.kind == "claim.edition_closed" ? "circle.badge.checkmark" : "app.badge")
                .font(.title).foregroundStyle(TohsenoTheme.amber).frame(width: 44)
            VStack(alignment: .leading, spacing: 7) {
                Text(app?.release.display.name ?? compact(event.shotID)).font(.title2.weight(.semibold))
                Text(event.kind == "shot.updated" ? "updated" :
                    event.kind == "shot.forked" ? "was born as a fork" :
                    event.kind == "claim.edition_closed" ? "Claim Edition closed" : "entered Tohseno")
                    .foregroundStyle(TohsenoTheme.silver)
                Text(app?.release.display.description ?? "Signed native software")
                    .font(.subheadline).foregroundStyle(TohsenoTheme.silver).lineLimit(2)
                Text(event.occurredAt).font(.caption.monospaced()).foregroundStyle(TohsenoTheme.silver)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 10) {
                Button(model.followedBuilderIDs.contains(event.builderID) ? "Following" : "Follow") {
                    Task { await model.toggleFollow(builderID: event.builderID) }
                }
                .buttonStyle(.plain).foregroundStyle(TohsenoTheme.amber)
                if let app {
                    Link("View", destination: URL(string: "https://tohseno.com\(app.route)")!)
                }
            }
        }
        .padding(20).background(TohsenoTheme.graphite)
    }

    private func updates(_ snapshot: RegistrySnapshot) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Updates").font(.title2.weight(.semibold))
            if let message = model.networkActionMessage {
                Label(message, systemImage: "arrow.triangle.2.circlepath")
                    .font(.subheadline)
                    .foregroundStyle(TohsenoTheme.amber)
            }
            if let review = model.networkReview {
                VStack(alignment: .leading, spacing: 10) {
                    Label("Requires review on your Mac", systemImage: "exclamationmark.shield")
                        .font(.headline)
                    Text(review.reasons).font(.caption).foregroundStyle(TohsenoTheme.silver)
                    Button("I Reviewed the Source — Build") {
                        Task { await model.approveNetworkReview() }
                    }
                    .buttonStyle(PrimaryActionStyle())
                    .disabled(model.isSubmitting)
                }
                .padding(16).background(TohsenoTheme.carbon)
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(TohsenoTheme.amber))
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            if !snapshot.privateUpdates.isEmpty {
                LazyVStack(spacing: 1) {
                    ForEach(snapshot.privateUpdates) { update in
                        Button {
                            Task { await model.markPrivateUpdateRead(update) }
                        } label: {
                            HStack(alignment: .top, spacing: 12) {
                                Circle()
                                    .fill(update.readAt == nil ? TohsenoTheme.amber : Color.clear)
                                    .frame(width: 8, height: 8)
                                    .padding(.top, 7)
                                VStack(alignment: .leading, spacing: 5) {
                                    Text(update.title).font(.headline).foregroundStyle(.primary)
                                    Text(update.detail).font(.subheadline)
                                        .foregroundStyle(TohsenoTheme.silver)
                                    Text(update.occurredAt).font(.caption.monospaced())
                                        .foregroundStyle(TohsenoTheme.silver)
                                }
                                Spacer()
                            }
                            .padding(16)
                            .background(TohsenoTheme.graphite)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            if model.networkActionMessage == nil, model.networkReview == nil,
               snapshot.privateUpdates.isEmpty {
                ContentUnavailableView("Nothing needs you", systemImage: "checkmark.circle",
                    description: Text("Only high-signal personal software changes appear here."))
            }
        }
        .accessibilityIdentifier("registry.updates")
    }

    private func compact(_ value: String) -> String {
        guard value.count > 30 else { return value }
        return "\(value.prefix(20))…\(value.suffix(8))"
    }
}

private struct RegistryOnRampCard: View {
    @Bindable var model: TohsenoAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: "terminal.fill")
                    .font(.title2)
                    .foregroundStyle(TohsenoTheme.amber)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 5) {
                    Text("Ship from Terminal")
                        .font(.title3.weight(.semibold))
                    Text("Connect an Xcode app, prepare its exact source, and approve the public Ship from Companion.")
                        .foregroundStyle(TohsenoTheme.silver)
                }
                Spacer()
                cliAction
            }

            if model.cliIntegration?.enabled == true {
                VStack(alignment: .leading, spacing: 10) {
                    RegistryCommandRow(number: "1", command: "tohseno init", detail: "Run inside the Xcode app you want to connect.")
                    RegistryCommandRow(number: "2", command: "tohseno deploy --app-slug your-app", detail: "Sign a stable human slug into the source release and send the exact approval to Companion.")
                }
                if let message = model.cliMessage {
                    Label(message, systemImage: "checkmark.circle.fill")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.amber)
                }
            } else {
                Text("The command is already installed inside ~/.tohseno. Activation adds one Tohseno-owned PATH block to \(model.cliIntegration?.profilePath ?? "your shell profile") and preserves everything else.")
                    .font(.caption)
                    .foregroundStyle(TohsenoTheme.silver)
                if let message = model.cliMessage {
                    Text(message).font(.caption).foregroundStyle(.red)
                }
            }

            if let network = model.registrySnapshot?.network {
                Label(
                    network.publishingAvailable
                        ? "Registry publishing is available for Companion-approved Ships."
                        : "Registry browsing is online; new Ships are temporarily closed while the constrained publication relay is disabled.",
                    systemImage: network.publishingAvailable ? "checkmark.shield.fill" : "lock.shield"
                )
                .font(.caption)
                .foregroundStyle(network.publishingAvailable ? TohsenoTheme.amber : TohsenoTheme.silver)
            }

            Divider().overlay(TohsenoTheme.iron)

            HStack(alignment: .top, spacing: 14) {
                Image(systemName: model.pairedCompanionDevices.isEmpty ? "iphone.gen3.badge.exclamationmark" : "iphone.gen3.circle.fill")
                    .font(.title2)
                    .foregroundStyle(TohsenoTheme.amber)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 5) {
                    Text(model.pairedCompanionDevices.isEmpty ? "Pair the iPhone that approves shipping" : "Companion approval is connected")
                        .font(.headline)
                    Text(model.pairedCompanionDevices.isEmpty
                        ? "Scan one pairing QR in Companion. After pairing, deploy approvals arrive on your iPhone automatically—you do not scan a new QR for every Ship."
                        : "Run tohseno deploy --app-slug your-app and the exact source, stable slug, and Registry action will appear on your paired iPhone for approval.")
                        .font(.subheadline)
                        .foregroundStyle(TohsenoTheme.silver)
                }
                Spacer()
                if model.pairedCompanionDevices.isEmpty {
                    Button("Show Pairing QR") { Task { await model.beginCompanionPairing() } }
                        .buttonStyle(PrimaryActionStyle())
                        .accessibilityIdentifier("registry.pair-companion")
                }
            }

            if model.pairedCompanionDevices.isEmpty,
               let session = model.companionPairingSession {
                CompanionPairingCard(session: session)
            }

            Text("After the Ship is verified, share its Registry link. Your friend claims it in Companion; their Mac then verifies the source and signs the iPhone build with their own Apple Account. A QR alone cannot bypass Apple signing.")
                .font(.caption)
                .foregroundStyle(TohsenoTheme.silver)
        }
        .padding(20)
        .background(TohsenoTheme.graphite)
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(TohsenoTheme.iron))
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .accessibilityIdentifier("registry.ship-from-terminal")
    }

    @ViewBuilder private var cliAction: some View {
        if model.cliIntegration?.enabled == true {
            Label("Terminal ready", systemImage: "checkmark.circle.fill")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(TohsenoTheme.amber)
        } else {
            Button(model.isEnablingCLI ? "Activating…" : "Activate CLI") {
                Task { await model.enableCLIIntegration() }
            }
            .buttonStyle(PrimaryActionStyle())
            .disabled(model.isEnablingCLI || model.cliIntegration?.installed != true)
            .accessibilityIdentifier("registry.activate-cli")
        }
    }
}

private struct RegistryCommandRow: View {
    let number: String
    let command: String
    let detail: String

    var body: some View {
        HStack(spacing: 12) {
            Text(number)
                .font(.caption.monospaced().weight(.bold))
                .foregroundStyle(TohsenoTheme.amber)
                .frame(width: 20)
            Text(command)
                .font(.body.monospaced().weight(.semibold))
                .textSelection(.enabled)
            Text(detail)
                .font(.caption)
                .foregroundStyle(TohsenoTheme.silver)
            Spacer()
            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(command, forType: .string)
            } label: {
                Image(systemName: "doc.on.doc")
            }
            .buttonStyle(.plain)
            .help("Copy \(command)")
        }
        .padding(.vertical, 3)
    }
}

private struct RegistryMetric: View {
    let value: String
    let label: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value).font(.title3.weight(.semibold))
            Text(label).font(.caption).foregroundStyle(TohsenoTheme.silver)
        }
    }
}

private struct ProfileView: View {
    @Bindable var model: TohsenoAppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Profile").font(.largeTitle.bold())
                    Text("Your public Builder identity is controlled by the Secure Enclave key on Tohseno Companion.")
                        .foregroundStyle(TohsenoTheme.silver)
                }
                VStack(alignment: .leading, spacing: 14) {
                    Label(
                        model.pairedCompanionDevices.isEmpty
                            ? "Connect Tohseno Companion to become a Builder"
                            : "Builder authority lives on your iPhone",
                        systemImage: model.pairedCompanionDevices.isEmpty
                            ? "iphone.gen3.slash" : "iphone.gen3.badge.play"
                    )
                    .font(.title2.weight(.semibold))
                    Text("This Mac prepares source and builds. It cannot publish without the human approval and DeviceKey signature produced on Companion.")
                        .foregroundStyle(TohsenoTheme.silver)
                    if model.pairedCompanionDevices.isEmpty {
                        Button("Connect Companion") { Task { await model.beginCompanionPairing() } }
                            .buttonStyle(PrimaryActionStyle())
                    }
                }
                .padding(22).background(TohsenoTheme.graphite)
                .clipShape(RoundedRectangle(cornerRadius: 14))

                if let snapshot = model.registrySnapshot {
                    HStack(spacing: 34) {
                        RegistryMetric(value: "\(snapshot.published.count)", label: "network releases visible")
                        RegistryMetric(value: "\(snapshot.records.count)", label: "private apps on this Mac")
                        RegistryMetric(value: snapshot.network.activeGeneration, label: "contract generation")
                    }
                    .padding(22).background(TohsenoTheme.carbon)
                    .overlay(RoundedRectangle(cornerRadius: 14).stroke(TohsenoTheme.iron))
                }
            }
            .frame(maxWidth: 820, alignment: .leading).padding(40)
        }
        .background(TohsenoTheme.void)
        .task {
            await model.refreshCompanionDevices()
            await model.refreshRegistry()
        }
    }
}

private struct ReadinessScreen: View {
    let model: TohsenoAppModel
    let readiness: ReadinessView
    let introductionInitiallyRevealed: Bool
    let introductionMotionEnabled: Bool
    @State private var showsIntroduction: Bool

    init(
        model: TohsenoAppModel,
        readiness: ReadinessView,
        introductionInitiallyRevealed: Bool = false,
        introductionMotionEnabled: Bool = true,
        setupInitiallyVisible: Bool = false
    ) {
        self.model = model
        self.readiness = readiness
        self.introductionInitiallyRevealed = introductionInitiallyRevealed
        self.introductionMotionEnabled = introductionMotionEnabled
        _showsIntroduction = State(
            initialValue: !setupInitiallyVisible
                && (readiness.step == "welcome" || readiness.step == "install_companion")
        )
    }

    var body: some View {
        ZStack {
            if showsIntroduction {
                TohsenoWelcomeSequence(
                    readiness: readiness,
                    enter: enterTohseno,
                    initiallyRevealed: introductionInitiallyRevealed,
                    motionEnabled: introductionMotionEnabled
                )
                    .transition(.opacity.combined(with: .scale(scale: 0.985)))
            } else {
                ReadinessSetupContent(model: model, readiness: readiness)
                    .transition(.opacity.combined(with: .move(edge: .trailing)))
            }
        }
        .animation(.easeInOut(duration: 0.42), value: showsIntroduction)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("readiness.\(readiness.step)")
    }

    private func enterTohseno() {
        withAnimation { showsIntroduction = false }
        guard readiness.step == "welcome" else { return }
        Task { await model.performReadinessAction() }
    }
}

private struct TohsenoWelcomeSequence: View {
    let readiness: ReadinessView
    let enter: () -> Void
    let initiallyRevealed: Bool
    let motionEnabled: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var revealPhase: Int

    init(
        readiness: ReadinessView,
        enter: @escaping () -> Void,
        initiallyRevealed: Bool,
        motionEnabled: Bool
    ) {
        self.readiness = readiness
        self.enter = enter
        self.initiallyRevealed = initiallyRevealed
        self.motionEnabled = motionEnabled
        _revealPhase = State(initialValue: initiallyRevealed ? 3 : 0)
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                TohsenoLivingMark(size: 82, animated: motionEnabled)

                VStack(spacing: 9) {
                    Text("WELCOME TO TOHSENO")
                        .font(.caption.weight(.semibold))
                        .tracking(3.4)
                        .foregroundStyle(TohsenoTheme.amber)
                    Text("TAKE A SHOT")
                        .font(.system(size: 44, weight: .semibold, design: .rounded))
                        .tracking(0.8)
                    Text("This is where your ideas transform into apps.")
                        .font(.title3)
                        .foregroundStyle(TohsenoTheme.silver)
                }
                .multilineTextAlignment(.center)
                .opacity(revealPhase >= 1 ? 1 : 0.45)
                .offset(y: revealPhase >= 1 ? 0 : 7)

                TohsenoJourney(motionEnabled: motionEnabled)
                    .frame(maxWidth: 680)
                    .opacity(revealPhase >= 2 ? 1 : 0.42)
                    .offset(y: revealPhase >= 2 ? 0 : 7)

                VStack(spacing: 12) {
                    Text("Describe what you need in ordinary words. This Mac creates the native iPhone app, keeps its source, and remembers every change. Your iPhone is where it becomes useful.")
                        .font(.body)
                        .foregroundStyle(TohsenoTheme.silver)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 590)

                    HStack(spacing: 18) {
                        Label("Your source stays here", systemImage: "internaldrive")
                        Label("Nothing publishes without you", systemImage: "hand.raised")
                    }
                    .font(.caption)
                    .foregroundStyle(TohsenoTheme.silver.opacity(0.78))
                    .multilineTextAlignment(.center)

                    Button(readiness.step == "welcome" ? "Begin" : "Continue setup", action: enter)
                        .buttonStyle(PrimaryActionStyle())
                        .accessibilityIdentifier("readiness.welcome.begin")

                    Text("You’ll connect your iPhone next. Keep it nearby and unlocked. Tohseno asks for a cable only when Apple requires first pairing.")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.silver.opacity(0.72))
                        .multilineTextAlignment(.center)
                }
                .opacity(revealPhase >= 3 ? 1 : 0.42)
                .offset(y: revealPhase >= 3 ? 0 : 7)
            }
            .padding(.horizontal, 56)
            .padding(.vertical, 34)
            .frame(maxWidth: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(
            RadialGradient(
                colors: [TohsenoTheme.ember.opacity(0.42), TohsenoTheme.void.opacity(0)],
                center: UnitPoint(x: 0.5, y: 0.16),
                startRadius: 8,
                endRadius: 430
            )
        )
        .task { await reveal() }
    }

    private func reveal() async {
        guard !initiallyRevealed else { return }
        if reduceMotion || !motionEnabled {
            revealPhase = 3
            return
        }
        withAnimation(.easeOut(duration: 0.5)) { revealPhase = 1 }
        try? await Task.sleep(for: .milliseconds(180))
        withAnimation(.easeOut(duration: 0.55)) { revealPhase = 2 }
        try? await Task.sleep(for: .milliseconds(220))
        withAnimation(.easeOut(duration: 0.55)) { revealPhase = 3 }
    }
}

private struct TohsenoJourney: View {
    let motionEnabled: Bool

    var body: some View {
        HStack(spacing: 12) {
            JourneyNode(
                icon: "text.bubble.fill",
                title: "Your intention",
                detail: "Say what you need"
            )
            JourneyConnector(animated: motionEnabled)
            JourneyNode(
                icon: "macbook",
                title: "Your Mac",
                detail: "Builds and remembers"
            )
            JourneyConnector(delay: 0.55, animated: motionEnabled)
            JourneyNode(
                icon: "iphone.gen3",
                title: "Your iPhone",
                detail: "Where you use it"
            )
        }
        .padding(.vertical, 4)
    }
}

private struct JourneyNode: View {
    let icon: String
    let title: String
    let detail: String

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundStyle(TohsenoTheme.amber)
                .frame(width: 48, height: 48)
                .background(TohsenoTheme.ember.opacity(0.8))
                .clipShape(Circle())
                .overlay(Circle().stroke(TohsenoTheme.amber.opacity(0.2)))
            Text(title).font(.callout.weight(.semibold))
            Text(detail)
                .font(.caption)
                .foregroundStyle(TohsenoTheme.silver)
                .multilineTextAlignment(.center)
        }
        .frame(width: 138)
    }
}

private struct JourneyConnector: View {
    let delay: Double
    let animated: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var travels = false

    init(delay: Double = 0, animated: Bool = true) {
        self.delay = delay
        self.animated = animated
    }

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(TohsenoTheme.iron)
                    .frame(height: 1)
                Circle()
                    .fill(TohsenoTheme.amber)
                    .frame(width: 5, height: 5)
                    .shadow(color: TohsenoTheme.amber.opacity(0.55), radius: 4)
                    .offset(x: travels && animated && !reduceMotion ? max(0, geometry.size.width - 5) : 0)
            }
            .frame(maxHeight: .infinity)
        }
        .frame(width: 64, height: 12)
        .animation(
            reduceMotion || !animated
                ? nil
                : .easeInOut(duration: 2.1).repeatForever(autoreverses: true).delay(delay),
            value: travels
        )
        .onAppear { travels = true }
        .accessibilityHidden(true)
    }
}

private struct ReadinessSetupContent: View {
    let model: TohsenoAppModel
    let readiness: ReadinessView

    var body: some View {
        ScrollView {
            VStack(spacing: 18) {
                SetupContextBanner(readiness: readiness)

                if readiness.isWorking {
                    TohsenoSpinner(size: 56)
                } else {
                    TohsenoMark().frame(width: 56, height: 56)
                }
                VStack(spacing: 8) {
                    Text(readiness.headline).font(.largeTitle.weight(.semibold))
                    Text(readiness.detail)
                        .foregroundStyle(TohsenoTheme.silver)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 620)
                }
                if let device = model.connectedDeviceDescription, readiness.step != "welcome" {
                    Label(device, systemImage: "iphone.gen3")
                        .font(.callout.weight(.medium))
                        .foregroundStyle(TohsenoTheme.silver)
                }
                ReadinessProgressPanel(readiness: readiness)
                if readiness.companionInstallState == "failed" {
                    Label(
                        "Your iPhone and Apple Account checks already passed. The message above describes the later step that stopped.",
                        systemImage: "checkmark.shield"
                    )
                    .font(.callout)
                    .foregroundStyle(TohsenoTheme.silver)
                    .frame(maxWidth: 620, alignment: .leading)
                }
                if readiness.primaryAction != nil {
                    HStack(spacing: 12) {
                        if readiness.companionInstallState == "failed" {
                            Link(
                                "Check for a Tohseno update",
                                destination: URL(string: "https://tohseno.com/download/macos")!
                            )
                            .buttonStyle(.bordered)
                        }
                        Button(readiness.primaryLabel ?? "Continue") {
                            Task { await model.performReadinessAction() }
                        }
                        .buttonStyle(PrimaryActionStyle())
                        .disabled(model.isSubmitting)
                        .accessibilityIdentifier("readiness.primary")
                    }
                }
                if model.isSubmitting {
                    HStack(spacing: 8) {
                        TohsenoSpinner(size: 18)
                        Text("Starting the next check…")
                            .font(.caption)
                            .foregroundStyle(TohsenoTheme.silver)
                    }
                }
            }
            .padding(.horizontal, 56)
            .padding(.vertical, 36)
            .frame(maxWidth: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct SetupContextBanner: View {
    let readiness: ReadinessView

    var body: some View {
        HStack(spacing: 13) {
            TohsenoMark().frame(width: 34, height: 34)
            VStack(alignment: .leading, spacing: 3) {
                Text("YOUR MAC IS THE WORKSHOP · YOUR IPHONE IS WHERE THE APP LIVES")
                    .font(.caption2.weight(.semibold))
                    .tracking(0.9)
                    .foregroundStyle(TohsenoTheme.amber)
                Label(purpose, systemImage: symbol)
                    .font(.callout)
                    .foregroundStyle(TohsenoTheme.bone)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .frame(maxWidth: 620)
        .background(TohsenoTheme.ember.opacity(0.34))
        .overlay(RoundedRectangle(cornerRadius: 13).stroke(TohsenoTheme.amber.opacity(0.18)))
        .clipShape(RoundedRectangle(cornerRadius: 13))
    }

    private var symbol: String {
        switch readiness.step {
        case "connect_cable", "trust_mac": "cable.connector"
        case "install_xcode", "developer_mode", "add_apple_account": "hammer"
        case "install_companion", "building_companion": "iphone.and.arrow.forward"
        case "installing_companion", "launching_companion": "arrow.down.to.line"
        case "pairing_companion": "link"
        default: "sparkles"
        }
    }

    private var purpose: String {
        switch readiness.step {
        case "connect_cable":
            "This one-time cable step lets Apple pair this Mac and iPhone. After pairing, Tohseno also uses Xcode-supported Wi-Fi delivery when available."
        case "trust_mac":
            "Trust lets your two devices work together directly, without sending your project elsewhere."
        case "install_xcode":
            "Xcode is Apple’s official tool for turning source code into a real native iPhone app."
        case "developer_mode":
            "Developer Mode lets your iPhone run personal apps built and signed on this Mac."
        case "add_apple_account":
            "Your Apple Account lets Xcode sign the app as yours. Tohseno never sees your password."
        case "install_companion", "building_companion":
            "Companion is the private bridge that receives apps from your Mac and keeps them connected."
        case "installing_companion", "launching_companion":
            "The private bridge is moving onto your iPhone now."
        case "pairing_companion":
            "Your iPhone is proving that it belongs to this local Tohseno workspace."
        default:
            "First, Tohseno will connect your Mac workshop to your iPhone."
        }
    }
}

private struct ReadinessProgressPanel: View {
    let readiness: ReadinessView

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Setup progress").font(.headline)
                    Text("Step \(readiness.setupStepNumber) of 8 · \(readiness.setupStatus)")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.silver)
                }
                Spacer()
                Text(readiness.setupProgress.formatted(.percent.precision(.fractionLength(0))))
                    .font(.callout.monospacedDigit().weight(.semibold))
            }
            ProgressView(value: readiness.setupProgress)
                .progressViewStyle(.linear)
                .tint(readiness.companionInstallState == "failed" ? .red : TohsenoTheme.amber)
            VStack(alignment: .leading, spacing: 7) {
                Text("Setup log")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(TohsenoTheme.silver)
                ForEach(readiness.setupCheckpoints) { checkpoint in
                    ReadinessCheckpointRow(checkpoint: checkpoint)
                }
            }
        }
        .padding(16)
        .frame(maxWidth: 620)
        .background(TohsenoTheme.carbon)
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(TohsenoTheme.iron))
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .accessibilityIdentifier("readiness.progress")
    }
}

private struct ReadinessCheckpointRow: View {
    let checkpoint: ReadinessCheckpoint

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: symbol)
                .foregroundStyle(color)
                .frame(width: 16)
            Text(checkpoint.label)
                .foregroundStyle(checkpoint.state == .waiting ? TohsenoTheme.ash : TohsenoTheme.bone)
            Spacer()
            Text(status)
                .font(.caption)
                .foregroundStyle(color)
        }
        .font(.callout)
    }

    private var symbol: String {
        switch checkpoint.state {
        case .complete: "checkmark.circle.fill"
        case .current: "circle.inset.filled"
        case .working: "arrow.triangle.2.circlepath"
        case .failed: "exclamationmark.circle.fill"
        case .waiting: "circle"
        }
    }

    private var status: String {
        switch checkpoint.state {
        case .complete: "Done"
        case .current: "Now"
        case .working: "Working"
        case .failed: "Stopped"
        case .waiting: "Waiting"
        }
    }

    private var color: Color {
        switch checkpoint.state {
        case .complete: TohsenoTheme.silver
        case .current, .working: TohsenoTheme.amber
        case .failed: .red
        case .waiting: TohsenoTheme.ash
        }
    }
}

private struct HarnessReadinessScreen: View {
    let model: TohsenoAppModel
    let defaults: FactoryDefaults

    var body: some View {
        VStack(spacing: 22) {
            TohsenoLivingMark(size: 58)
            VStack(spacing: 10) {
                Text("Choose how Tohseno thinks").font(.largeTitle.weight(.semibold))
                Text("To turn your words into an app, Tohseno works with a coding assistant you already use. Choose one below; its sign-in stays with that tool.")
                    .foregroundStyle(TohsenoTheme.silver)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 560)
            }
            VStack(alignment: .leading, spacing: 10) {
                ForEach(defaults.harnesses) { harness in
                    Label {
                        HStack {
                            Text(harness.label)
                            Spacer()
                            Text(harnessStatus(harness))
                                .foregroundStyle(TohsenoTheme.silver)
                        }
                    } icon: {
                        Image(systemName: harness.installed ? "terminal.fill" : "terminal")
                    }
                }
            }
            .frame(maxWidth: 500)
            HStack {
                SettingsLink { Text("Choose in Settings") }
                    .buttonStyle(PrimaryActionStyle())
                Button("Look Again") { Task { await model.reload() } }
            }
            Text("For Codex, install and sign in with Codex itself. Tohseno never asks for your coding-provider password, API key, or Apple password here.")
                .font(.caption)
                .foregroundStyle(TohsenoTheme.ash)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 520)
        }
        .padding(56)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("readiness.harness")
    }

    private func harnessStatus(_ harness: FactoryHarnessOption) -> String {
        if !harness.installed { return "Not installed" }
        if harness.authentication == .notDetected { return "Sign in required" }
        return harness.selected ? "Selected" : "Available"
    }
}

private struct CreationView: View {
    @Bindable var model: TohsenoAppModel
    @State private var choosingReferences = false
    @FocusState private var intentionFocused: Bool

    private var canSubmit: Bool {
        !model.isSubmitting &&
            !model.creation.intention.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                Text("Create App").font(.largeTitle.bold())
                Text("What would make your life easier?")
                    .font(.title2.weight(.semibold))
                StarterCapabilitiesView(
                    intention: $model.creation.intention,
                    deviceDescription: model.connectedDeviceDescription
                )
                TextEditor(text: $model.creation.intention)
                    .font(.body)
                    .scrollContentBackground(.hidden)
                    .padding(10)
                    .frame(minHeight: 180)
                    .background(TohsenoTheme.carbon)
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(TohsenoTheme.iron))
                    .focused($intentionFocused)
                    .shotSubmitOnReturn(enabled: canSubmit) {
                        Task { await model.submitCreation() }
                    }
                    .accessibilityLabel("What would make your life easier?")
                    .accessibilityIdentifier("creation.intention")
                TextField("Optional app name", text: $model.creation.name)
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("creation.name")
                ReferenceStrip(references: model.creation.references) { _, id in
                    model.creation.references.removeAll { $0.id == id }
                }
                HStack {
                    Button("Add reference images…") { choosingReferences = true }
                        .disabled(model.creation.references.count >= 8)
                        .accessibilityIdentifier("creation.references")
                    Text("or drop PNG/JPEG images here · \(model.creation.references.count)/8")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.silver)
                }
                if model.defaults?.ready != true, model.managedCatalog?.models.isEmpty == false {
                    ManagedAccessCard(model: model)
                }
                AdvancedRouteDisclosure(
                    model: model,
                    harness: $model.creation.harness,
                    selectedModel: $model.creation.model,
                    privacy: $model.creation.managedPrivacy,
                    maximumMicrousd: $model.creation.managedMaximumMicrousd,
                    consent: $model.creation.managedConsent,
                    estimate: model.creationEstimate
                )
                .task(id: creationEstimateKey) { await model.estimateCreation() }
                HStack {
                    RouteCostView(model: model, harness: model.creation.harness, estimate: model.creationEstimate)
                    ShotKeyboardHint()
                    Spacer()
                    Button {
                        Task { await model.submitCreation() }
                    } label: {
                        HStack(spacing: 7) {
                            if model.isSubmitting {
                                TohsenoSpinner(
                                    size: 14,
                                    stroke: TohsenoTheme.void,
                                    gap: TohsenoTheme.amber
                                )
                            }
                            Text(model.isSubmitting ? "Creating…" : "Create App")
                        }
                    }
                    .buttonStyle(PrimaryActionStyle())
                    .disabled(!canSubmit)
                    .keyboardShortcut(.return, modifiers: [])
                    .accessibilityIdentifier("creation.submit")
                }
            }
            .frame(maxWidth: 760, alignment: .leading)
            .padding(40)
        }
        .background(TohsenoTheme.void)
        .fileImporter(isPresented: $choosingReferences, allowedContentTypes: [.png, .jpeg], allowsMultipleSelection: true) { result in
            model.addReferences(result, to: .creation)
        }
        .dropDestination(for: URL.self) { urls, _ in
            model.addReferences(.success(urls), to: .creation)
            return !urls.isEmpty
        }
        .task { intentionFocused = true }
    }

    private var creationEstimateKey: String {
        [
            model.creation.harness ?? "automatic",
            model.creation.model ?? "default",
            model.creation.managedPrivacy,
            String(model.creation.intention.utf8.count),
            String(model.creation.references.reduce(0) { $0 + $1.data.count })
        ].joined(separator: ":")
    }
}

private enum AppWorkspaceTab: String, CaseIterable, Identifiable {
    case build = "Build"
    case app = "App"
    case source = "Source"

    var id: String { rawValue }
}

private struct AppDetailView: View {
    @Bindable var model: TohsenoAppModel
    let app: AppSummary
    @State private var tab = AppWorkspaceTab.build
    @State private var showingEvolution = false
    @State private var showingDetails = false
    @State private var confirmingRetire = false

    var body: some View {
        VStack(spacing: 0) {
            workspaceHeader
            Divider()
            HStack(spacing: 0) {
                ScrollView {
                    Group {
                        switch tab {
                        case .build:
                            BuildWorkspaceView(model: model, app: app)
                        case .app:
                            AppWorkspaceView(
                                model: model,
                                app: app,
                                change: { showingEvolution = true },
                                details: showDetails
                            )
                        case .source:
                            SourceWorkspaceView(
                                model: model,
                                app: app,
                                details: showDetails,
                                retire: { confirmingRetire = true }
                            )
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                    .padding(28)
                }
                .frame(maxWidth: .infinity)

                Divider()

                ScrollView {
                    IPhoneWorkspaceView(model: model, app: app)
                        .padding(22)
                }
                .frame(minWidth: 280, idealWidth: 330, maxWidth: 360)
                .background(Color(nsColor: .controlBackgroundColor).opacity(0.45))
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .foregroundStyle(.primary)
        .task(id: app.id) { await model.prepareEvolution(for: app) }
        .sheet(isPresented: $showingEvolution) {
            EvolutionComposerSheet(model: model, app: app, isPresented: $showingEvolution)
        }
        .sheet(isPresented: $showingDetails) {
            ExecutionDetailsView(receipt: model.receipt, balance: model.managedBalance)
        }
        .confirmationDialog("Retire \(app.displayName)?", isPresented: $confirmingRetire) {
            Button("Retire App", role: .destructive) { Task { await model.retire(app) } }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This removes the app from Your Apps. Its source and history stay on this Mac and remain recoverable.")
        }
    }

    private var workspaceHeader: some View {
        HStack(spacing: 14) {
            AppArtwork(data: model.icons[app.id], size: 48, cornerRadius: 11)
            VStack(alignment: .leading, spacing: 3) {
                Text(app.displayName)
                    .font(.title2.weight(.semibold))
                    .lineLimit(1)
                Label(app.presentation.headline, systemImage: stateSymbol(app.presentation.state))
                    .font(.caption)
                    .foregroundStyle(app.presentation.state == .failed ? .red : .secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 12)
            Picker("Workspace", selection: $tab) {
                ForEach(AppWorkspaceTab.allCases) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .labelsHidden()
            .pickerStyle(.segmented)
            .frame(width: 225)
            .accessibilityIdentifier("app.workspace-tabs")
            if app.latestVersionID != nil || app.sourceState != nil,
               !app.presentation.state.isInFlight {
                Button("What should change?") { showingEvolution = true }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("app.change")
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 14)
        .background(.bar)
    }

    private func showDetails() {
        Task {
            await model.showReceipt(for: app)
            showingDetails = true
        }
    }

    private func stateSymbol(_ state: PresentedState) -> String {
        switch state {
        case .waiting: "clock"
        case .building: "hammer.fill"
        case .readyForPhone: "iphone.gen3"
        case .installing: "arrow.down.to.line.compact"
        case .installed: "checkmark.circle.fill"
        case .failed: "exclamationmark.triangle.fill"
        }
    }
}

private struct BuildWorkspaceView: View {
    let model: TohsenoAppModel
    let app: AppSummary

    private var activity: ExecutionActivity? { model.activities[app.id] }
    private var files: [ExecutionActivityFile] { activity?.files ?? [] }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Build")
                    .font(.largeTitle.bold())
                Text(app.presentation.detail ?? progressLanguage(app.presentation.state))
                    .foregroundStyle(.secondary)
            }

            GroupBox {
                BuildJourney(state: app.presentation.state)
                    .padding(.vertical, 8)
            } label: {
                Label("From request to iPhone", systemImage: "point.forward.to.point.capsulepath")
            }

            GroupBox {
                VStack(alignment: .leading, spacing: 0) {
                    if files.isEmpty {
                        ContentUnavailableView(
                            "No source changes yet",
                            systemImage: "doc.badge.ellipsis",
                            description: Text(app.presentation.state.isInFlight
                                ? "Files appear here as the app takes shape."
                                : "This build did not report changed source files.")
                        )
                        .frame(maxWidth: .infinity, minHeight: 115)
                    } else {
                        ForEach(Array(files.enumerated()), id: \.element.id) { index, file in
                            SourceFileRow(file: file)
                            if index != files.indices.last { Divider() }
                        }
                        if activity?.filesTruncated == true {
                            Text("Showing the first \(files.count) of \(activity?.fileCount ?? files.count) files.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .padding(.top, 10)
                        }
                    }
                }
                .padding(.top, 4)
                .frame(maxWidth: .infinity, alignment: .leading)
            } label: {
                Label(fileHeading, systemImage: "doc.on.doc")
            }
            .accessibilityIdentifier("app.files")

            GroupBox {
                VStack(alignment: .leading, spacing: 0) {
                    if let entries = activity?.entries, !entries.isEmpty {
                        ForEach(Array(entries.enumerated()), id: \.element.id) { index, entry in
                            ActivityRow(entry: entry, isLast: index == entries.indices.last)
                        }
                    } else {
                        HStack(spacing: 10) {
                            if app.presentation.state.isInFlight { TohsenoSpinner(size: 18) }
                            Text(app.presentation.state.isInFlight
                                ? "Waiting for the first factory update…"
                                : "No build log is available for this app yet.")
                                .foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity, minHeight: 70, alignment: .leading)
                    }
                }
                .padding(.top, 4)
                .frame(maxWidth: .infinity, alignment: .leading)
            } label: {
                HStack {
                    Label("Build log", systemImage: "text.alignleft")
                    Spacer()
                    if let tokens = activity?.totalTokens {
                        Text("\(tokens.formatted()) tokens")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .accessibilityIdentifier("app.build-log")

            if let history = app.recentEvolutions, !history.isEmpty {
                GroupBox {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(history.enumerated()), id: \.element.id) { index, evolution in
                            VStack(alignment: .leading, spacing: 5) {
                                HStack {
                                    Text(evolution.requestSummary)
                                        .font(.body.weight(.medium))
                                        .lineLimit(3)
                                    Spacer()
                                    Text(evolution.status.replacingOccurrences(of: "_", with: " ").capitalized)
                                        .font(.caption.weight(.semibold))
                                        .foregroundStyle(.secondary)
                                }
                                if let completion = evolution.completionSummary {
                                    Text(completion)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let installation = evolution.installationSummary {
                                    Text(installation)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            .padding(.vertical, 9)
                            if index != history.indices.last { Divider() }
                        }
                    }
                } label: {
                    Label("Evolution history", systemImage: "clock.arrow.circlepath")
                }
                .accessibilityIdentifier("app.evolution-history")
            }
        }
    }

    private var fileHeading: String {
        let count = activity?.fileCount ?? files.count
        return count == 1 ? "1 source file changed" : "\(count) source files changed"
    }
}

private struct BuildJourney: View {
    let state: PresentedState
    private let stages = [
        ("Intent", "text.bubble"),
        ("Source", "curlybraces"),
        ("Simulator", "iphone"),
        ("iPhone", "iphone.gen3"),
    ]

    var body: some View {
        HStack(spacing: 8) {
            ForEach(Array(stages.enumerated()), id: \.offset) { index, stage in
                VStack(spacing: 7) {
                    ZStack {
                        Circle()
                            .fill(color(for: index).opacity(isReached(index) ? 1 : 0.11))
                            .frame(width: 34, height: 34)
                        if isActive(index), state != .failed, state != .installed {
                            TohsenoSpinner(size: 22, stroke: .white, gap: color(for: index))
                        } else {
                            Image(systemName: symbol(for: index, fallback: stage.1))
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(isReached(index) ? .white : .secondary)
                        }
                    }
                    Text(stage.0)
                        .font(.caption.weight(isActive(index) ? .semibold : .regular))
                        .foregroundStyle(isReached(index) ? .primary : .secondary)
                        .lineLimit(1)
                }
                .frame(maxWidth: .infinity)
                if index < stages.count - 1 {
                    Capsule()
                        .fill(index < activeIndex ? TohsenoTheme.amber : Color.secondary.opacity(0.18))
                        .frame(height: 2)
                        .offset(y: -11)
                }
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Build path: intent, source, Simulator, your iPhone")
    }

    private var activeIndex: Int {
        switch state {
        case .waiting: 0
        case .building, .failed: 1
        case .readyForPhone: 2
        case .installing, .installed: 3
        }
    }

    private func isReached(_ index: Int) -> Bool { index <= activeIndex }
    private func isActive(_ index: Int) -> Bool { index == activeIndex }
    private func color(for index: Int) -> Color {
        state == .failed && index == activeIndex ? .red : TohsenoTheme.amber
    }
    private func symbol(for index: Int, fallback: String) -> String {
        if state == .failed && index == activeIndex { return "exclamationmark" }
        if state == .installed && index == stages.count - 1 { return "checkmark" }
        if index < activeIndex { return "checkmark" }
        return fallback
    }
}

private struct ActivityRow: View {
    let entry: ExecutionActivityEntry
    let isLast: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 11) {
            VStack(spacing: 0) {
                Circle()
                    .fill(isLast ? TohsenoTheme.amber : Color.secondary.opacity(0.4))
                    .frame(width: 8, height: 8)
                    .padding(.top, 5)
                if !isLast {
                    Rectangle()
                        .fill(Color.secondary.opacity(0.18))
                        .frame(width: 1, height: 34)
                }
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.message)
                    .textSelection(.enabled)
                Text("Update \(entry.sequence)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(.bottom, isLast ? 0 : 12)
        }
    }
}

private struct SourceFileRow: View {
    let file: ExecutionActivityFile

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: icon)
                .foregroundStyle(color)
                .frame(width: 18)
            Text(file.path)
                .font(.system(.body, design: .monospaced))
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
            Spacer()
            if let additions = file.additions, additions > 0 {
                Text("+\(additions)").foregroundStyle(.green)
            }
            if let deletions = file.deletions, deletions > 0 {
                Text("−\(deletions)").foregroundStyle(.red)
            }
        }
        .font(.caption)
        .padding(.vertical, 8)
    }

    private var icon: String {
        if file.status.contains("D") { return "doc.badge.minus" }
        if file.status.contains("A") || file.status == "??" { return "doc.badge.plus" }
        if file.status.contains("R") { return "arrow.right.doc.on.clipboard" }
        return "doc.badge.ellipsis"
    }

    private var color: Color {
        if file.status.contains("D") { return .red }
        if file.status.contains("A") || file.status == "??" { return .green }
        return TohsenoTheme.amber
    }
}

private struct AppWorkspaceView: View {
    let model: TohsenoAppModel
    let app: AppSummary
    let change: () -> Void
    let details: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Your app")
                    .font(.largeTitle.bold())
                Text("The accepted app, its next change, and the device it lives on.")
                    .foregroundStyle(.secondary)
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 18) {
                    LabeledContent("Status", value: app.presentation.headline)
                    if let ordinal = app.latestVersionOrdinal {
                        LabeledContent("Accepted version", value: "\(ordinal)")
                    }
                    if let bundleIdentifier = app.bundleIdentifier {
                        LabeledContent("Bundle", value: bundleIdentifier)
                    }
                    Divider()
                    Button("What should change?", action: change)
                        .buttonStyle(.borderedProminent)
                        .controlSize(.large)
                        .disabled(app.latestVersionID == nil || app.presentation.state.isInFlight)
                    Text("Describe one change. Return sends; Shift–Return adds a line.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 5)
            } label: {
                Label("App overview", systemImage: "app.dashed")
            }
            HStack {
                if app.presentation.state == .installed {
                    Button("Open on iPhone") { Task { await model.openOnPhone(for: app) } }
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("app.open-on-iphone")
                }
                Button("Details…", action: details)
                Spacer()
                Button("Ship…") { Task { await model.ship(app) } }
                    .buttonStyle(.borderedProminent)
                    .disabled(model.isSubmitting || app.presentation.state.isInFlight)
                    .accessibilityIdentifier("app.ship")
            }
        }
    }
}

private struct SourceWorkspaceView: View {
    let model: TohsenoAppModel
    let app: AppSummary
    let details: () -> Void
    let retire: () -> Void

    private var files: [ExecutionActivityFile] { model.activities[app.id]?.files ?? [] }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Source")
                    .font(.largeTitle.bold())
                Text("This app's source and complete Git history stay on your Mac.")
                    .foregroundStyle(.secondary)
            }
            Button("Open Source Folder") { Task { await model.openSource(for: app) } }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .accessibilityIdentifier("app.open-source")
            GroupBox("Latest changes") {
                VStack(alignment: .leading, spacing: 0) {
                    if files.isEmpty {
                        Text(model.receipt?.diffSummary ?? "No file-level summary is available yet.")
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, minHeight: 70, alignment: .leading)
                    } else {
                        ForEach(Array(files.enumerated()), id: \.element.id) { index, file in
                            SourceFileRow(file: file)
                            if index != files.indices.last { Divider() }
                        }
                    }
                }
                .padding(.top, 5)
            }
            HStack {
                Button("Details…", action: details)
                Spacer()
                Button("Retire App…", role: .destructive, action: retire)
            }
        }
    }
}

private struct IPhoneWorkspaceView: View {
    let model: TohsenoAppModel
    let app: AppSummary

    var body: some View {
        VStack(spacing: 16) {
            HStack {
                Label("iPhone", systemImage: "iphone")
                    .font(.headline)
                Spacer()
                if model.previews[app.id] != nil {
                    Text("SIMULATOR")
                        .font(.caption2.weight(.semibold))
                        .tracking(1)
                        .foregroundStyle(.secondary)
                }
            }
            IPhonePreview(data: model.previews[app.id], state: app.presentation.state)
                .accessibilityIdentifier("app.preview")
            Text(model.previews[app.id] == nil
                ? "The Simulator appears here as soon as the first verified app is ready."
                : "Latest verified Simulator capture · not interactive")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            DeviceHandoffCard(model: model, app: app)
        }
    }
}

private struct IPhonePreview: View {
    let data: Data?
    let state: PresentedState

    var body: some View {
        ZStack(alignment: .top) {
            RoundedRectangle(cornerRadius: 38, style: .continuous)
                .fill(Color.black)
            Group {
                if let data, let image = NSImage(data: data) {
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFill()
                        .accessibilityLabel("Latest verified iPhone Simulator preview")
                } else {
                    VStack(spacing: 14) {
                        if state.isInFlight {
                            TohsenoSpinner(size: 38, stroke: TohsenoTheme.amber, gap: .black)
                        } else {
                            TohsenoMark(stroke: TohsenoTheme.amber, gap: .black)
                                .frame(width: 38, height: 38)
                        }
                        Text(previewMessage)
                            .font(.caption)
                            .foregroundStyle(.white.opacity(0.7))
                            .multilineTextAlignment(.center)
                    }
                    .padding(30)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 31, style: .continuous))
            .padding(7)
            Capsule()
                .fill(Color.black)
                .frame(width: 72, height: 21)
                .padding(.top, 13)
        }
        .aspectRatio(0.51, contentMode: .fit)
        .frame(maxHeight: 410)
        .shadow(color: .black.opacity(0.2), radius: 14, y: 7)
    }

    private var previewMessage: String {
        switch state {
        case .waiting: "Preparing the app"
        case .building: "Building for Simulator"
        case .readyForPhone: "Preview is being prepared"
        case .installing: "Installing on your iPhone"
        case .installed: "Preview unavailable"
        case .failed: "Build stopped safely"
        }
    }
}

private struct DeviceHandoffCard: View {
    let model: TohsenoAppModel
    let app: AppSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 11) {
            HStack(alignment: .top, spacing: 11) {
                Image(systemName: symbol)
                    .font(.title2)
                    .foregroundStyle(app.presentation.state == .failed ? .red : TohsenoTheme.amber)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(.headline)
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            if app.presentation.state == .installed {
                Button("Open on iPhone") { Task { await model.openOnPhone(for: app) } }
                    .buttonStyle(.borderedProminent)
                    .frame(maxWidth: .infinity)
                    .accessibilityIdentifier("app.open-on-iphone")
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(nsColor: .windowBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Color.secondary.opacity(0.18)))
        .accessibilityIdentifier("app.iphone-handoff")
    }

    private var title: String {
        switch app.presentation.state {
        case .waiting, .building, .readyForPhone: "Make your iPhone reachable"
        case .installing: "Installing on your iPhone"
        case .installed: "Your app is on your iPhone"
        case .failed: "Your source is safe"
        }
    }

    private var detail: String {
        switch app.presentation.state {
        case .waiting, .building: "Keep the paired iPhone nearby, unlocked, and on the same Wi-Fi. USB remains available when Xcode needs it."
        case .readyForPhone: "The verified build is saved. Installation begins when the paired iPhone is reachable over Wi-Fi or USB."
        case .installing: "Keep the iPhone unlocked until the app opens."
        case .installed: "Future builds use Xcode-supported Wi-Fi or USB whenever this paired iPhone is reachable."
        case .failed: "Open Build to see where work stopped. Nothing accepted was replaced."
        }
    }

    private var symbol: String {
        switch app.presentation.state {
        case .installing: "arrow.down.to.line.compact"
        case .installed: "checkmark.circle.fill"
        case .failed: "exclamationmark.triangle.fill"
        default: "iphone.gen3"
        }
    }
}

private struct EvolutionComposerSheet: View {
    @Bindable var model: TohsenoAppModel
    let app: AppSummary
    @Binding var isPresented: Bool
    @State private var choosingReferences = false
    @FocusState private var intentionFocused: Bool

    private var draft: Binding<EvolutionDraft> {
        Binding(
            get: { model.evolutions[app.id] ?? EvolutionDraft() },
            set: { model.evolutions[app.id] = $0 }
        )
    }

    private var canSubmit: Bool {
        !model.isSubmitting && (app.sourceState != nil || app.latestVersionID != nil) &&
            !draft.wrappedValue.intention.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("What should change?")
                        .font(.title.bold())
                    Text("One clear change to \(app.displayName).")
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Cancel") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
            }
            TextEditor(text: draft.intention)
                .font(.body)
                .padding(10)
                .frame(minHeight: 150)
                .background(Color(nsColor: .textBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 9))
                .overlay(RoundedRectangle(cornerRadius: 9).stroke(Color.secondary.opacity(0.22)))
                .focused($intentionFocused)
                .shotSubmitOnReturn(enabled: canSubmit, action: submit)
                .accessibilityIdentifier("evolution.intention")
            ReferenceStrip(references: draft.wrappedValue.references) { _, id in
                draft.wrappedValue.references.removeAll { $0.id == id }
            }
            Button("Add reference images…") { choosingReferences = true }
                .disabled(draft.wrappedValue.references.count >= 8)
            AdvancedRouteDisclosure(
                model: model,
                harness: draft.harness,
                selectedModel: draft.model,
                privacy: draft.managedPrivacy,
                maximumMicrousd: draft.managedMaximumMicrousd,
                consent: draft.managedConsent,
                estimate: model.evolutionEstimates[app.id]
            )
            .task(id: evolutionEstimateKey) { await model.estimateEvolution(for: app) }
            Spacer(minLength: 0)
            HStack {
                RouteCostView(
                    model: model,
                    harness: draft.wrappedValue.harness,
                    estimate: model.evolutionEstimates[app.id]
                )
                ShotKeyboardHint()
                Spacer()
                Button(action: submit) {
                    HStack(spacing: 7) {
                        if model.isSubmitting {
                            TohsenoSpinner(size: 14, stroke: .white, gap: TohsenoTheme.amber)
                        }
                        Text(model.isSubmitting ? "Sending…" : "Send change")
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!canSubmit)
                .keyboardShortcut(.return, modifiers: [])
                .accessibilityIdentifier("evolution.submit")
            }
        }
        .padding(26)
        .frame(minWidth: 650, minHeight: 500)
        .background(Color(nsColor: .windowBackgroundColor))
        .foregroundStyle(.primary)
        .fileImporter(
            isPresented: $choosingReferences,
            allowedContentTypes: [.png, .jpeg],
            allowsMultipleSelection: true
        ) { result in
            model.addReferences(result, to: .evolution(app.id))
        }
        .dropDestination(for: URL.self) { urls, _ in
            model.addReferences(.success(urls), to: .evolution(app.id))
            return !urls.isEmpty
        }
        .onAppear { intentionFocused = true }
    }

    private func submit() {
        guard canSubmit else { return }
        Task {
            await model.submitEvolution(for: app)
            if model.errorMessage == nil { isPresented = false }
        }
    }

    private var evolutionEstimateKey: String {
        let value = draft.wrappedValue
        return [
            value.harness ?? "automatic",
            value.model ?? "default",
            value.managedPrivacy,
            String(value.intention.utf8.count),
            String(value.references.reduce(0) { $0 + $1.data.count }),
            app.id
        ].joined(separator: ":")
    }
}

private func progressLanguage(_ state: PresentedState) -> String {
    switch state {
    case .waiting: "Waiting for the local factory."
    case .building: "Creating the interface, writing source, and checking the build."
    case .readyForPhone: "The verified build is ready for your connected iPhone."
    case .installing: "Installing the verified build on your iPhone."
    case .installed: "The latest accepted version is installed."
    case .failed: "Work stopped safely. The build log explains where."
    }
}

private struct AppArtwork: View {
    let data: Data?
    let size: CGFloat
    let cornerRadius: CGFloat

    var body: some View {
        Group {
            if let data, let image = NSImage(data: data) {
                Image(nsImage: image).resizable().scaledToFill()
            } else {
                RoundedRectangle(cornerRadius: cornerRadius)
                    .fill(TohsenoTheme.graphite)
                    .overlay(Image(systemName: "app.fill").foregroundStyle(TohsenoTheme.silver))
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
        .accessibilityHidden(true)
    }
}

private struct AdvancedRouteDisclosure: View {
    @Bindable var model: TohsenoAppModel
    @Binding var harness: String?
    @Binding var selectedModel: String?
    @Binding var privacy: String
    @Binding var maximumMicrousd: UInt64?
    @Binding var consent: Bool
    let estimate: ManagedEstimate?

    var body: some View {
        DisclosureGroup("Choose intelligence", isExpanded: $model.advancedExpanded) {
            VStack(alignment: .leading, spacing: 12) {
                Picker("Intelligence", selection: $harness) {
                    Text("Automatic — \(model.defaults?.harnessLabel ?? "best available")").tag(String?.none)
                    ForEach(model.defaults?.harnesses ?? []) { option in
                        Text("\(option.label) — \(availability(option))").tag(Optional(option.id))
                    }
                    if model.managedCatalog?.models.isEmpty == false {
                        Text("Tohseno managed intelligence — uses creation balance")
                            .tag(Optional("tohseno-managed"))
                    }
                }
                .accessibilityIdentifier("advanced.harness")
                .onChange(of: harness) { _, selection in
                    consent = false
                    if selection == "tohseno-managed", selectedModel == nil {
                        selectedModel = model.managedCatalog?.models.first?.model
                    }
                }
                if harness == "tohseno-managed" {
                    managedControls
                } else if let harness, let option = model.defaults?.harnesses.first(where: { $0.id == harness }) {
                    Picker("Model", selection: $selectedModel) {
                        ForEach(option.models) { choice in
                            Text(choice.label).tag(Optional(choice.id))
                        }
                    }
                    .accessibilityIdentifier("advanced.model")
                    Text("Tohseno will use this exact selection for this request and will not substitute another route during recovery.")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.silver)
                }
            }
            .padding(.top, 10)
        }
    }

    @ViewBuilder private var managedControls: some View {
        Picker("Managed model", selection: $selectedModel) {
            ForEach(model.managedCatalog?.models ?? []) { choice in
                Text(choice.model).tag(Optional(choice.model))
            }
        }
        .accessibilityIdentifier("advanced.managed.model")
        if let selectedModel,
           let selected = model.managedCatalog?.models.first(where: { $0.model == selectedModel }) {
            Picker("Privacy", selection: $privacy) {
                ForEach(selected.privacyTiers, id: \.self) { tier in
                    Text(privacyLabel(tier)).tag(tier)
                }
            }
            .onChange(of: privacy) { _, _ in consent = false }
            .accessibilityIdentifier("advanced.managed.privacy")
        }
        Text(model.managedEstimateDescription(estimate))
            .font(.caption)
            .foregroundStyle(TohsenoTheme.silver)
            .accessibilityIdentifier("advanced.managed.estimate")
        if let estimate {
            Stepper(
                "Maximum authorized: \(model.currency(maximumMicrousd ?? estimate.recommendedMaximumMicrousd))",
                value: Binding(
                    get: { maximumMicrousd ?? estimate.recommendedMaximumMicrousd },
                    set: { maximumMicrousd = $0; consent = false }
                ),
                in: estimate.highMicrousd...100_000_000,
                step: 10_000
            )
            Toggle(
                "I approve up to \(model.currency(maximumMicrousd ?? estimate.recommendedMaximumMicrousd)) for this request",
                isOn: $consent
            )
            .accessibilityIdentifier("advanced.managed.consent")
        }
        if let balance = model.managedBalance {
            Text("Spendable creation balance: \(model.signedCurrency(balance.spendableMicrousd))")
                .font(.caption)
        }
        Text("Managed work sends necessary app context through Tohseno and Bankr to the selected upstream provider under this privacy tier. It never activates automatically.")
            .font(.caption)
            .foregroundStyle(TohsenoTheme.silver)
    }

    private func privacyLabel(_ value: String) -> String {
        switch value {
        case "zdr": "Zero data retention"
        case "private": "Private inference"
        default: "Standard"
        }
    }

    private func availability(_ option: FactoryHarnessOption) -> String {
        if !option.installed { return "not installed" }
        if option.authentication == .notDetected { return "needs sign-in" }
        if !option.routes.contains(where: \.available) { return "needs attention" }
        return option.authentication == .authenticated ? "ready" : "configured"
    }
}

private struct RouteCostView: View {
    let model: TohsenoAppModel
    let harness: String?
    let estimate: ManagedEstimate?

    var body: some View {
        Text(harness == "tohseno-managed" ? model.managedEstimateDescription(estimate) : model.costDescription(for: harness))
            .font(.caption)
            .foregroundStyle(TohsenoTheme.silver)
            .accessibilityIdentifier("route.cost")
    }
}

private struct ManagedAccessCard: View {
    let model: TohsenoAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("No local intelligence route is ready").font(.headline)
            Text("You can use Tohseno-managed intelligence with creation balance, or configure your own route in Settings.")
                .foregroundStyle(TohsenoTheme.silver)
            if let balance = model.managedBalance {
                Text("Available balance: \(model.signedCurrency(balance.spendableMicrousd))")
            }
            HStack {
                Button("Use Managed Intelligence") { Task { await model.chooseManagedForCreation() } }
                if model.managedStatus?.welcomeContactURL != nil {
                    Button("Message JP for Welcome Compute") { model.requestWelcomeCompute() }
                }
            }
        }
        .padding(14)
        .background(TohsenoTheme.carbon)
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .accessibilityIdentifier("managed.access")
    }
}

private struct ReferenceStrip: View {
    let references: [ReferenceDraft]
    let remove: (ReferenceDraft, UUID) -> Void

    var body: some View {
        if !references.isEmpty {
            ScrollView(.horizontal) {
                HStack {
                    ForEach(references) { reference in
                        HStack(spacing: 6) {
                            Image(systemName: "photo")
                            Text(reference.filename).lineLimit(1)
                            Button { remove(reference, reference.id) } label: {
                                Image(systemName: "xmark.circle.fill")
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("Remove \(reference.filename)")
                        }
                        .padding(8)
                        .background(TohsenoTheme.graphite)
                        .clipShape(RoundedRectangle(cornerRadius: 7))
                    }
                }
            }
        }
    }
}

private struct ShotKeyboardHint: View {
    var body: some View {
        Label("Return sends · Shift–Return adds a line", systemImage: "return")
            .font(.caption)
            .foregroundStyle(TohsenoTheme.silver)
            .fixedSize()
    }
}

private extension View {
    func shotSubmitOnReturn(
        enabled: Bool,
        action: @escaping () -> Void
    ) -> some View {
        onKeyPress(.return, phases: .down) { press in
            if press.modifiers.contains(.shift) {
                return .ignored
            }
            if enabled {
                action()
            }
            return .handled
        }
    }
}

private struct ExecutionDetailsView: View {
    let receipt: ExecutionReceipt?
    let balance: ManagedBalance?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Group {
                if let receipt {
                    Form {
                        Section("Request") {
                            LabeledContent("Exact intention", value: receipt.intention ?? "Unavailable")
                            LabeledContent("References", value: String(receipt.referenceCount))
                        }
                        Section("Intelligence") {
                            LabeledContent("Harness", value: receipt.harness)
                            LabeledContent("Model", value: receipt.model)
                            LabeledContent("Route", value: receipt.route)
                            if let tokens = receipt.totalTokens { LabeledContent("Recorded tokens", value: String(tokens)) }
                            if let cost = receipt.additionalCostUSD { LabeledContent("Recorded additional charge", value: cost.formatted(.currency(code: "USD"))) }
                            let managedCharges = balance?.transactions.filter {
                                $0.relatedExecutionID == receipt.executionID && $0.entryType == "inference_charge"
                            } ?? []
                            if !managedCharges.isEmpty {
                                let total = -managedCharges.reduce(Int64(0)) { $0 + $1.amountMicrousd }
                                LabeledContent("Managed charge", value: (Double(total) / 1_000_000).formatted(.currency(code: "USD")))
                                if let provider = managedCharges.compactMap(\.relatedProviderID).first {
                                    LabeledContent("Provider request", value: provider)
                                }
                                if let model = managedCharges.compactMap(\.relatedModel).first {
                                    LabeledContent("Managed model", value: model)
                                }
                                if let privacy = managedCharges.compactMap(\.privacyTier).first {
                                    LabeledContent("Privacy tier", value: privacy)
                                }
                                if let reconciliation = managedCharges.compactMap(\.reconciliationStatus).first {
                                    LabeledContent("Reconciliation", value: reconciliation)
                                }
                            }
                        }
                        Section("Result") {
                            LabeledContent("Phase", value: receipt.phase)
                            if let duration = receipt.durationSeconds { LabeledContent("Duration", value: "\(duration) seconds") }
                            if let summary = receipt.diffSummary { Text(summary) }
                            ForEach(Array(receipt.refusals.enumerated()), id: \.offset) { _, refusal in
                                Text("\(refusal.check): \(refusal.evidence ?? refusal.status)")
                            }
                        }
                    }
                } else {
                    ContentUnavailableView("No execution details yet", systemImage: "doc.text.magnifyingglass")
                }
            }
            .padding()
            .frame(minWidth: 560, minHeight: 420)
            .navigationTitle("App Details")
            .toolbar { Button("Done") { dismiss() } }
        }
        .accessibilityIdentifier("execution.details")
    }
}
