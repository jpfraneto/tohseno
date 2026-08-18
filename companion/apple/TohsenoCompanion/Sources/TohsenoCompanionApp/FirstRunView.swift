import SwiftUI
import TohsenoCompanionKit

/// One-time setup, then the person never sees it again.
///
/// The permission is described in one sentence. The underlying capabilities
/// stay granular, signed, and revocable from the Mac; nothing here weakens
/// them, and none of that vocabulary appears on this screen.
struct FirstRunView: View {
    @Bindable var model: CompanionModel
    @State private var scanning = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            WordmarkView()
            Spacer()
            Text("Connect this iPhone\nto your Mac.")
                .font(.system(size: 30, weight: .semibold))
                .foregroundStyle(Tohseno.bone)
                .fixedSize(horizontal: false, vertical: true)
            Text("Open TOHSENO on your Mac, choose Settings, then Add iPhone. This iPhone will be able to create and evolve apps on that Mac, and you can revoke it at any time.")
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
            Button("Scan the code") {
                Task {
                    await model.createIdentity()
                    scanning = true
                }
            }
            .buttonStyle(PrimaryButtonStyle(enabled: !model.busy))
            .disabled(model.busy)
            .frame(maxWidth: .infinity)
        }
        .padding(28)
        .sheet(isPresented: $scanning) { scanner }
    }

#if os(iOS)
    private var scanner: some View {
        NavigationStack {
            PairingScannerView(
                onScan: { payload in
                    scanning = false
                    Task { await model.pair(scanned: payload) }
                },
                onFailure: { _ in scanning = false }
            )
            .ignoresSafeArea()
            .navigationTitle("Scan your Mac")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { scanning = false }
                }
            }
        }
    }
#else
    private var scanner: some View {
        Text("Scanning requires an iPhone camera.")
            .foregroundStyle(Tohseno.ash)
            .padding(40)
    }
#endif
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
