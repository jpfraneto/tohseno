import SwiftUI
import UniformTypeIdentifiers

enum WorkshopMotion {
    static func ambient(reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeInOut(duration: 3.2).repeatForever(autoreverses: true)
    }

    static func activity(reduceMotion: Bool, active: Bool) -> Animation? {
        reduceMotion || !active
            ? nil
            : .easeInOut(duration: 1.1).repeatForever(autoreverses: true)
    }
}

public enum WorkshopChapter: String, CaseIterable, Sendable {
    case bringIPhone = "bring_iphone"
    case takeShot = "take_shot"
    case building
    case readyToInstall = "ready_to_install"
    case installing
    case installed
    case needsAttention = "needs_attention"

    public var title: String {
        switch self {
        case .bringIPhone: "Bring your iPhone into the workshop"
        case .takeShot: "Take one clear Shot"
        case .building: "The Mac factory is building"
        case .readyToInstall: "The app is ready for its iPhone"
        case .installing: "The app is moving to the iPhone"
        case .installed: "The workshop is connected"
        case .needsAttention: "One app needs your attention"
        }
    }

    public var symbol: String {
        switch self {
        case .bringIPhone: "iphone.and.arrow.forward"
        case .takeShot: "scope"
        case .building: "hammer.fill"
        case .readyToInstall: "iphone.gen3"
        case .installing: "arrow.right"
        case .installed: "checkmark.circle.fill"
        case .needsAttention: "exclamationmark.triangle.fill"
        }
    }
}

public enum WorkshopConnection: String, Sendable {
    case unknown
    case nearby
    case connected
    case attention
}

public enum WorkshopThreshold: String, Sendable {
    case unknown
    case privateOnly = "private_only"
    case witnessed
    case publishingAvailable = "publishing_available"
}

public struct WorkshopAppObject: Equatable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let state: PresentedState
    public let headline: String
}

/// A pure projection of facts already held by the local service. It adds no
/// lifecycle or protocol state; the scene and accessible list consume the same
/// value so decorative layout cannot invent a different result.
public struct LivingWorkshopProjection: Equatable, Sendable {
    public let chapter: WorkshopChapter
    public let apps: [WorkshopAppObject]
    public let phoneName: String?
    public let phone: WorkshopConnection
    public let keeper: WorkshopConnection
    public let threshold: WorkshopThreshold
    public let unreadUpdates: Int
    public let networkDetail: String

    public init(
        apps: [AppSummary],
        readiness: ReadinessView?,
        pairedDevices: [PairedCompanionDevice],
        registry: RegistrySnapshot?
    ) {
        self.apps = apps.map {
            WorkshopAppObject(
                id: $0.id,
                name: $0.displayName,
                state: $0.presentation.state,
                headline: $0.presentation.headline
            )
        }
        phoneName = [readiness?.deviceName, readiness?.deviceProductType]
            .compactMap { $0 }
            .uniqued()
            .joined(separator: " · ")
            .nilIfEmpty
        if readiness?.companionInstallState == "failed" {
            phone = .attention
        } else if readiness?.companionConnected == true {
            phone = .connected
        } else if phoneName != nil {
            phone = .nearby
        } else {
            phone = .unknown
        }
        let activeKeepers = pairedDevices.filter { !$0.revoked }
        if readiness?.companionConnected == true {
            keeper = .connected
        } else if !activeKeepers.isEmpty {
            keeper = .nearby
        } else {
            keeper = .unknown
        }
        if let registry {
            if registry.network.publishingAvailable {
                threshold = .publishingAvailable
            } else if registry.network.ready && registry.network.publicAuthorityAvailable {
                threshold = .witnessed
            } else {
                threshold = .privateOnly
            }
            unreadUpdates = registry.privateUpdates.filter { $0.readAt == nil }.count
            networkDetail = registry.network.reason
        } else {
            threshold = .unknown
            unreadUpdates = 0
            networkDetail = "Public evidence has not been checked yet."
        }
        chapter = Self.chapter(apps: apps, readiness: readiness)
    }

