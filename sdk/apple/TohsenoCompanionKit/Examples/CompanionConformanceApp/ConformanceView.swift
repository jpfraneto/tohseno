import SwiftUI
import TohsenoCompanionKit
import UIKit

@MainActor
@Observable
final class ConformanceModel {
    var pairingPayload = ""
    var recoveryWords = ""
    var status = "Identity not created"
    var shots: [ShotSummary] = []
    var iconBytes: [String: Data] = [:]
    var selectedShotID: String?
    var feedbackBody = "Conformance feedback for the exact accepted Version."
    var marketingBody = "Private conformance marketing note."
    var evolutionIntention = "Conformance evolution request."
    var creationName = "fixture-child"
    var creationIntention = "Conformance Shot request."
    var feedbackCommandID = "physical-feedback-verification"
    var marketingCommandID = "physical-marketing-verification"
    var evolutionCommandID = "physical-evolution-verification"
    var creationCommandID = "physical-creation-verification"
    let client: TohsenoCompanionClient

    init() throws {
        let stateURL = URL.applicationSupportDirectory
            .appending(path: "TOHSENO", directoryHint: .isDirectory)
            .appending(path: "companion-state.bin")
        let stateStore = try FileCompanionStateStore(fileURL: stateURL)
        let payloadStore = try FileCompanionPayloadStore(
            directoryURL: stateURL.deletingLastPathComponent().appending(path: "outbox")
        )
        let productionRelay = "https://companion.tohseno.com"
#if DEBUG
        let configuredRelay = Bundle.main.object(
            forInfoDictionaryKey: "TOHSENOVerificationRelayOrigin"
        ) as? String
        let relayOrigin = configuredRelay.flatMap(URL.init(string:))
            ?? URL(string: productionRelay)!
#else
        let relayOrigin = URL(string: productionRelay)!
#endif
        let endpoint = try RelayEndpoint(
            id: "official-v1",
            baseURL: relayOrigin
        )
        client = TohsenoCompanionClient(
            stateStore: stateStore,
            payloadStore: payloadStore,
            relayAllowlist: try RelayAllowlist([endpoint])
        )
        Task {
            for await connection in client.connectionState {
                status = String(describing: connection)
            }
        }
        Task {
            for await _ in client.workspaceEvents {
                if let workspace = try? await client.currentWorkspace() {
                    shots = workspace.shots
                }
            }
        }
    }

    func createIdentity() {
        Task {
            do {
                let phrase = try await client.createIdentity()
                recoveryWords = phrase.reveal()
                status = "Identity created in Keychain; recovery words shown explicitly"
            } catch {
                status = "Identity creation failed"
            }
        }
    }

    func restoreIdentity() {
        Task {
            do {
                try await client.restoreIdentity(from: RecoveryPhrase(recoveryWords))
                status = "Identity restored; scan a new pairing seal"
            } catch {
                status = "Identity restoration failed"
            }
        }
    }

    func acceptScannedPayload(_ payload: String) {
        pairingPayload = payload
        status = "Studio QR scanned by the camera"
    }

    func pairScannedPayload() {
        Task {
            do {
                try await client.pair(with: pairingPayload, displayName: UIDevice.current.name)
                try await refresh()
                try await client.startForegroundSynchronization()
                status = "Paired; encrypted workspace received; live sync active"
            } catch {
                status = "Pairing failed"
            }
        }
    }

    func startSynchronization() {
        Task {
            do {
                try await client.startForegroundSynchronization()
                status = "Foreground live synchronization active"
            } catch {
                status = "Live synchronization failed"
            }
        }
    }

    func stopSynchronization() {
        Task {
            await client.stopForegroundSynchronization()
            status = "Foreground live synchronization stopped"
        }
    }

    func refresh() async throws {
        try await client.reconcile()
        shots = try await client.currentWorkspace().shots
        if selectedShotID == nil { selectedShotID = shots.first?.shotID }
        var loaded: [String: Data] = [:]
        for shot in shots {
            if let descriptor = shot.icon,
               let blob = try await client.iconBlob(for: descriptor) {
                loaded[shot.shotID] = blob.bytes
            }
        }
        iconBytes = loaded
    }

    func select(_ shot: ShotSummary) {
        selectedShotID = shot.shotID
        status = "Selected exact Version \(shot.latestVersionOrdinal.map(String.init) ?? "none")"
    }

    var selectedShot: ShotSummary? {
        shots.first { $0.shotID == selectedShotID }
    }

    func submitFeedback() {
        guard let shot = selectedShot,
              let expression = shot.expressionID,
              let version = shot.latestVersionID,
              let ordinal = shot.latestVersionOrdinal else { return }
        Task {
            do {
                _ = try await client.submitFeedback(FeedbackRequest(
                    commandID: feedbackCommandID,
                    shotID: shot.shotID,
                    expressionID: expression,
                    versionID: version,
                    versionOrdinal: ordinal,
                    body: feedbackBody
                ))
                status = "Feedback acknowledged for exact Version \(ordinal)"
            } catch {
                status = "Feedback queued or rejected; reconcile to resolve"
            }
        }
    }

    func submitMarketing() {
        guard let shot = selectedShot else { return }
        Task {
            do {
                _ = try await client.submitMarketingNote(MarketingRequest(
                    commandID: marketingCommandID,
                    noteID: marketingCommandID,
                    shotID: shot.shotID,
                    body: marketingBody
                ))
                status = "Marketing note acknowledged"
            } catch {
                status = "Marketing note queued or rejected; reconcile to resolve"
            }
        }
    }

