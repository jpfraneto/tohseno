#if os(iOS)
@preconcurrency import AVFoundation
import SwiftUI
import UIKit

struct PairingScannerView: UIViewControllerRepresentable {
    let onScan: @MainActor (String) -> Void
    let onFailure: @MainActor (String) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onScan: onScan)
    }

    func makeUIViewController(context: Context) -> PairingScannerViewController {
        PairingScannerViewController(
            delegate: context.coordinator,
            onFailure: onFailure
        )
    }

    func updateUIViewController(
        _ viewController: PairingScannerViewController,
        context: Context
    ) {}

    @MainActor
    final class Coordinator: NSObject, AVCaptureMetadataOutputObjectsDelegate {
        private let onScan: @MainActor (String) -> Void
        private var completed = false

        init(onScan: @escaping @MainActor (String) -> Void) {
            self.onScan = onScan
        }

        nonisolated func metadataOutput(
            _ output: AVCaptureMetadataOutput,
            didOutput metadataObjects: [AVMetadataObject],
            from connection: AVCaptureConnection
        ) {
            guard let payload = metadataObjects
                .compactMap({ $0 as? AVMetadataMachineReadableCodeObject })
                .first(where: { $0.type == .qr })?
                .stringValue
            else { return }
            Task { @MainActor [weak self] in
                guard let self, !completed else { return }
                completed = true
                onScan(payload)
            }
        }
    }
}

@MainActor
final class PairingScannerViewController: UIViewController {
    private let session = AVCaptureSession()
    private let metadataDelegate: AVCaptureMetadataOutputObjectsDelegate
    private let onFailure: @MainActor (String) -> Void
    private var preview: AVCaptureVideoPreviewLayer?

    init(
        delegate: AVCaptureMetadataOutputObjectsDelegate,
        onFailure: @escaping @MainActor (String) -> Void
    ) {
        metadataDelegate = delegate
        self.onFailure = onFailure
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configureAfterAuthorization()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if session.isRunning { session.stopRunning() }
    }

    private func configureAfterAuthorization() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureSession()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                Task { @MainActor in
                    guard let self else { return }
                    if granted {
                        self.configureSession()
                    } else {
                        self.onFailure("Camera access was denied.")
                    }
                }
            }
        case .denied, .restricted:
            onFailure("Camera access is unavailable.")
        @unknown default:
            onFailure("Camera authorization could not be determined.")
        }
    }

    private func configureSession() {
        do {
            guard let camera = AVCaptureDevice.default(for: .video) else {
                onFailure("No camera is available.")
                return
            }
            let input = try AVCaptureDeviceInput(device: camera)
            let output = AVCaptureMetadataOutput()
            guard session.canAddInput(input), session.canAddOutput(output) else {
                onFailure("The QR capture session could not be configured.")
                return
            }
            session.beginConfiguration()
            session.addInput(input)
            session.addOutput(output)
            output.setMetadataObjectsDelegate(metadataDelegate, queue: .main)
            output.metadataObjectTypes = [.qr]
            session.commitConfiguration()

            let preview = AVCaptureVideoPreviewLayer(session: session)
            preview.videoGravity = .resizeAspectFill
            view.layer.insertSublayer(preview, at: 0)
            self.preview = preview
            preview.frame = view.bounds
            session.startRunning()
        } catch {
            onFailure("The camera could not start.")
        }
    }
}
#endif