    private static func chapter(apps: [AppSummary], readiness: ReadinessView?) -> WorkshopChapter {
        if let readiness, !readiness.ready { return .bringIPhone }
        if apps.isEmpty { return .takeShot }
        if apps.contains(where: { $0.presentation.state == .failed }) { return .needsAttention }
        if apps.contains(where: { $0.presentation.state == .installing }) { return .installing }
        if apps.contains(where: { $0.presentation.state == .readyForPhone }) { return .readyToInstall }
        if apps.contains(where: { $0.presentation.state == .building || $0.presentation.state == .waiting }) {
            return .building
        }
        return .installed
    }
}

struct WorkshopDestinationBar: View {
    @Bindable var model: TohsenoAppModel

    var body: some View {
        HStack(spacing: 12) {
            Button {
                model.route = .library
            } label: {
                HStack(spacing: 9) {
                    TohsenoMark().frame(width: 25, height: 25)
                    Text("Workshop").font(.headline)
                }
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("workshop.return")

            Divider().frame(height: 22)

            Text(destinationTitle)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(TohsenoTheme.silver)
                .lineLimit(1)

            Spacer()

            Menu {
                ForEach(model.apps) { app in
                    Button(app.displayName) { model.route = .app(app.id) }
                }
                if model.apps.isEmpty { Text("No apps yet") }
            } label: {
                Label("App shelf", systemImage: "square.grid.2x2")
            }
            .accessibilityIdentifier("workshop.app-shelf-menu")

            Button("One Shot") { model.route = .library }
                .accessibilityIdentifier("create-app.workshop")
            Button("Network") { model.route = .registry }
                .accessibilityIdentifier("registry.workshop")
            Button("Keeper") { model.route = .profile }
                .accessibilityIdentifier("profile.workshop")
            SettingsLink { Image(systemName: "gearshape") }
                .accessibilityLabel("Workshop settings")
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 18)
        .frame(height: 48)
        .background(TohsenoTheme.carbon)
    }

    private var destinationTitle: String {
        switch model.route {
        case .library: "Living workshop"
        case .registry: "Network threshold"
        case .profile: "Keeper and authority"
        case .create: "One Shot options"
        case .app: model.selectedApp?.displayName ?? "App workbench"
        }
    }
}

struct LivingWorkshopView: View {
    @Bindable var model: TohsenoAppModel
    let adopt: () -> Void
    @State private var selectedIndex = 0
    @State private var showingPalette = false
    @State private var showingList = false
    @State private var choosingReferences = false
    @FocusState private var intentionFocused: Bool

    private var projection: LivingWorkshopProjection {
        LivingWorkshopProjection(
            apps: model.apps,
            readiness: model.readiness,
            pairedDevices: model.pairedCompanionDevices,
            registry: model.registrySnapshot
        )
    }

