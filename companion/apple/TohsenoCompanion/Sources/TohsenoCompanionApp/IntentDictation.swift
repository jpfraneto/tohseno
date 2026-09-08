import SwiftUI

#if os(iOS)
@preconcurrency import AVFoundation
@preconcurrency import Speech

@MainActor
@Observable
private final class IntentDictationController {
    var isListening = false
    var isStarting = false
    var message: String?

    private let audioEngine = AVAudioEngine()
    private var tapInstalled = false
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var generation = UUID()

    func toggle(currentText: String, update: @escaping @MainActor (String) -> Void) {
        if isListening || isStarting {
            stop()
        } else {
            isStarting = true
            message = nil
            let run = UUID()
            generation = run
            Task { await start(currentText: currentText, run: run, update: update) }
        }
    }

    func stop() {
        generation = UUID()
        audioEngine.stop()
        if tapInstalled {
            audioEngine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        recognitionRequest?.endAudio()
        recognitionTask?.cancel()
        recognitionTask = nil
        recognitionRequest = nil
        isListening = false
        isStarting = false
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private func start(currentText: String, run: UUID, update: @escaping @MainActor (String) -> Void) async {
        let speechAllowed = await Self.speechPermission()
        guard generation == run else { return }
        guard speechAllowed else {
            message = "Allow Speech Recognition in Settings to dictate."
            isStarting = false
            return
        }
        let microphoneAllowed = await AVAudioApplication.requestRecordPermission()
        guard generation == run else { return }
        guard microphoneAllowed else {
            message = "Allow Microphone access in Settings to dictate."
            isStarting = false
            return
        }
        guard let recognizer = SFSpeechRecognizer(), recognizer.isAvailable else {
            message = "Speech recognition is unavailable right now. You can keep typing."
            isStarting = false
            return
        }
        let initialText = currentText.trimmingCharacters(in: .whitespacesAndNewlines)
        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        if recognizer.supportsOnDeviceRecognition { request.requiresOnDeviceRecognition = true }
        recognitionRequest = request
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.record, mode: .measurement, options: .duckOthers)
            try session.setActive(true)
            let input = audioEngine.inputNode
            let format = input.outputFormat(forBus: 0)
            guard format.sampleRate > 0, format.channelCount > 0 else {
                message = "No microphone input is available. You can keep typing."
                stop()
                return
            }
            input.installTap(onBus: 0, bufferSize: 1_024, format: format, block: Self.audioTap(request))
            tapInstalled = true
            audioEngine.prepare()
            try audioEngine.start()
            isStarting = false
            isListening = true
            recognitionTask = Self.recognize(recognizer, request: request) { [weak self] spoken, final, failed in
                Task { @MainActor in
                    guard let self, self.generation == run else { return }
                    if let spoken {
                        let separator = initialText.isEmpty || spoken.isEmpty ? "" : " "
                        update(initialText + separator + spoken)
                    }
                    if failed { self.message = "Dictation stopped. Your text is saved; tap the microphone to continue." }
                    if final || failed { self.stop() }
                }
            }
        } catch {
            message = "The microphone couldn’t start. Your text is saved; try again."
            stop()
        }
    }

    // Apple calls these closures on its own queues. Construct them outside
    // MainActor so Swift 6 does not insert a main-executor assertion at entry.
    nonisolated private static func audioTap(_ request: SFSpeechAudioBufferRecognitionRequest) -> AVAudioNodeTapBlock {
        { buffer, _ in request.append(buffer) }
    }

    nonisolated private static func recognize(
        _ recognizer: SFSpeechRecognizer, request: SFSpeechAudioBufferRecognitionRequest,
        update: @escaping @Sendable (String?, Bool, Bool) -> Void
    ) -> SFSpeechRecognitionTask {
        recognizer.recognitionTask(with: request) { result, error in
            update(result?.bestTranscription.formattedString, result?.isFinal ?? false, error != nil)
        }
    }

    nonisolated private static func speechPermission() async -> Bool {
        switch SFSpeechRecognizer.authorizationStatus() {
        case .authorized: true
        case .notDetermined:
            await withCheckedContinuation { continuation in
                SFSpeechRecognizer.requestAuthorization { status in
                    continuation.resume(returning: status == .authorized)
                }
            }
        case .denied, .restricted: false
        @unknown default: false
        }
    }
}
#endif

struct IntentEditor: View {
    @Binding var text: String
    let placeholder: String
    let minimumHeight: CGFloat
    var submissionInProgress = false
    @FocusState private var editing: Bool
    @Environment(\.scenePhase) private var scenePhase

#if os(iOS)
    @State private var dictation = IntentDictationController()
#endif

    private var recording: Bool {
#if os(iOS)
        dictation.isListening || dictation.isStarting
#else
        false
#endif
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .topLeading) {
                if text.isEmpty {
                    Text(recording ? "Speak now…" : placeholder)
                        .font(.system(size: 17))
                        .foregroundStyle(Tohseno.ash)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 20)
                        .allowsHitTesting(false)
                }
                TextEditor(text: $text)
                    .focused($editing)
                    .scrollContentBackground(.hidden)
                    .font(.system(size: recording ? 24 : 19))
                    .allowsHitTesting(!recording)
                    .foregroundStyle(Tohseno.bone)
                    .padding(12)
                    .padding(.bottom, 68)

#if os(iOS)
                VStack {
                    Spacer()
                    HStack {
                        if dictation.isListening || dictation.isStarting {
                            Label(dictation.isStarting ? "Starting…" : "Listening…", systemImage: "waveform")
                                .font(.headline).foregroundStyle(Tohseno.orange)
                                .accessibilityAddTraits(.updatesFrequently)
                        }
                        Spacer()
                        Button {
                            editing = false
                            dictation.toggle(currentText: text) { text = $0 }
                        } label: {
                            Image(systemName: dictation.isListening || dictation.isStarting ? "stop.fill" : "mic.fill")
                                .font(.system(size: 17, weight: .semibold))
                                .foregroundStyle(dictation.isListening ? Tohseno.void : Tohseno.bone)
                                .frame(width: 56, height: 56)
                                .background(
                                    dictation.isListening ? Tohseno.orange : Tohseno.iron,
                                    in: Circle()
                                )
                        }
                        .buttonStyle(.plain)
                        .disabled(submissionInProgress)
                        .accessibilityLabel(recording ? "Stop listening" : "Speak intent")
                    }
                    .padding(12)
                }
#endif
            }
            .frame(minHeight: minimumHeight)
            .background(recording ? Tohseno.orange.opacity(0.08) : Tohseno.carbon.opacity(0.94), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .strokeBorder(recording || !text.isEmpty ? Tohseno.orange : Tohseno.iron, lineWidth: recording ? 2 : 1)
            )

#if os(iOS)
            if let message = dictation.message {
                Text(message)
                    .font(.system(size: 13))
                    .foregroundStyle(Tohseno.ash)
                    .fixedSize(horizontal: false, vertical: true)
            }
#endif
        }
#if os(iOS)
        .onDisappear { dictation.stop() }
        .onChange(of: submissionInProgress) { _, submitting in
            if submitting { dictation.stop() }
        }
        .onChange(of: scenePhase) { _, phase in
            if phase != .active && dictation.isListening { dictation.stop() }
        }
#endif
    }
}
