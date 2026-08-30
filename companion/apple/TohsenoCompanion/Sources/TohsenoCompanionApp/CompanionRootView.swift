import SwiftUI
import TohsenoCompanionKit

/// The whole product: Your Apps → choose app → what should change → Evolve.
public struct CompanionRootView: View {
    @State private var model: CompanionModel

    public init(model: CompanionModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        GeometryReader { geometry in
            ZStack {
                CompanionBackground()
                switch model.screen {
                case .loading:
                    CompanionLoadingView()
                case .firstRun:
                    FirstRunView(model: model)
                case .entitlementDecision, .trialEnded, .apps, .create, .app:
                    CompanionNavigation(model: model)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
        }
        .tint(Tohseno.orange)
        .preferredColorScheme(.dark)
        .task { model.start() }
        .onOpenURL { url in
            Task { await model.handleIncomingURL(url) }
        }
        .sheet(
            isPresented: Binding(
                get: { model.pendingPublication != nil },
                set: { if !$0 { model.dismissPublication() } }
            )
        ) {
            if let request = model.pendingPublication {
                PublicationApprovalView(model: model, request: request)
            }
        }
        .sheet(item: $model.linkedPublicRelease) { app in
            PublicReleaseDetailView(model: model, app: app)
        }
    }
}

private struct PublicationApprovalView: View {
    @Bindable var model: CompanionModel
    let request: PublicationApprovalRequest

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 22) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("SHIP TO TOHSENO").font(.caption.weight(.bold)).tracking(1.5)
                        .foregroundStyle(Tohseno.orange)
                    Text("Ship \(request.appName)?").font(.largeTitle.weight(.bold))
                    Text("This exact source snapshot will become public and independently verifiable.")
                        .foregroundStyle(Tohseno.ash)
                }
                Group {
                    LabeledContent("Source", value: "\(request.sourceFileCount) files · \(ByteCountFormatter.string(fromByteCount: Int64(request.sourceByteLength), countStyle: .file))")
                    LabeledContent("Install", value: request.installAllowed ? "Allowed" : "Not allowed")
                    LabeledContent("Fork", value: request.forkAllowed ? "Allowed" : "Not allowed")
                    LabeledContent("Link", value: request.requestedRoute)
                    LabeledContent("Checkpoint", value: "#\(request.checkpointSequence)")
                }
                .font(.subheadline)
                Spacer()
                Button {
                    Task { await model.approvePublication() }
                } label: {
                    Text(model.busy ? "Approving…" : "Ship to Tohseno")
                        .frame(maxWidth: .infinity).padding(.vertical, 8)
                }
                .buttonStyle(.borderedProminent).disabled(model.busy)
                Button("Not now", role: .cancel) { model.dismissPublication() }
                    .frame(maxWidth: .infinity)
            }
            .padding(28)
            .presentationDetents([.medium, .large])
            .interactiveDismissDisabled(model.busy)
        }
    }
}

private enum CompanionRoute: Hashable {
    case app(String)
}

private enum CompanionTab: Hashable {
    case apps, registry, create, profile
}

private struct CompanionNavigation: View {
    @Bindable var model: CompanionModel
    @State private var selectedTab: CompanionTab = .apps

    var body: some View {
        TabView(selection: $selectedTab) {
            NavigationStack(path: appPath) {
                YourAppsView(model: model)
                    .companionRootNavigationBar()
                    .navigationDestination(for: CompanionRoute.self) { route in
                        switch route {
                        case let .app(shotID):
                            if let shot = model.app(shotID) {
                                AppView(model: model, shot: shot)
                                    .companionDestinationNavigationBar()
                            }
                        }
                    }
            }
            .tabItem { Label("Apps", systemImage: "square.grid.2x2") }
            .tag(CompanionTab.apps)

            PublicRegistryView(model: model)
                .tabItem { Label("Registry", systemImage: "globe.americas.fill") }
                .tag(CompanionTab.registry)

            NavigationStack { CreateAppView(model: model).companionDestinationNavigationBar() }
                .tabItem { Label("Create", systemImage: "plus.circle.fill") }
                .tag(CompanionTab.create)

            BuilderProfileView(model: model)
                .tabItem { Label("Profile", systemImage: "person.crop.circle") }
                .tag(CompanionTab.profile)
        }
        .onChange(of: selectedTab) { _, tab in
            if tab == .create { model.openCreate() }
            else if tab == .apps, model.screen == .create { model.openApps() }
        }
    }