    private var canSubmit: Bool {
        !model.isSubmitting
            && !model.creation.intention.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        ZStack {
            WorkshopField()
            VStack(spacing: 0) {
                workshopHeader
                ScrollView {
                    VStack(spacing: 18) {
                        WorkshopStoryStage(
                            projection: projection,
                            selectedID: selectedAppID,
                            selectApp: openApp,
                            openNetwork: { model.route = .registry },
                            openKeeper: { model.route = .profile }
                        )
                        .frame(maxWidth: 1_080)

                        appShelf
                            .frame(maxWidth: 1_080)
                    }
                    .padding(.horizontal, 28)
                    .padding(.vertical, 20)
                }
                OneShotDock(
                    model: model,
                    choosingReferences: $choosingReferences,
                    intentionFocused: $intentionFocused,
                    canSubmit: canSubmit,
                    adopt: adopt
                )
            }

            if showingPalette {
                WorkshopPalette(
                    model: model,
                    close: { showingPalette = false },
                    adopt: adopt,
                    showList: { showingPalette = false; showingList = true },
                    focusShot: { showingPalette = false; intentionFocused = true }
                )
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .background(TohsenoTheme.void)
        .foregroundStyle(TohsenoTheme.bone)
        .fileImporter(
            isPresented: $choosingReferences,
            allowedContentTypes: [.png, .jpeg],
            allowsMultipleSelection: true
        ) { model.addReferences($0, to: .creation) }
        .dropDestination(for: URL.self) { urls, _ in
            model.addReferences(.success(urls), to: .creation)
            return !urls.isEmpty
        }
        .sheet(isPresented: $showingList) {
            WorkshopListFallback(model: model, isPresented: $showingList)
        }
        .onMoveCommand(perform: moveSelection)
        .onKeyPress("/", phases: .down) { _ in
            guard !intentionFocused else { return .ignored }
            withAnimation(.easeOut(duration: 0.16)) { showingPalette = true }
            return .handled
        }
        .onKeyPress(.return, phases: .down) { press in
            guard !intentionFocused, !press.modifiers.contains(.shift), let selectedAppID else {
                return .ignored
            }
            model.route = .app(selectedAppID)
            return .handled
        }
        .onExitCommand {
            if showingPalette { showingPalette = false }
            else if showingList { showingList = false }
        }
        .task {
            if model.registrySnapshot == nil { await model.refreshRegistry() }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Living software workshop. \(projection.chapter.title).")
        .accessibilityIdentifier("workshop.scene")
    }

    private var selectedAppID: String? {
        guard projection.apps.indices.contains(selectedIndex) else { return projection.apps.first?.id }
        return projection.apps[selectedIndex].id
    }

    private var workshopHeader: some View {
        HStack(spacing: 14) {
            TohsenoLivingMark(size: 30)
            VStack(alignment: .leading, spacing: 2) {
                Text("TOHSENO · ONE SHOT")
                    .font(.caption2.weight(.semibold))
                    .tracking(2.2)
                    .foregroundStyle(TohsenoTheme.amber)
                Text(projection.chapter.title)
                    .font(.title3.weight(.semibold))
            }
            Spacer()
            Button { showingList = true } label: {
                Label("List", systemImage: "list.bullet")
            }
            .help("Accessible app list fallback")
            .accessibilityIdentifier("workshop.list")
            Button { showingPalette = true } label: {
                Label("Commands", systemImage: "command")
            }
            .help("Open command palette (/) ")
            .accessibilityIdentifier("workshop.palette")
            SettingsLink { Image(systemName: "gearshape") }
                .accessibilityLabel("Workshop settings")
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 24)
        .frame(height: 64)
        .background(TohsenoTheme.carbon.opacity(0.94))
        .overlay(alignment: .bottom) { Rectangle().fill(TohsenoTheme.iron).frame(height: 1) }
    }

    private var appShelf: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("App shelf", systemImage: "square.grid.2x2")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(TohsenoTheme.silver)
                Spacer()
                Text("Arrow keys choose · Return opens")
                    .font(.caption2)
                    .foregroundStyle(TohsenoTheme.ash)
            }
            if projection.apps.isEmpty {
                Button {
                    intentionFocused = true
                } label: {
                    Label("Your first app will take shape here", systemImage: "sparkles.rectangle.stack")
                        .frame(maxWidth: .infinity, minHeight: 54)
                }
                .buttonStyle(.plain)
                .foregroundStyle(TohsenoTheme.silver)
                .accessibilityIdentifier("workshop.empty-shelf")
            } else {
                ScrollView(.horizontal) {
                    HStack(spacing: 10) {
                        ForEach(Array(projection.apps.enumerated()), id: \.element.id) { index, app in
                            WorkshopShelfObject(app: app, selected: index == selectedIndex) {
                                selectedIndex = index
                                openApp(app.id)
                            }
                            .accessibilityIdentifier("app.\(app.id)")
                        }
                    }
                }
                .scrollIndicators(.hidden)
            }
        }
        .padding(14)
        .background(TohsenoTheme.carbon.opacity(0.88))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(TohsenoTheme.iron))
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .accessibilityIdentifier("workshop.app-shelf")
    }

    private func openApp(_ id: String) {
        model.route = .app(id)
    }

    private func moveSelection(_ direction: MoveCommandDirection) {
        guard !intentionFocused, !projection.apps.isEmpty else { return }
        switch direction {
        case .left, .up:
            selectedIndex = max(0, selectedIndex - 1)
        case .right, .down:
            selectedIndex = min(projection.apps.count - 1, selectedIndex + 1)
        default:
            break
        }
    }
}

