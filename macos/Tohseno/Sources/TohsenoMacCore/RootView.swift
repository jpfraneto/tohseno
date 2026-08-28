import SwiftUI
import UniformTypeIdentifiers

public struct TohsenoRootView: View {
    @Bindable private var model: TohsenoAppModel

    public init(model: TohsenoAppModel) {
        self.model = model
    }

    public var body: some View {
        Group {
            if model.isLoading, model.workspace == nil {
                VStack(spacing: 14) {
                    TohsenoSpinner(size: 44)
                    Text("Opening your app factory…")
                }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let readiness = model.readiness, !readiness.ready {
                ReadinessScreen(model: model, readiness: readiness)
            } else {
                factory
            }
        }
        .background(TohsenoTheme.void)
        .foregroundStyle(TohsenoTheme.bone)
        .tint(TohsenoTheme.amber)
        .task { model.start() }
        .alert("TOHSENO", isPresented: errorBinding) {
            Button("OK") { model.dismissError() }
        } message: {
            Text(model.errorMessage ?? "Something stopped safely.")
        }
    }

    private var factory: some View {
        NavigationSplitView {
            List(selection: routeBinding) {
                Section {
                    Label("Registry", systemImage: "point.3.connected.trianglepath.dotted")
                        .tag(AppRoute.registry)
                        .accessibilityIdentifier("registry.sidebar")
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
                    Label("Create App", systemImage: "plus")
                        .tag(AppRoute.create)
                        .accessibilityIdentifier("create-app.sidebar")
                }
            }
            .scrollContentBackground(.hidden)
            .background(TohsenoTheme.carbon)
            .navigationSplitViewColumnWidth(min: 210, ideal: 250)
            .safeAreaInset(edge: .top) {
                HStack(spacing: 10) {
                    TohsenoMark().frame(width: 28, height: 28)
                    Text("TOHSENO").font(.headline).tracking(2.5)
                    Spacer()
                }
                .padding(14)
                .background(TohsenoTheme.carbon)
            }
        } detail: {
            switch model.route {
            case .library:
                LibraryEmptyView { model.route = .create }
            case .registry:
                RegistryView(model: model)
            case .create:
                CreationView(model: model)
            case .app:
                if let app = model.selectedApp {
                    AppDetailView(model: model, app: app)
                } else {
                    LibraryEmptyView { model.route = .create }
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

private struct LibraryEmptyView: View {
    let create: () -> Void

    var body: some View {
        ContentUnavailableView {
            Label("Your Apps", systemImage: "square.grid.2x2")
        } description: {
            Text("Personal iPhone apps you make live here, with their source and history on this Mac.")
        } actions: {
            Button("Create App", action: create)
                .buttonStyle(PrimaryActionStyle())
                .accessibilityIdentifier("create-app.empty")
        }
    }
}

private struct RegistryView: View {
    @Bindable var model: TohsenoAppModel
    @FocusState private var quickShotFocused: Bool

    private var canSubmitQuickShot: Bool {
        !model.isSubmitting &&
            !model.quickShotIntention.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Registry").font(.largeTitle.bold())
                    Text("Your verified local track record. Nothing on this screen is published publicly.")
                        .foregroundStyle(TohsenoTheme.silver)
                }

                quickShotComposer

                if model.isLoadingRegistry, model.registrySnapshot == nil {
                    HStack(spacing: 10) {
                        TohsenoSpinner(size: 20)
                        Text("Verifying your local Registry…")
                            .foregroundStyle(TohsenoTheme.silver)
                    }
                } else if let snapshot = model.registrySnapshot {
                    builders(snapshot)
                    network(snapshot.network)
                    shots(snapshot.records)
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
            quickShotFocused = true
        }
    }

    private var quickShotComposer: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("New Shot").font(.title2.weight(.semibold))
                Spacer()
                ShotKeyboardHint()
            }
            TextField(
                "Describe a new app…",
                text: $model.quickShotIntention,
                axis: .vertical
            )
            .lineLimit(1...5)
            .textFieldStyle(.plain)
            .font(.body)
            .padding(13)
            .background(TohsenoTheme.carbon)
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(TohsenoTheme.iron))
            .focused($quickShotFocused)
            .shotSubmitOnReturn(enabled: canSubmitQuickShot) {
                Task { await model.submitQuickShot() }
            }
            .accessibilityLabel("Describe a new app")
            .accessibilityIdentifier("registry.quick.intention")
            HStack {
                Text("Uses your automatic intelligence route. Create App adds names, references, and advanced choices.")
                    .font(.caption)
                    .foregroundStyle(TohsenoTheme.silver)
                Spacer()
                Button {
                    Task { await model.submitQuickShot() }
                } label: {
                    HStack(spacing: 7) {
                        if model.isSubmitting {
                            TohsenoSpinner(
                                size: 14,
                                stroke: TohsenoTheme.void,
                                gap: TohsenoTheme.amber
                            )
                        }
                        Text(model.isSubmitting ? "Sending…" : "Send Shot")
                    }
                }
                .buttonStyle(PrimaryActionStyle())
                .disabled(!canSubmitQuickShot)
                .keyboardShortcut(.return, modifiers: [])
                .accessibilityIdentifier("registry.quick.submit")
            }
        }
        .padding(18)
        .background(TohsenoTheme.graphite)
        .clipShape(RoundedRectangle(cornerRadius: 14))
    }

    private func builders(_ snapshot: RegistrySnapshot) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Builders").font(.title2.weight(.semibold))
            VStack(alignment: .leading, spacing: 15) {
                HStack(alignment: .top, spacing: 13) {
                    TohsenoMark().frame(width: 38, height: 38)
                    VStack(alignment: .leading, spacing: 5) {
                        Text("This Mac").font(.headline)
                        Text(compact(snapshot.builder.builderID))
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(TohsenoTheme.silver)
                    }
                    Spacer()
                    Label("Local only", systemImage: "lock.fill")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(TohsenoTheme.amber)
                }
                Divider().overlay(TohsenoTheme.iron)
                HStack(spacing: 28) {
                    RegistryMetric(value: "\(snapshot.records.count)", label: "verified Shots")
                    RegistryMetric(value: "\(snapshot.acceptedVersionCount)", label: "accepted versions")
                    RegistryMetric(value: snapshot.network.activeGeneration, label: "active generation")
                }
                Text("This is the existing legacy local DeviceKey track record, not a public Builder authority.")
                    .font(.caption)
                    .foregroundStyle(TohsenoTheme.silver)
            }
            .padding(18)
            .background(TohsenoTheme.graphite)
            .clipShape(RoundedRectangle(cornerRadius: 14))
            .accessibilityIdentifier("registry.builder")
        }
    }

    private func network(_ status: RegistryNetworkStatus) -> some View {
        HStack(alignment: .top, spacing: 13) {
            Image(systemName: status.rpcChecked ? "network" : "network.slash")
                .font(.title2)
                .foregroundStyle(TohsenoTheme.amber)
            VStack(alignment: .leading, spacing: 5) {
                Text("Public Registry").font(.headline)
                Text(status.rpcChecked ? "Public witness checked" : "Not connected")
                    .foregroundStyle(TohsenoTheme.silver)
                Text(status.reason)
                    .font(.caption)
                    .foregroundStyle(TohsenoTheme.silver)
            }
            Spacer()
        }
        .padding(18)
        .background(TohsenoTheme.carbon)
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(TohsenoTheme.iron))
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .accessibilityIdentifier("registry.public-status")
    }

    private func shots(_ records: [LocalRegistryRecord]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Shots").font(.title2.weight(.semibold))
            if records.isEmpty {
                Text("Your first accepted app will appear here.")
                    .foregroundStyle(TohsenoTheme.silver)
            } else {
                ForEach(records) { record in
                    HStack(spacing: 13) {
                        if let app = model.apps.first(where: { $0.id == record.shotID }) {
                            AppArtwork(data: model.icons[app.id], size: 42, cornerRadius: 9)
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            Text(record.appName).font(.headline)
                            HStack(spacing: 8) {
                                Label(
                                    record.localVerified ? "Verified locally" : "Unverified",
                                    systemImage: record.localVerified ? "checkmark.seal.fill" : "xmark.seal"
                                )
                                Text("Version \(record.localSequence)")
                            }
                            .font(.caption)
                            .foregroundStyle(TohsenoTheme.silver)
                        }
                        Spacer()
                        Text("Private")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(TohsenoTheme.amber)
                        Button("Open") {
                            if let app = model.apps.first(where: { $0.id == record.shotID }) {
                                model.route = .app(app.id)
                            }
                        }
                    }
                    .padding(14)
                    .background(TohsenoTheme.graphite)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .accessibilityIdentifier("registry.record.\(record.shotID)")
                }
            }
        }
    }

    private func compact(_ value: String) -> String {
        guard value.count > 30 else { return value }
        return "\(value.prefix(20))…\(value.suffix(8))"
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

private struct ReadinessScreen: View {
    let model: TohsenoAppModel
    let readiness: ReadinessView

    var body: some View {
        VStack(spacing: 24) {
            if readiness.isWorking {
                TohsenoSpinner(size: 72)
            } else {
                TohsenoMark().frame(width: 72, height: 72)
            }
            VStack(spacing: 10) {
                Text(readiness.headline).font(.largeTitle.weight(.semibold))
                Text(readiness.detail)
                    .foregroundStyle(TohsenoTheme.silver)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 560)
            }
            if readiness.primaryAction != nil {
                Button(readiness.primaryLabel ?? "Continue") {
                    Task { await model.performReadinessAction() }
                }
                .buttonStyle(PrimaryActionStyle())
                .disabled(model.isSubmitting)
                .accessibilityIdentifier("readiness.primary")
            }
            if model.isSubmitting { TohsenoSpinner(size: 18) }
        }
        .padding(56)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("readiness.\(readiness.step)")
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
                Button("Add reference images…") { choosingReferences = true }
                    .disabled(model.creation.references.count >= 8)
                    .accessibilityIdentifier("creation.references")
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

private struct AppDetailView: View {
    @Bindable var model: TohsenoAppModel
    let app: AppSummary
    @State private var choosingReferences = false
    @State private var showingDetails = false
    @State private var confirmingRetire = false
    @FocusState private var intentionFocused: Bool

    private var draft: Binding<EvolutionDraft> {
        Binding(
            get: { model.evolutions[app.id] ?? EvolutionDraft() },
            set: { model.evolutions[app.id] = $0 }
        )
    }

    private var canSubmit: Bool {
        !model.isSubmitting && app.latestVersionID != nil &&
            !draft.wrappedValue.intention.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(alignment: .top, spacing: 16) {
                    AppArtwork(data: model.icons[app.id], size: 72, cornerRadius: 15)
                    VStack(alignment: .leading, spacing: 5) {
                        Text(app.displayName).font(.largeTitle.bold())
                        Label(app.presentation.headline, systemImage: app.presentation.state == .failed ? "exclamationmark.triangle" : "circle.fill")
                            .foregroundStyle(app.presentation.state == .failed ? .red : TohsenoTheme.silver)
                    }
                    Spacer()
                }
                if let detail = app.presentation.detail {
                    Text(detail).foregroundStyle(TohsenoTheme.silver)
                }
                if let data = model.previews[app.id], let image = NSImage(data: data) {
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFit()
                        .frame(maxWidth: 520, maxHeight: 360)
                        .clipShape(RoundedRectangle(cornerRadius: 14))
                        .overlay(RoundedRectangle(cornerRadius: 14).stroke(TohsenoTheme.iron))
                        .accessibilityLabel("Latest accepted first-screen preview")
                        .accessibilityIdentifier("app.preview")
                    Text("Latest accepted preview — not interactive")
                        .font(.caption)
                        .foregroundStyle(TohsenoTheme.silver)
                }
                if app.presentation.state.isInFlight {
                    HStack(spacing: 9) {
                        TohsenoSpinner(size: 18)
                        Text(progressLanguage(app.presentation.state))
                            .font(.caption)
                            .foregroundStyle(TohsenoTheme.silver)
                    }
                }
                Divider().overlay(TohsenoTheme.iron)
                Text("What should change?").font(.title2.weight(.semibold))
                TextEditor(text: draft.intention)
                    .scrollContentBackground(.hidden)
                    .padding(10)
                    .frame(minHeight: 130)
                    .background(TohsenoTheme.carbon)
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(TohsenoTheme.iron))
                    .focused($intentionFocused)
                    .shotSubmitOnReturn(enabled: canSubmit) {
                        Task { await model.submitEvolution(for: app) }
                    }
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
                HStack {
                    RouteCostView(model: model, harness: draft.wrappedValue.harness, estimate: model.evolutionEstimates[app.id])
                    ShotKeyboardHint()
                    Spacer()
                    Button {
                        Task { await model.submitEvolution(for: app) }
                    } label: {
                        HStack(spacing: 7) {
                            if model.isSubmitting {
                                TohsenoSpinner(
                                    size: 14,
                                    stroke: TohsenoTheme.void,
                                    gap: TohsenoTheme.amber
                                )
                            }
                            Text(model.isSubmitting ? "Evolving…" : "Evolve App")
                        }
                    }
                    .buttonStyle(PrimaryActionStyle())
                    .disabled(!canSubmit)
                    .keyboardShortcut(.return, modifiers: [])
                    .accessibilityIdentifier("evolution.submit")
                }
                Divider().overlay(TohsenoTheme.iron)
                HStack {
                    if app.presentation.state == .installed {
                        Button("Open on iPhone") { Task { await model.openOnPhone(for: app) } }
                            .accessibilityIdentifier("app.open-on-iphone")
                    }
                    Button("Open Source Folder") { Task { await model.openSource(for: app) } }
                    Button("Details…") {
                        Task { await model.showReceipt(for: app); showingDetails = true }
                    }
                    Spacer()
                    Button("Retire…", role: .destructive) { confirmingRetire = true }
                }
            }
            .frame(maxWidth: 820, alignment: .leading)
            .padding(40)
        }
        .fileImporter(isPresented: $choosingReferences, allowedContentTypes: [.png, .jpeg], allowsMultipleSelection: true) { result in
            model.addReferences(result, to: .evolution(app.id))
        }
        .dropDestination(for: URL.self) { urls, _ in
            model.addReferences(.success(urls), to: .evolution(app.id))
            return !urls.isEmpty
        }
        .task(id: app.id) {
            await model.prepareEvolution(for: app)
            intentionFocused = true
        }
        .sheet(isPresented: $showingDetails) { ExecutionDetailsView(receipt: model.receipt, balance: model.managedBalance) }
        .confirmationDialog("Retire \(app.displayName)?", isPresented: $confirmingRetire) {
            Button("Retire App", role: .destructive) { Task { await model.retire(app) } }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This removes the app from Your Apps. Its source and history stay on this Mac and remain recoverable.")
        }
    }

    private func progressLanguage(_ state: PresentedState) -> String {
        switch state {
        case .waiting: "Waiting for the local factory"
        case .building: "Creating the interface and checking the build"
        case .readyForPhone: "The build is ready for your connected iPhone"
        case .installing: "Installing the verified build on your iPhone"
        case .installed: "Installed"
        case .failed: "Stopped safely"
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
    let model: TohsenoAppModel
    @Binding var harness: String?
    @Binding var selectedModel: String?
    @Binding var privacy: String
    @Binding var maximumMicrousd: UInt64?
    @Binding var consent: Bool
    let estimate: ManagedEstimate?

    var body: some View {
        DisclosureGroup("Advanced") {
            VStack(alignment: .leading, spacing: 12) {
                Picker("Intelligence", selection: $harness) {
                    Text("Automatic — \(model.defaults?.harnessLabel ?? "best available")").tag(String?.none)
                    ForEach(model.defaults?.harnesses ?? []) { option in
                        Text("\(option.label) — \(availability(option))").tag(Optional(option.id))
                    }
                    if model.managedCatalog?.models.isEmpty == false {
                        Text("TOHSENO managed intelligence — uses creation balance")
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
                    Text("TOHSENO will use this exact selection for this request and will not substitute another route during recovery.")
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
        Text("Managed work sends necessary app context through TOHSENO and Bankr to the selected upstream provider under this privacy tier. It never activates automatically.")
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
            Text("You can use TOHSENO-managed intelligence with creation balance, or configure your own route in Settings.")
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