    private var appPath: Binding<[CompanionRoute]> {
        Binding(
            get: {
                switch model.screen {
                case let .app(shotID): [.app(shotID)]
                default: []
                }
            },
            set: { routes in
                guard let route = routes.last else {
                    if model.screen != .apps { model.openApps() }
                    return
                }
                switch route {
                case let .app(shotID):
                    if model.screen != .app(shotID), let shot = model.app(shotID) {
                        model.open(shot)
                    }
                }
            }
        )
    }
}

private struct PublicRegistryView: View {
    @Bindable var model: CompanionModel

    var body: some View {
        NavigationStack {
            Group {
                if model.publicApps.isEmpty {
                    ContentUnavailableView(
                        "A network made by people",
                        systemImage: "point.3.connected.trianglepath.dotted",
                        description: Text(model.networkNotice ?? "The first verified public app will appear here.")
                    )
                } else {
                    List(model.publicApps) { app in
                        NavigationLink {
                            PublicReleaseDetailView(model: model, app: app)
                        } label: {
                            VStack(alignment: .leading, spacing: 6) {
                                Text(app.release.display.name).font(.headline)
                                Text(app.release.display.description)
                                    .font(.subheadline).foregroundStyle(Tohseno.ash).lineLimit(3)
                                HStack {
                                    Label("Signed", systemImage: "signature")
                                    if app.release.permissions.forkAllowed {
                                        Label("Forkable", systemImage: "arrow.triangle.branch")
                                    }
                                }
                                .font(.caption).foregroundStyle(Tohseno.orange)
                            }
                            .padding(.vertical, 6)
                        }
                    }
                    .refreshable { await model.refreshPublicNetwork() }
                }
            }
            .navigationTitle("Registry")
        }
    }
}

private struct PublicReleaseDetailView: View {
    @Bindable var model: CompanionModel
    let app: PublicAppRelease

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Text(app.release.display.name).font(.largeTitle.weight(.bold))
                    Text(app.release.display.description).foregroundStyle(Tohseno.ash)
                }
                Section("Signed release") {
                    LabeledContent("Builder", value: "…\(app.release.builderID.suffix(18))")
                    LabeledContent("Checkpoint", value: "#\(app.release.checkpointSequence)")
                    LabeledContent("Source", value: "Available")
                    LabeledContent("Fork", value: app.release.permissions.forkAllowed ? "Allowed" : "Not allowed")
                    Text("Your Mac verifies the exact release and fresh chain state before Install or Fork.")
                        .font(.caption).foregroundStyle(Tohseno.ash)
                }
                Section {
                    Button {
                        Task { await model.requestNetworkRelease(app, action: .install) }
                    } label: {
                        Label(model.busy ? "Queuing…" : "Install", systemImage: "iphone.and.arrow.forward")
                    }
                    .disabled(model.busy)
                    if app.release.permissions.forkAllowed {
                        Button {
                            Task { await model.requestNetworkRelease(app, action: .fork) }
                        } label: {
                            Label("Fork", systemImage: "arrow.triangle.branch")
                        }
                        .disabled(model.busy)
                    }
                }
                if let notice = model.networkNotice {
                    Section("Status") { Text(notice).foregroundStyle(Tohseno.ash) }
                }
            }
            .navigationTitle("Registry")
        }
    }
}

private struct BuilderProfileView: View {
    @Bindable var model: CompanionModel