private struct WorkshopStoryStage: View {
    let projection: LivingWorkshopProjection
    let selectedID: String?
    let selectApp: (String) -> Void
    let openNetwork: () -> Void
    let openKeeper: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var breathes = false

    var body: some View {
        VStack(spacing: 18) {
            HStack(alignment: .center, spacing: 12) {
                WorkshopActor(
                    title: "Mac factory",
                    detail: "Source · harness · Xcode",
                    symbol: "macbook",
                    state: .connected
                )
                WorkshopFlowLine(active: projection.chapter == .building, reduceMotion: reduceMotion)
                centerBench
                WorkshopFlowLine(
                    active: [.readyToInstall, .installing].contains(projection.chapter),
                    reduceMotion: reduceMotion
                )
                WorkshopActor(
                    title: projection.phoneName ?? "Intended iPhone",
                    detail: phoneDetail,
                    symbol: "iphone.gen3",
                    state: projection.phone
                )
            }

            HStack(spacing: 12) {
                Button(action: openKeeper) {
                    WorkshopActor(
                        title: "Keeper",
                        detail: keeperDetail,
                        symbol: "hand.raised.fill",
                        state: projection.keeper,
                        compact: true
                    )
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Keeper. \(keeperDetail)")

                TohsenoKeeperActor(chapter: projection.chapter)

                Spacer(minLength: 10)

                Button(action: openNetwork) {
                    WorkshopThresholdView(
                        threshold: projection.threshold,
                        updates: projection.unreadUpdates,
                        detail: projection.networkDetail
                    )
                }
                .buttonStyle(.plain)
            }
        }
        .padding(22)
        .background(
            LinearGradient(
                colors: [TohsenoTheme.ember.opacity(0.42), TohsenoTheme.carbon.opacity(0.94)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
        .overlay(RoundedRectangle(cornerRadius: 22).stroke(TohsenoTheme.amber.opacity(0.16)))
        .clipShape(RoundedRectangle(cornerRadius: 22))
        .shadow(color: TohsenoTheme.amber.opacity(0.06), radius: 24)
        .scaleEffect(breathes && !reduceMotion ? 1.002 : 1)
        .animation(WorkshopMotion.ambient(reduceMotion: reduceMotion), value: breathes)
        .onAppear { breathes = true }
    }

    private var centerBench: some View {
        VStack(spacing: 9) {
            Image(systemName: projection.chapter.symbol)
                .font(.system(size: 30, weight: .medium))
                .foregroundStyle(projection.chapter == .needsAttention ? Color.red : TohsenoTheme.amber)
                .frame(width: 58, height: 58)
                .background(TohsenoTheme.void.opacity(0.72))
                .clipShape(Circle())
                .overlay(Circle().stroke(TohsenoTheme.amber.opacity(0.28)))
            Text(projection.chapter.title)
                .font(.headline)
                .multilineTextAlignment(.center)
            Text(chapterDetail)
                .font(.caption)
                .foregroundStyle(TohsenoTheme.silver)
                .multilineTextAlignment(.center)
                .lineLimit(3)
        }
        .frame(minWidth: 190, idealWidth: 240, maxWidth: 280, minHeight: 145)
        .padding(14)
        .background(TohsenoTheme.graphite.opacity(0.8))
        .clipShape(RoundedRectangle(cornerRadius: 18))
        .accessibilityElement(children: .combine)
    }

    private var phoneDetail: String {
        switch projection.phone {
        case .connected: "Observed and privately connected"
        case .nearby: "Known; waiting for connection"
        case .attention: "Setup stopped safely"
        case .unknown: "Not currently observed"
        }
    }

    private var keeperDetail: String {
        switch projection.keeper {
        case .connected: "Human authority is connected"
        case .nearby: "Paired; currently away"
        case .attention: "Authority needs attention"
        case .unknown: "No paired authority observed"
        }
    }

    private var chapterDetail: String {
        switch projection.chapter {
        case .bringIPhone: "Connect the phone that will actually receive the app."
        case .takeShot: "One intention enters the one existing factory."
        case .building: "Real source and build state are changing on this Mac."
        case .readyToInstall: "The verified build is retained until the intended phone is reachable."
        case .installing: "Installation is not complete until the exact app is observed on the phone."
        case .installed: "Choose an app to change it, or take another Shot."
        case .needsAttention: "Open the app object for the smallest truthful recovery action."
        }
    }
}

/// Tohseno is an inhabitant of the room, not a source of state. Posture and
/// gesture are selected exclusively from the same real chapter rendered by the
/// workbench, so the character cannot celebrate ahead of evidence.
private struct TohsenoKeeperActor: View {
    let chapter: WorkshopChapter
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var moving = false

    var body: some View {
        content
            .padding(10)
            .frame(minWidth: 190, maxWidth: 230, minHeight: 72, alignment: .leading)
            .background {
                RoundedRectangle(cornerRadius: 14).fill(TohsenoTheme.void.opacity(0.72))
            }
            .overlay {
                RoundedRectangle(cornerRadius: 14).stroke(TohsenoTheme.amber.opacity(0.28))
            }
            .animation(
                WorkshopMotion.activity(reduceMotion: reduceMotion, active: isWorking),
                value: moving
            )
            .onAppear { moving = isWorking }
            .onChange(of: chapter) { _, _ in moving = isWorking }
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Tohseno, workshop keeper. \(line)")
            .accessibilityIdentifier("workshop.tohseno-keeper")
    }

    private var content: some View {
        HStack(spacing: 10) {
            keeperMark
            VStack(alignment: .leading, spacing: 2) {
                Text("Tohseno")
                    .font(.callout.weight(.semibold))
                Text(line)
                    .font(.caption2)
                    .foregroundStyle(TohsenoTheme.silver)
                    .lineLimit(2)
            }
        }
    }

    private var keeperMark: some View {
        ZStack(alignment: .bottomTrailing) {
            TohsenoLivingMark(size: 42)
                .offset(y: moving && !reduceMotion ? -2 : 1)
            Image(systemName: gesture)
                .font(.caption2.weight(.bold))
                .foregroundStyle(TohsenoTheme.void)
                .frame(width: 20, height: 20)
                .background(TohsenoTheme.amber, in: Circle())
        }
    }

    private var isWorking: Bool { chapter == .building || chapter == .installing }

    private var line: String {
        switch chapter {
        case .bringIPhone: "Bring your iPhone in."
        case .takeShot: "Ready when you are."
        case .building: "Working at the bench."
        case .readyToInstall: "Waiting beside the dock."
        case .installing: "Watching the real handoff."
        case .installed: "Verified on your iPhone."
        case .needsAttention: "Open the app to recover."
        }
    }

    private var gesture: String {
        switch chapter {
        case .bringIPhone: "hand.wave.fill"
        case .takeShot: "scope"
        case .building: "hammer.fill"
        case .readyToInstall: "arrow.down.to.line.compact"
        case .installing: "eye.fill"
        case .installed: "checkmark"
        case .needsAttention: "exclamationmark"
        }
    }
}

private struct WorkshopActor: View {
    let title: String
    let detail: String
    let symbol: String
    let state: WorkshopConnection
    var compact = false

    var body: some View {
        VStack(spacing: compact ? 6 : 10) {
            Image(systemName: symbol)
                .font(compact ? .title3 : .system(size: 32, weight: .light))
                .foregroundStyle(color)
            Text(title)
                .font((compact ? Font.callout : Font.headline).weight(.semibold))
                .lineLimit(1)
            Text(detail)
                .font(.caption2)
                .foregroundStyle(TohsenoTheme.silver)
                .multilineTextAlignment(.center)
                .lineLimit(2)
        }
        .padding(compact ? 10 : 14)
        .frame(
            minWidth: compact ? 170 : 155,
            idealWidth: compact ? 210 : 180,
            maxWidth: compact ? 230 : 205,
            minHeight: compact ? 72 : 145
        )
        .background(TohsenoTheme.void.opacity(0.7))
        .overlay(RoundedRectangle(cornerRadius: 17).stroke(color.opacity(0.3)))
        .clipShape(RoundedRectangle(cornerRadius: 17))
        .accessibilityElement(children: .combine)
    }

    private var color: Color {
        switch state {
        case .connected: TohsenoTheme.amber
        case .nearby: TohsenoTheme.silver
        case .attention: .red
        case .unknown: TohsenoTheme.ash
        }
    }
}

private struct WorkshopFlowLine: View {
    let active: Bool
    let reduceMotion: Bool
    @State private var travels = false

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Capsule().fill(TohsenoTheme.iron).frame(height: 2)
                Circle()
                    .fill(active ? TohsenoTheme.amber : TohsenoTheme.ash)
                    .frame(width: 7, height: 7)
                    .offset(x: travels && active && !reduceMotion ? max(0, geometry.size.width - 7) : 0)
                    .shadow(color: active ? TohsenoTheme.amber.opacity(0.7) : .clear, radius: 5)
            }
            .frame(maxHeight: .infinity)
        }
        .frame(minWidth: 32, idealWidth: 64, maxWidth: 90, minHeight: 10, maxHeight: 10)
        .animation(
            WorkshopMotion.activity(reduceMotion: reduceMotion, active: active),
            value: travels
        )
        .onAppear { travels = true }
        .accessibilityHidden(true)
    }
}

private struct WorkshopThresholdView: View {
    let threshold: WorkshopThreshold
    let updates: Int
    let detail: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: symbol)
                .font(.title2)
                .foregroundStyle(color)
            VStack(alignment: .leading, spacing: 3) {
                Text("Network threshold").font(.callout.weight(.semibold))
                Text(label).font(.caption).foregroundStyle(TohsenoTheme.silver)
                if updates > 0 {
                    Text("\(updates) unread keeper update\(updates == 1 ? "" : "s")")
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(TohsenoTheme.amber)
                }
            }
        }
        .padding(11)
        .frame(minWidth: 230, alignment: .leading)
        .background(TohsenoTheme.void.opacity(0.72))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(color.opacity(0.28)))
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .help(detail)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Network threshold. \(label). \(detail)")
        .accessibilityIdentifier("workshop.network-threshold")
    }

    private var label: String {
        switch threshold {
        case .unknown: "Public evidence not checked"
        case .privateOnly: "Workshop remains private"
        case .witnessed: "Public witness is available"
        case .publishingAvailable: "Companion-approved shipping is available"
        }
    }

    private var symbol: String {
        switch threshold {
        case .unknown: "questionmark.circle"
        case .privateOnly: "door.left.hand.closed"
        case .witnessed: "point.3.connected.trianglepath.dotted"
        case .publishingAvailable: "door.left.hand.open"
        }
    }

    private var color: Color {
        threshold == .publishingAvailable ? TohsenoTheme.amber : TohsenoTheme.silver
    }
}

