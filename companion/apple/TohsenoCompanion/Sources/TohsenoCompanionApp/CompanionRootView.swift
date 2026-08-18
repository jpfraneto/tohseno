import SwiftUI
import TohsenoCompanionKit

/// The whole product: Your Apps → choose app → what should change → Evolve App.
public struct CompanionRootView: View {
    @State private var model: CompanionModel

    public init(model: CompanionModel) {
        _model = State(initialValue: model)
    }

    public var body: some View {
        ZStack {
            Tohseno.void.ignoresSafeArea()
            switch model.screen {
            case .firstRun:
                FirstRunView(model: model)
            case .apps:
                YourAppsView(model: model)
            case let .app(shotID):
                if let shot = model.app(shotID) {
                    AppView(model: model, shot: shot)
                } else {
                    YourAppsView(model: model)
                }
            }
        }
        .tint(Tohseno.orange)
        .preferredColorScheme(.dark)
        .task { model.start() }
    }
}

struct YourAppsView: View {
    @Bindable var model: CompanionModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            WordmarkView().padding(.horizontal, 24).padding(.bottom, 28)
            Text("Your Apps")
                .font(.system(size: 30, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
                .padding(.horizontal, 24)
            if model.apps.isEmpty {
                Spacer()
                Text("Apps you make on your Mac appear here.")
                    .font(.system(size: 16))
                    .foregroundStyle(Tohseno.ash)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.horizontal, 40)
                Spacer()
            } else {
                ScrollView {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 12)], spacing: 12) {
                        ForEach(model.apps, id: \.shotID) { shot in
                            Button { model.open(shot) } label: {
                                AppTile(
                                    shot: shot,
                                    icon: model.icons[shot.icon?.blobID ?? shot.shotID],
                                    presentation: model.presentation(for: shot)
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(24)
                }
            }
            if let notice = model.notice {
                NoticeView(text: notice).padding(24)
            }
        }
        .padding(.top, 24)
        .refreshable { await model.refresh() }
    }
}

struct AppTile: View {
    let shot: ShotSummary
    let icon: Data?
    let presentation: TohsenoPresentation

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            IconView(name: shot.displayName, bytes: icon)
            Text(shot.displayName)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
                .lineLimit(1)
            // One subtle indicator, and only when something is actually going
            // on. A settled app says nothing.
            if presentation.state != .installed {
                Text(status)
                    .font(.system(size: 13))
                    .foregroundStyle(presentation.state == .failed ? Tohseno.ash : Tohseno.orange)
                    .lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Tohseno.iron))
    }

    private var status: String {
        switch presentation.state {
        case .waiting: "Waiting"
        case .building: "Evolving"
        case .readyForPhone: "Ready to install"
        case .installing: "Installing"
        case .installed: ""
        case .failed: "Failed"
        }
    }
}

struct IconView: View {
    let name: String
    let bytes: Data?

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
        .frame(width: 52, height: 52)
        .clipShape(RoundedRectangle(cornerRadius: 13))
    }

    private var placeholder: some View {
        RoundedRectangle(cornerRadius: 13)
            .fill(Tohseno.iron)
            .overlay(
                Text(String(name.prefix(1)).uppercased())
                    .font(.system(size: 22, weight: .bold))
                    .foregroundStyle(Tohseno.orange)
            )
    }
}

struct AppView: View {
    @Bindable var model: CompanionModel
    let shot: ShotSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Button { model.openApps() } label: {
                    Label("Your Apps", systemImage: "chevron.left")
                        .font(.system(size: 15))
                        .foregroundStyle(Tohseno.ash)
                }
                Spacer()
            }
            .padding(.horizontal, 20)

            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    Text(shot.displayName.uppercased())
                        .font(.system(size: 26, weight: .semibold))
                        .kerning(1.5)
                        .foregroundStyle(Tohseno.bone)
                        .padding(.top, 20)

                    let presentation = model.presentation(for: shot)
                    if presentation.state != .installed {
                        StateView(presentation: presentation).padding(.top, 22)
                    }

                    if presentation.state != .building, presentation.state != .installing {
                        composer.padding(.top, 30)
                    }

                    if let notice = model.notice {
                        NoticeView(text: notice).padding(.top, 22)
                    }
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 40)
            }
        }
        .padding(.top, 20)
        .refreshable { await model.refresh() }
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("What should change?")
                .font(.system(size: 16))
                .foregroundStyle(Tohseno.ash)
            TextEditor(text: $model.intent)
                .scrollContentBackground(.hidden)
                .font(.system(size: 17))
                .foregroundStyle(Tohseno.bone)
                .frame(minHeight: 190)
                .padding(14)
                .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 14))
                .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Tohseno.iron))
            ScreenshotPicker(attachments: $model.attachments)
            HStack {
                Spacer()
                Button("Evolve App") {
                    Task { await model.evolve() }
                }
                .buttonStyle(PrimaryButtonStyle(enabled: model.canEvolve))
                .disabled(!model.canEvolve)
            }
        }
    }
}

struct StateView: View {
    let presentation: TohsenoPresentation

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(presentation.headline)
                .font(.system(size: 24, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
            if let detail = presentation.detail {
                Text(detail)
                    .font(.system(size: 16))
                    .foregroundStyle(Tohseno.ash)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if presentation.state.inFlight {
                ProgressView()
                    .progressViewStyle(.linear)
                    .tint(Tohseno.orange)
                    .frame(width: 160)
                    .padding(.top, 6)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
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