    var body: some View {
        NavigationStack {
            List {
                Section("Builder identity") {
                    if let identity = model.builderDevice {
                        Label("Protected by this iPhone", systemImage: "iphone.gen3.badge.play")
                        LabeledContent("DeviceKey", value: compact(identity.keyID))
                        LabeledContent("Security", value: identity.securityLevel == "secure_enclave" ? "Secure Enclave" : "Test only")
                    } else {
                        Label("Secure Enclave identity unavailable", systemImage: "exclamationmark.shield")
                    }
                }
                Section("Public work") {
                    LabeledContent("Published apps", value: "\(model.publicApps.count)")
                    Text("Your BuilderID becomes public only when you explicitly approve Ship to Tohseno.")
                        .font(.caption).foregroundStyle(Tohseno.ash)
                }
                Section("Public profile") {
                    TextField("Display name", text: $model.profileDisplayName)
                    TextField("Builder handle", text: $model.profileHandle)
                    Button {
                        Task { await model.savePublicProfile() }
                    } label: {
                        Label(model.busy ? "Signing…" : "Sign & Update Profile", systemImage: "person.crop.circle.badge.checkmark")
                    }
                    .disabled(model.busy || model.profileDisplayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    Text("Profile changes are signed by this iPhone and checked against the current BuilderAccount.")
                        .font(.caption).foregroundStyle(Tohseno.ash)
                }
                Section("Global alias request") {
                    TextField("Alias", text: $model.requestedAlias)
                    Button {
                        Task { await model.requestGlobalAlias() }
                    } label: {
                        Label("Sign Alias Request", systemImage: "link.badge.plus")
                    }
                    .disabled(model.busy || model.requestedAlias.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    Text("Aliases are convenience routes, not Shot identity. Every request is rate-limited and requires explicit policy review.")
                        .font(.caption).foregroundStyle(Tohseno.ash)
                }
                if let status = model.profileNotice {
                    Section("Profile status") { Text(status).foregroundStyle(Tohseno.ash) }
                }
                Section("Private connection") {
                    Label(model.connection == .connected ? "Mac connected" : "Mac currently offline",
                          systemImage: model.connection == .connected ? "checkmark.circle.fill" : "desktopcomputer")
                    Text("Pairing capabilities and private intentions never enter your public profile.")
                        .font(.caption).foregroundStyle(Tohseno.ash)
                }
            }
            .navigationTitle("Profile")
        }
    }

    private func compact(_ value: String) -> String {
        value.count > 22 ? "\(value.prefix(12))…\(value.suffix(8))" : value
    }
}

private extension View {
    @ViewBuilder
    func companionRootNavigationBar() -> some View {
#if os(iOS)
        toolbar(.hidden, for: .navigationBar)
#else
        self
#endif
    }

    @ViewBuilder
    func companionDestinationNavigationBar() -> some View {
#if os(iOS)
        toolbar(.visible, for: .navigationBar)
#else
        self
#endif
    }

    @ViewBuilder
    func companionInlineNavigationTitle(_ title: String) -> some View {
#if os(iOS)
        navigationTitle(title).navigationBarTitleDisplayMode(.inline)
#else
        navigationTitle(title)
#endif
    }
}

private struct CompanionBackground: View {
    var body: some View {
        ZStack {
            Tohseno.void
            RadialGradient(
                colors: [Color.white.opacity(0.035), .clear],
                center: .topTrailing,
                startRadius: 0,
                endRadius: 520
            )
            LinearGradient(
                colors: [.clear, Tohseno.orange.opacity(0.025), .clear],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        }
        .ignoresSafeArea()
    }
}

private struct CompanionLoadingView: View {
    var body: some View {
        VStack(spacing: 22) {
            TohsenoMark(size: 92)
            ProgressView()
                .tint(Tohseno.orange)
            Text("Loading your apps…")
                .font(.system(size: 15))
                .foregroundStyle(Tohseno.ash)
        }
        .accessibilityElement(children: .combine)
    }
}

struct YourAppsView: View {
    @Bindable var model: CompanionModel

    private let columns = [
        GridItem(.adaptive(minimum: 82, maximum: 104), spacing: 12, alignment: .top)
    ]

    var body: some View {
        VStack(spacing: 0) {
            CompanionHeader(connection: model.connection, syncing: model.syncing) {
                await model.syncNow()
            }

            if model.apps.isEmpty {
                Spacer()
                VStack(spacing: 12) {
                    Image(systemName: "square.grid.2x2")
                        .font(.system(size: 30, weight: .light))
                        .foregroundStyle(Tohseno.ash)
                    Text("Adopt an existing iPhone app in Tohseno on your Mac. Once connected, it appears here ready for change requests.")
                        .font(.system(size: 16))
                        .foregroundStyle(Tohseno.ash)
                        .multilineTextAlignment(.center)
                }
                .padding(.horizontal, 40)
                Spacer()
            } else {
                ScrollView {
                    LazyVGrid(columns: columns, alignment: .center, spacing: 24) {
                        ForEach(model.apps, id: \.shotID) { shot in
                            Button { model.open(shot) } label: {
                                AppTile(
                                    shot: shot,
                                    icon: model.icon(for: shot),
                                    presentation: model.presentation(for: shot)
                                )
                            }
                            .buttonStyle(AppRowButtonStyle())
                        }
                    }
                    .padding(.horizontal, 20)
                    .padding(.top, 16)
                    .padding(.bottom, 28)
                }
                .refreshable { await model.syncNow() }
            }

            if let notice = model.notice {
                NoticeView(text: notice)
                    .padding(.horizontal, 20)
                    .padding(.bottom, 12)
            }
        }
    }
}

private struct CompanionHeader: View {
    let connection: CompanionConnectionState
    let syncing: Bool
    let onSync: () async -> Void

    var body: some View {
        HStack(spacing: 12) {
            WordmarkView()
            Spacer()
            Image(systemName: "desktopcomputer")
                .font(.system(size: 14, weight: .medium))
            Text(connectionText)
                .font(.system(size: 12, weight: .medium))
            Button {
                Task { await onSync() }
            } label: {
                Group {
                    if syncing {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Image(systemName: "arrow.clockwise")
                            .font(.system(size: 12, weight: .semibold))
                    }
                }
                .frame(width: 28, height: 28)
                .background(Tohseno.carbon, in: Circle())
                .overlay(Circle().strokeBorder(Tohseno.iron))
            }
            .buttonStyle(.plain)
            .disabled(syncing)
            .accessibilityLabel(syncing ? "Syncing with Mac" : "Sync with Mac")
        }
        .foregroundStyle(connectionColor)
        .padding(.horizontal, 24)
        .padding(.top, 14)
        .padding(.bottom, 6)
    }

    private var connectionText: String {
        switch connection {
        case .connected: "Mac connected"
        case .pairing: "Connecting…"
        case .reconnecting: "Reconnecting…"
        case .disconnected: "Mac offline"
        case .revoked: "Access removed"
        }
    }

    private var connectionColor: Color {
        switch connection {
        case .connected: Tohseno.connected
        case .pairing, .reconnecting: Tohseno.warning
        case .disconnected: Tohseno.ash
        case .revoked: Tohseno.failed
        }
    }
}

struct AppTile: View {
    let shot: ShotSummary
    let icon: Data?
    let presentation: TohsenoPresentation

    var body: some View {
        VStack(spacing: 8) {
            IconView(name: shot.displayName, bytes: icon, size: 64)
                .overlay(alignment: .bottomTrailing) {
                    Circle()
                        .fill(statusColor)
                        .frame(width: 12, height: 12)
                        .overlay(Circle().strokeBorder(Tohseno.void, lineWidth: 2))
                        .accessibilityLabel(status)
                }
            Text(shot.displayName)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Tohseno.bone)
                .lineLimit(1)
                .minimumScaleFactor(0.76)
                .frame(maxWidth: .infinity)
            Text("This Mac · \(status)")
                .font(.system(size: 10))
                .foregroundStyle(Tohseno.ash)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity)
        .contentShape(Rectangle())
    }

    private var statusColor: Color {
        switch presentation.state {
        case .installed, .readyForPhone: Tohseno.connected
        case .waiting, .building, .installing: Tohseno.warning
        case .failed: Tohseno.failed
        }
    }

    private var status: String {
        switch presentation.state {
        case .waiting: "Waiting"
        case .building: "Building"
        case .readyForPhone: "Ready to install"
        case .installing: "Installing"
        case .installed: "Installed"
        case .failed: "Failed"
        }
    }
}

private struct AppRowButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.94 : 1)
            .opacity(configuration.isPressed ? 0.82 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

struct IconView: View {
    let name: String
    let bytes: Data?
    var size: CGFloat = 64

    var body: some View {
        Group {
#if canImport(UIKit)
            if let bytes, let image = UIImage(data: bytes) {
                Image(uiImage: image).resizable()
            } else {
                placeholder
            }
#else
            placeholder
#endif
        }
        .scaledToFill()
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: size * 0.22, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: size * 0.22, style: .continuous)
                .strokeBorder(Color.white.opacity(0.1), lineWidth: 0.5)
        )
        .shadow(color: .black.opacity(0.36), radius: 7, y: 4)
    }