private struct WorkshopShelfObject: View {
    let app: WorkshopAppObject
    let selected: Bool
    let open: () -> Void

    var body: some View {
        Button(action: open) {
            HStack(spacing: 10) {
                RoundedRectangle(cornerRadius: 9)
                    .fill(stateColor.opacity(0.18))
                    .frame(width: 38, height: 38)
                    .overlay(Image(systemName: stateSymbol).foregroundStyle(stateColor))
                VStack(alignment: .leading, spacing: 2) {
                    Text(app.name).font(.callout.weight(.semibold)).lineLimit(1)
                    Text(app.headline).font(.caption2).foregroundStyle(TohsenoTheme.silver).lineLimit(1)
                }
            }
            .padding(9)
            .frame(width: 210, alignment: .leading)
            .background(selected ? TohsenoTheme.ember.opacity(0.72) : TohsenoTheme.graphite)
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(selected ? TohsenoTheme.amber : TohsenoTheme.iron))
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(app.name). \(app.headline)")
    }

    private var stateColor: Color { app.state == .failed ? .red : TohsenoTheme.amber }
    private var stateSymbol: String {
        switch app.state {
        case .waiting: "clock"
        case .building: "hammer.fill"
        case .readyForPhone: "iphone.gen3"
        case .installing: "arrow.down.to.line.compact"
        case .installed: "checkmark.circle.fill"
        case .failed: "exclamationmark.triangle.fill"
        }
    }
}

