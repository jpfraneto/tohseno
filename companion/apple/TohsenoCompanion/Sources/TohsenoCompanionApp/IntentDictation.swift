import SwiftUI

#if os(iOS)
@preconcurrency import AVFoundation
@preconcurrency import Speech

@MainActor
@Observable
private final class IntentDictationController {
    var isListening = false
    var message: String?

    private let audioEngine = AVAudioEngine()
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var initialText = ""

    func toggle(currentText: String, update: @escaping @MainActor (String) -> Void) {
        if isListening {
            stop()
        } else {
            Task { await start(currentText: currentText, update: update) }
        }
    }

    func stop() {
        if audioEngine.isRunning {
            audioEngine.stop()
            audioEngine.inputNode.removeTap(onBus: 0)
        }
        recognitionRequest?.endAudio()
        recognitionTask?.cancel()
        recognitionTask = nil
        recognitionRequest = nil
        isListening = false
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private func start(
        currentText: String,
        update: @escaping @MainActor (String) -> Void
    ) async {
        message = nil

        guard await speechPermission() else {
            message = "Allow Speech Recognition in Settings to dictate an intent."
            return
        }
        guard await AVAudioApplication.requestRecordPermission() else {
            message = "Allow Microphone access in Settings to dictate an intent."
            return
        }
        guard let recognizer = SFSpeechRecognizer(), recognizer.isAvailable else {
            message = "Speech recognition is unavailable right now."
            return
        }

        stop()
        initialText = currentText.trimmingCharacters(in: .whitespacesAndNewlines)

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        recognitionRequest = request

        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.record, mode: .measurement, options: .duckOthers)
            try session.setActive(true, options: .notifyOthersOnDeactivation)

            let input = audioEngine.inputNode
            let format = input.outputFormat(forBus: 0)
            input.installTap(onBus: 0, bufferSize: 1_024, format: format) { buffer, _ in
                request.append(buffer)
            }
            audioEngine.prepare()
            try audioEngine.start()
            isListening = true

            recognitionTask = recognizer.recognitionTask(with: request) { [weak self] result, error in
                Task { @MainActor in
                    guard let self else { return }
                    if let result {
                        let spoken = result.bestTranscription.formattedString
                        let separator = self.initialText.isEmpty || spoken.isEmpty ? "" : " "
                        update(self.initialText + separator + spoken)
                        if result.isFinal { self.stop() }
                    } else if error != nil {
                        self.message = "I couldn't hear that. Tap the microphone to try again."
                        self.stop()
                    }
                }
            }
        } catch {
            message = "The microphone couldn't start. Tap it to try again."
            stop()
        }
    }

    private func speechPermission() async -> Bool {
        switch SFSpeechRecognizer.authorizationStatus() {
        case .authorized:
            true
        case .notDetermined:
            await withCheckedContinuation { continuation in
                SFSpeechRecognizer.requestAuthorization { status in
                    continuation.resume(returning: status == .authorized)
                }
            }
        case .denied, .restricted:
            false
        @unknown default:
            false
        }
    }
}
#endif

struct IntentEditor: View {
    @Binding var text: String
    let placeholder: String
    let minimumHeight: CGFloat

#if os(iOS)
    @State private var dictation = IntentDictationController()
#endif

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .topLeading) {
                if text.isEmpty {
                    Text(placeholder)
                        .font(.system(size: 17))
                        .foregroundStyle(Tohseno.ash)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 20)
                        .allowsHitTesting(false)
                }
                TextEditor(text: $text)
                    .scrollContentBackground(.hidden)
                    .font(.system(size: 17))
                    .foregroundStyle(Tohseno.bone)
                    .padding(12)
                    .padding(.bottom, 50)

#if os(iOS)
                VStack {
                    Spacer()
                    HStack {
                        Spacer()
                        Button {
                            dictation.toggle(currentText: text) { text = $0 }
                        } label: {
                            Image(systemName: dictation.isListening ? "stop.fill" : "mic.fill")
                                .font(.system(size: 17, weight: .semibold))
                                .foregroundStyle(dictation.isListening ? Tohseno.void : Tohseno.bone)
                                .frame(width: 44, height: 44)
                                .background(
                                    dictation.isListening ? Tohseno.orange : Tohseno.iron,
                                    in: Circle()
                                )
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(dictation.isListening ? "Stop listening" : "Speak intent")
                    }
                    .padding(12)
                }
#endif
            }
            .frame(minHeight: minimumHeight)
            .background(Tohseno.carbon.opacity(0.94), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .strokeBorder(text.isEmpty ? Tohseno.iron : Tohseno.orange, lineWidth: 1)
            )

#if os(iOS)
            if dictation.isListening {
                Label("Listening…", systemImage: "waveform")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Tohseno.orange)
            } else if let message = dictation.message {
                Text(message)
                    .font(.system(size: 13))
                    .foregroundStyle(Tohseno.ash)
                    .fixedSize(horizontal: false, vertical: true)
            }
#endif
        }
#if os(iOS)
        .onDisappear { dictation.stop() }
#endif
    }
}