    private var placeholder: some View {
        RoundedRectangle(cornerRadius: size * 0.22, style: .continuous)
            .fill(
                LinearGradient(
                    colors: [Tohseno.iron, Tohseno.carbon],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
            )
            .overlay(
                Text(String(name.prefix(1)).uppercased())
                    .font(.system(size: size * 0.36, weight: .semibold))
                    .foregroundStyle(Tohseno.orange)
            )
    }
}

struct AppView: View {
    @Bindable var model: CompanionModel
    let shot: ShotSummary

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                let presentation = model.presentation(for: shot)
                StateView(presentation: presentation)

                composer

                if let history = shot.recentEvolutions, !history.isEmpty {
                    EvolutionHistoryView(history: history)
                }

                if let notice = model.notice {
                    NoticeView(text: notice)
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 18)
            .padding(.bottom, 36)
        }
        .companionInlineNavigationTitle(shot.displayName)
        .scrollDismissesKeyboard(.interactively)
        .refreshable { await model.syncNow() }
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("What should become different?")
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(Tohseno.bone)

            IntentEditor(
                text: $model.intent,
                placeholder: "Describe the change you want…",
                minimumHeight: 170
            )

            ScreenshotPicker(attachments: $model.attachments)

            Text("Opening this app never starts a build. Evolve App sends one request.")
                .font(.system(size: 12))
                .foregroundStyle(Tohseno.ash)