private struct OneShotDock: View {
    @Bindable var model: TohsenoAppModel
    @Binding var choosingReferences: Bool
    var intentionFocused: FocusState<Bool>.Binding
    let canSubmit: Bool
    let adopt: () -> Void

    var body: some View {
        VStack(spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("ONE SHOT").font(.caption.weight(.bold)).tracking(2).foregroundStyle(TohsenoTheme.amber)
                    Text("Describe one app in ordinary words. The Mac keeps the source; nothing Ships without Companion approval.")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.silver)
                }
                Spacer()
                Button("Adopt app…", action: adopt)
                    .disabled(model.isSubmitting)
                    .accessibilityIdentifier("adopt-app.workshop")
                Button("More options…") { model.route = .create }
                    .accessibilityIdentifier("creation.options")
            }

            HStack(alignment: .bottom, spacing: 12) {
                VStack(spacing: 7) {
                    TextEditor(text: $model.creation.intention)
                        .font(.body)
                        .scrollContentBackground(.hidden)
                        .padding(8)
                        .frame(minHeight: 64, maxHeight: 92)
                        .background(TohsenoTheme.void)
                        .overlay(RoundedRectangle(cornerRadius: 11).stroke(
                            intentionFocused.wrappedValue ? TohsenoTheme.amber : TohsenoTheme.iron
                        ))
                        .focused(intentionFocused)
                        .shotSubmitOnReturn(enabled: canSubmit) {
                            Task { await model.submitCreation() }
                        }
                        .accessibilityLabel("One Shot intention")
                        .accessibilityIdentifier("workshop.shot.intention")
                    if !model.creation.references.isEmpty {
                        ScrollView(.horizontal) {
                            HStack(spacing: 6) {
                                ForEach(model.creation.references) { reference in
                                    HStack(spacing: 5) {
                                        Image(systemName: "photo")
                                        Text(reference.filename).lineLimit(1)
                                        Button {
                                            model.creation.references.removeAll { $0.id == reference.id }
                                        } label: {
                                            Image(systemName: "xmark.circle.fill")
                                        }
                                        .buttonStyle(.plain)
                                        .accessibilityLabel("Remove \(reference.filename)")
                                    }
                                    .font(.caption2)
                                    .padding(.horizontal, 7)
                                    .padding(.vertical, 5)
                                    .background(TohsenoTheme.graphite)
                                    .clipShape(Capsule())
                                }
                            }
                        }
                        .frame(height: 28)
                        .scrollIndicators(.hidden)
                    }
                }

                Button { choosingReferences = true } label: {
                    Image(systemName: "photo.badge.plus")
                        .frame(width: 28, height: 28)
                }
                .disabled(model.creation.references.count >= 8)
                .help("Add up to eight PNG or JPEG references")
                .accessibilityLabel("Add reference images")
                .accessibilityIdentifier("workshop.shot.references")

                Button {
                    Task { await model.submitCreation() }
                } label: {
                    HStack(spacing: 7) {
                        if model.isSubmitting {
                            TohsenoSpinner(size: 14, stroke: TohsenoTheme.void, gap: TohsenoTheme.amber)
                        }
                        Text(model.isSubmitting ? "Taking the Shot…" : "Take the Shot")
                    }
                }
                .buttonStyle(PrimaryActionStyle())
                .disabled(!canSubmit)
                .keyboardShortcut(.return, modifiers: [])
                .accessibilityIdentifier("workshop.shot.submit")
            }

            HStack {
                Label("Return sends · Shift–Return adds a line", systemImage: "return")
                Spacer()
                Text("Drop PNG/JPEG references here · \(model.creation.references.count)/8")
            }
            .font(.caption2)
            .foregroundStyle(TohsenoTheme.ash)
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 14)
        .background(.ultraThinMaterial)
        .overlay(alignment: .top) { Rectangle().fill(TohsenoTheme.amber.opacity(0.28)).frame(height: 1) }
        .accessibilityIdentifier("workshop.one-shot")
    }
}

