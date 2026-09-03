import SwiftUI
import TohsenoCompanionKit

/// One-time setup, then the person never sees it again.
///
/// The permission is described in one sentence. The underlying capabilities
/// stay granular, signed, and revocable from the Mac; nothing here weakens
/// them, and none of that vocabulary appears on this screen.
struct FirstRunView: View {
    @Bindable var model: CompanionModel
#if DEBUG && os(iOS)
    @State private var localNetworkPermission = LocalNetworkPermissionRequest()
#endif

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            WordmarkView()
            Spacer()
            Text("BRING YOUR IPHONE INTO THE WORKSHOP")
                .font(.caption.weight(.bold))
                .tracking(1.7)
                .foregroundStyle(Tohseno.orange)
            FirstRunWorkshopConnection()
                .padding(.vertical, 22)
            Text("Connect this iPhone\nto your Mac workshop.")
                .font(.system(size: 30, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
                .fixedSize(horizontal: false, vertical: true)
            Text("The Mac is the factory. This iPhone becomes the private remote and keeper of your human approval. You can revoke it from the Mac at any time.")
                .font(.system(size: 16))
                .foregroundStyle(Tohseno.ash)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 14)

            if let words = model.recoveryWords {
                RecoveryWordsView(words: words).padding(.top, 26)
            }

            if let notice = model.notice {
                NoticeView(text: notice).padding(.top, 22)
            }

            Spacer()
            if model.busy {
                ProgressView("Connecting…")
                    .tint(Tohseno.orange)
                    .foregroundStyle(Tohseno.ash)
                    .frame(maxWidth: .infinity)
            } else if model.recoveryWords != nil {
                Button("I've written them down") {
                    Task { await model.confirmRecoveryWords() }
                }
                .buttonStyle(PrimaryButtonStyle(enabled: true))
                .frame(maxWidth: .infinity)
            }
        }
        .padding(28)
        .onAppear {
#if DEBUG && os(iOS)
            // The development Companion talks to the same encrypted relay on
            // its Mac. Trigger iOS's permission sheet before camera capture so
            // a denied local-network operation is not misreported as a bad QR.
            localNetworkPermission.begin()
#endif
        }
    }

}

private struct FirstRunWorkshopConnection: View {
    var body: some View {
        HStack(spacing: 10) {
            actor("Mac factory", symbol: "macbook")
            HStack(spacing: 3) {
                Circle().frame(width: 4, height: 4)
                Rectangle().frame(height: 1)
                Circle().frame(width: 4, height: 4)
            }
            .foregroundStyle(Tohseno.orange)
            .accessibilityHidden(true)
            actor("This iPhone", symbol: "iphone.gen3")
        }
        .padding(14)
        .frame(maxWidth: .infinity)
        .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(Tohseno.orange.opacity(0.22)))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Connect the Mac factory to this iPhone keeper")
        .accessibilityIdentifier("workshop.first-run-connection")
    }

    private func actor(_ title: String, symbol: String) -> some View {
        VStack(spacing: 7) {
            Image(systemName: symbol).font(.title2).foregroundStyle(Tohseno.orange)
            Text(title).font(.caption.weight(.semibold)).foregroundStyle(Tohseno.bone)
        }
        .frame(maxWidth: .infinity)
    }
}

/// Shown exactly once, only when this iPhone's identity is first created.
struct RecoveryWordsView: View {
    let words: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Write these twelve words down.")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
            Text(words)
                .font(.system(size: 15, design: .monospaced))
                .foregroundStyle(Tohseno.orange)
                .fixedSize(horizontal: false, vertical: true)
            Text("They restore this iPhone's identity. They do not restore access to a Mac — you connect again from the Mac.")
                .font(.system(size: 13))
                .foregroundStyle(Tohseno.ash)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Tohseno.iron))
    }
}