            Button {
                Task { await model.evolve() }
            } label: {
                if model.busy {
                    ProgressView()
                        .tint(Tohseno.void)
                } else {
                    Text("Evolve App")
                }
            }
            .buttonStyle(PrimaryButtonStyle(enabled: model.canEvolve))
            .disabled(!model.canEvolve)
            .padding(.top, 6)
        }
    }
}

private struct EvolutionHistoryView: View {
    let history: [EvolutionHistorySummary]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Previous changes")
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
            ForEach(history) { evolution in
                VStack(alignment: .leading, spacing: 5) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(evolution.requestSummary)
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(Tohseno.bone)
                            .lineLimit(3)
                        Spacer()
                        Text(evolution.status.replacingOccurrences(of: "_", with: " ").capitalized)
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(Tohseno.ash)
                    }
                    if let completion = evolution.completionSummary {
                        Text(completion)
                            .font(.system(size: 12))
                            .foregroundStyle(Tohseno.ash)
                    }
                    if let installation = evolution.installationSummary {
                        Text(installation)
                            .font(.system(size: 12))
                            .foregroundStyle(Tohseno.ash)
                    }
                }
                .padding(12)
                .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Tohseno.iron))
            }
        }
    }
}

private struct CreateAppView: View {
    @Bindable var model: CompanionModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                nameField
                    .font(.system(size: 18, weight: .semibold))
                    .padding(14)
                    .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 14))
                    .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Tohseno.iron))

                Text("What do you want this app to be?")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(Tohseno.bone)

                IntentEditor(
                    text: $model.intent,
                    placeholder: "Describe the app you want…",
                    minimumHeight: 190
                )

                ScreenshotPicker(attachments: $model.attachments)

                if let notice = model.notice { NoticeView(text: notice) }

                Button {
                    Task { await model.create() }
                } label: {
                    if model.busy { ProgressView().tint(Tohseno.void) }
                    else { Text("Create App") }
                }
                .buttonStyle(PrimaryButtonStyle(enabled: model.canCreate))
                .disabled(!model.canCreate)
            }
            .padding(20)
            .padding(.bottom, 24)
        }
        .companionInlineNavigationTitle("New App")
        .scrollDismissesKeyboard(.interactively)
    }

    @ViewBuilder
    private var nameField: some View {
#if os(iOS)
        TextField("app-name", text: $model.appName)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
#else
        TextField("app-name", text: $model.appName)
#endif
    }
}

struct StateView: View {
    let presentation: TohsenoPresentation

    var body: some View {
        HStack(spacing: 12) {
            if presentation.state.inFlight {
                ProgressView()
                    .tint(Tohseno.orange)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text(presentation.headline)
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(Tohseno.bone)
                if let detail = presentation.detail {
                    Text(detail)
                        .font(.system(size: 14))
                        .foregroundStyle(Tohseno.ash)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

struct NoticeView: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 15))
            .foregroundStyle(Tohseno.bone)
            .fixedSize(horizontal: false, vertical: true)
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Tohseno.iron))
    }
}