private struct WorkshopPalette: View {
    @Bindable var model: TohsenoAppModel
    let close: () -> Void
    let adopt: () -> Void
    let showList: () -> Void
    let focusShot: () -> Void

    var body: some View {
        ZStack {
            Color.black.opacity(0.48).ignoresSafeArea().onTapGesture(perform: close)
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Workshop commands").font(.headline)
                    Spacer()
                    Text("Esc").font(.caption.monospaced()).foregroundStyle(TohsenoTheme.silver)
                }
                Divider()
                paletteButton("Focus One Shot", symbol: "scope", action: focusShot)
                paletteButton("Open app shelf", symbol: "list.bullet", action: showList)
                paletteButton("Adopt an existing app", symbol: "folder.badge.plus") { close(); adopt() }
                paletteButton("Open network threshold", symbol: "point.3.connected.trianglepath.dotted") {
                    close(); model.route = .registry
                }
                paletteButton("Open keeper and authority", symbol: "hand.raised") {
                    close(); model.route = .profile
                }
                SettingsLink { Label("Workshop settings", systemImage: "gearshape") }
                    .buttonStyle(.plain)
                    .padding(8)
            }
            .padding(16)
            .frame(width: 390)
            .background(TohsenoTheme.carbon)
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(TohsenoTheme.iron))
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .shadow(radius: 28)
        }
        .accessibilityIdentifier("workshop.command-palette")
    }

    private func paletteButton(_ title: String, symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(title, systemImage: symbol).frame(maxWidth: .infinity, alignment: .leading).padding(8)
        }
        .buttonStyle(.plain)
    }
}

