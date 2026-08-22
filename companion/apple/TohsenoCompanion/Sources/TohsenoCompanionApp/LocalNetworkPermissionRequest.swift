#if DEBUG && os(iOS)
import Foundation
import Network

/// Makes iOS ask for the local-network permission used by physical-device
/// development pairing. The production Companion uses the HTTPS relay and
/// never constructs this helper.
@MainActor
final class LocalNetworkPermissionRequest {
    private var browser: NWBrowser?
    private var timeout: Task<Void, Never>?

    func begin() {
        guard browser == nil else { return }

        let browser = NWBrowser(
            for: .bonjour(type: "_tohseno._tcp", domain: nil),
            using: .tcp
        )
        browser.stateUpdateHandler = { [weak self] state in
            NSLog("TOHSENO local-network permission request: %@", String(describing: state))
            switch state {
            case .ready, .failed:
                Task { @MainActor [weak self] in self?.finish() }
            default:
                break
            }
        }
        self.browser = browser
        browser.start(queue: .main)

        timeout = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(30))
            guard !Task.isCancelled else { return }
            self?.finish()
        }
    }

    private func finish() {
        browser?.cancel()
        browser = nil
        timeout?.cancel()
        timeout = nil
    }
}
#endif