    func requestEvolution() {
        guard let shot = selectedShot,
              let expression = shot.expressionID,
              let version = shot.latestVersionID,
              let ordinal = shot.latestVersionOrdinal else { return }
        Task {
            do {
                _ = try await client.requestEvolution(EvolutionRequest(
                    commandID: evolutionCommandID,
                    shotID: shot.shotID,
                    baseExpressionID: expression,
                    baseVersionID: version,
                    baseVersionOrdinal: ordinal,
                    intention: evolutionIntention
                ))
                status = "Evolution accepted from exact base Version \(ordinal)"
            } catch {
                status = "Evolution queued or rejected; reconcile to resolve"
            }
        }
    }

    func requestCreation() {
        Task {
            do {
                _ = try await client.requestShotCreation(CreateShotRequest(
                    commandID: creationCommandID,
                    suggestedName: creationName,
                    intention: creationIntention
                ))
                status = "New Shot request accepted"
            } catch {
                status = "New Shot request queued or rejected; reconcile to resolve"
            }
        }
    }

    func exerciseCommands(for shot: ShotSummary) {
        guard let expression = shot.expressionID,
              let version = shot.latestVersionID,
              let ordinal = shot.latestVersionOrdinal else { return }
        Task {
            _ = try await client.submitFeedback(FeedbackRequest(
                shotID: shot.shotID, expressionID: expression,
                versionID: version, versionOrdinal: ordinal,
                body: "Conformance feedback for the exact accepted Version."
            ))
            _ = try await client.submitMarketingNote(MarketingRequest(
                shotID: shot.shotID, body: "Private conformance marketing note."
            ))
            _ = try await client.requestEvolution(EvolutionRequest(
                shotID: shot.shotID, baseExpressionID: expression,
                baseVersionID: version, baseVersionOrdinal: ordinal,
                intention: "Conformance evolution request."
            ))
            _ = try await client.requestShotCreation(CreateShotRequest(
                suggestedName: "fixture-child", intention: "Conformance Shot request."
            ))
        }
    }
}

struct ConformanceView: View {
    @State var model: ConformanceModel
    @State private var scannerPresented = false

    var body: some View {
        NavigationStack {
            Form {
                Section("IDENTITY") {
                    Button("CREATE 12-WORD IDENTITY", action: model.createIdentity)
                    SecureField("Recovery words", text: $model.recoveryWords)
                    Button("RESTORE (PAIR AGAIN)", action: model.restoreIdentity)
                }
                Section("PAIRING") {
                    Button("SCAN STUDIO QR WITH CAMERA") { scannerPresented = true }
                    Text(model.pairingPayload.isEmpty ? "No Studio QR scanned" : "Studio QR captured")
                    Button("PAIR SCANNED STUDIO", action: model.pairScannedPayload)
                        .disabled(model.pairingPayload.isEmpty)
                }
                Section("WORKSPACE") {
                    Text(model.status)
                    Button("START LIVE SYNC", action: model.startSynchronization)
                    Button("STOP LIVE SYNC", action: model.stopSynchronization)
                    Button("RECONCILE") { Task { try await model.refresh() } }
                    ForEach(model.shots, id: \.shotID) { shot in
                        Button { model.select(shot) } label: {
                            HStack {
                                if let bytes = model.iconBytes[shot.shotID],
                                   let image = UIImage(data: bytes) {
                                    Image(uiImage: image)
                                        .resizable()
                                        .frame(width: 40, height: 40)
                                        .clipShape(RoundedRectangle(cornerRadius: 9))
                                }
                                VStack(alignment: .leading) {
                                    Text(shot.displayName)
                                    Text("\(shot.kind.rawValue) · Version \(shot.latestVersionOrdinal.map(String.init) ?? "none")")
                                        .font(.caption)
                                }
                            }
                        }
                    }
                }
                if let shot = model.selectedShot {
                    Section("EXACT VERSION · \(shot.latestVersionOrdinal.map(String.init) ?? "none")") {
                        TextField("Feedback body", text: $model.feedbackBody, axis: .vertical)
                        TextField("Feedback command ID", text: $model.feedbackCommandID)
                        Button("SUBMIT EXACT-VERSION FEEDBACK", action: model.submitFeedback)
                        TextField("Marketing body", text: $model.marketingBody, axis: .vertical)
                        TextField("Marketing command ID", text: $model.marketingCommandID)
                        Button("SUBMIT MARKETING NOTE", action: model.submitMarketing)
                        TextField("Evolution intention", text: $model.evolutionIntention, axis: .vertical)
                        TextField("Evolution command ID", text: $model.evolutionCommandID)
                        Button("EVOLVE FROM THIS VERSION", action: model.requestEvolution)
                    }
                }
                Section("NEW SHOT") {
                    TextField("Shot name", text: $model.creationName)
                    TextField("Creation intention", text: $model.creationIntention, axis: .vertical)
                    TextField("Creation command ID", text: $model.creationCommandID)
                    Button("CREATE SHOT", action: model.requestCreation)
                }
            }
            .navigationTitle("Companion Conformance")
            .sheet(isPresented: $scannerPresented) {
                NavigationStack {
                    PairingScannerView(
                        onScan: { payload in
                            model.acceptScannedPayload(payload)
                            scannerPresented = false
                        },
                        onFailure: { message in
                            model.status = message
                            scannerPresented = false
                        }
                    )
                    .ignoresSafeArea()
                    .navigationTitle("Scan Studio QR")
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Cancel") { scannerPresented = false }
                        }
                    }
                }
            }
        }
    }
}