private struct WorkshopListFallback: View {
    @Bindable var model: TohsenoAppModel
    @Binding var isPresented: Bool

    var body: some View {
        NavigationStack {
            List {
                Section("Apps in this workshop") {
                    ForEach(model.apps) { app in
                        Button {
                            isPresented = false
                            model.route = .app(app.id)
                        } label: {
                            Label {
                                VStack(alignment: .leading) {
                                    Text(app.displayName)
                                    Text(app.presentation.headline).font(.caption).foregroundStyle(.secondary)
                                }
                            } icon: {
                                Image(systemName: app.presentation.state == .failed
                                    ? "exclamationmark.triangle" : "app")
                            }
                        }
                    }
                    if model.apps.isEmpty { Text("No apps yet").foregroundStyle(.secondary) }
                }
                Section("Workshop places") {
                    Button("One Shot") { isPresented = false }
                    Button("Network threshold") { isPresented = false; model.route = .registry }
                    Button("Keeper and authority") { isPresented = false; model.route = .profile }
                }
            }
            .navigationTitle("Workshop list")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { isPresented = false }
                }
            }
        }
        .frame(minWidth: 480, minHeight: 520)
        .accessibilityIdentifier("workshop.list-fallback")
    }
}

private struct WorkshopField: View {
    var body: some View {
        Canvas { context, size in
            var path = Path()
            let spacing: CGFloat = 34
            stride(from: 0 as CGFloat, through: size.width, by: spacing).forEach { x in
                path.move(to: CGPoint(x: x, y: 0))
                path.addLine(to: CGPoint(x: x, y: size.height))
            }
            stride(from: 0 as CGFloat, through: size.height, by: spacing).forEach { y in
                path.move(to: CGPoint(x: 0, y: y))
                path.addLine(to: CGPoint(x: size.width, y: y))
            }
            context.stroke(path, with: .color(TohsenoTheme.iron.opacity(0.22)), lineWidth: 0.5)
        }
        .background(
            RadialGradient(
                colors: [TohsenoTheme.ember.opacity(0.24), TohsenoTheme.void],
                center: .top,
                startRadius: 0,
                endRadius: 680
            )
        )
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }
}

#if DEBUG
public struct TohsenoLivingWorkshopFixtureView: View {
    @Bindable private var model: TohsenoAppModel

    public init(model: TohsenoAppModel) {
        self.model = model
    }

    public var body: some View {
        LivingWorkshopView(model: model, adopt: {})
            .background(TohsenoTheme.void)
            .foregroundStyle(TohsenoTheme.bone)
            .tint(TohsenoTheme.amber)
    }
}
#endif

private extension Array where Element: Hashable {
    func uniqued() -> [Element] {
        var seen = Set<Element>()
        return filter { seen.insert($0).inserted }
    }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}
