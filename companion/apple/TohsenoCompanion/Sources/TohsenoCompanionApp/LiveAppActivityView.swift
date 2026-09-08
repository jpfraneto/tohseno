import SwiftUI
import TohsenoCompanionKit

struct LiveAppActivityView: View {
    @Bindable var model: CompanionModel
    let shot: ShotSummary

    var body: some View {
        ScrollViewReader { scroll in
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if let execution = shot.execution {
                        HStack(spacing: 12) {
                            if !execution.state.isTerminal && execution.state != .waitingForDevice { ProgressView().tint(Tohseno.orange) }
                            Text(WorkshopRequest.message(for: execution)).font(.title3.weight(.semibold))
                        }
                        if execution.state == .waitingForDevice {
                            Text("Connect your iPhone by cable or to the same Wi-Fi as your Mac, and keep it unlocked for installation.")
                                .foregroundStyle(Tohseno.ash)
                        }
                        if let activity = execution.activity {
                            HStack(spacing: 16) {
                                Label("\(activity.fileCount) files changed", systemImage: "doc.text")
                                if let tokens = activity.totalTokens {
                                    Text("\(tokens.formatted()) tokens processed")
                                }
                            }.font(.caption).foregroundStyle(Tohseno.ash)
                            ForEach(activity.entries) { entry in
                                VStack(alignment: .leading, spacing: 5) {
                                    Text(entry.message).font(.body).textSelection(.enabled)
                                    if let date = ISO8601DateFormatter().date(from: entry.timestamp) {
                                        Text(date, format: .dateTime.hour().minute().second())
                                            .font(.caption2.monospaced()).foregroundStyle(Tohseno.ash)
                                    }
                                }.frame(maxWidth: .infinity, alignment: .leading)
                            }
                            if !activity.files.isEmpty {
                                VStack(alignment: .leading, spacing: 8) {
                                    Text("Source taking shape").font(.headline)
                                    ForEach(activity.files, id: \.self) { file in
                                        Label(file, systemImage: "doc.text")
                                            .font(.caption.monospaced()).foregroundStyle(Tohseno.ash)
                                    }
                                }
                            }
                        } else {
                            Text("Waiting for the next activity report from your Mac…")
                                .foregroundStyle(Tohseno.ash)
                        }
                        Color.clear.frame(height: 1).id("latest")
                    } else {
                        StateView(presentation: model.presentation(for: shot))
                    }
                }.padding(24).foregroundStyle(Tohseno.bone)
            }
            .onChange(of: shot.execution?.activity?.entries.last?.sequence) { _, _ in
                withAnimation { scroll.scrollTo("latest", anchor: .bottom) }
            }
            .refreshable { await model.syncNow() }
        }
        .accessibilityIdentifier("app.liveActivity")
    }
}
