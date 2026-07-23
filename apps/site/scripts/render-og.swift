import AppKit
import WebKit

enum RenderError: Error, CustomStringConvertible {
    case usage
    case imageEncoding

    var description: String {
        switch self {
        case .usage:
            return "usage: render-og.swift <input.html> <output.png>"
        case .imageEncoding:
            return "could not encode the rendered Open Graph card as PNG"
        }
    }
}

final class Renderer: NSObject, WKNavigationDelegate {
    private let inputURL: URL
    private let outputURL: URL
    private let webView: WKWebView

    init(inputURL: URL, outputURL: URL) {
        self.inputURL = inputURL
        self.outputURL = outputURL
        self.webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 1_200, height: 630))
        super.init()
        self.webView.navigationDelegate = self
    }

    func run() {
        webView.loadFileURL(
            inputURL,
            allowingReadAccessTo: inputURL.deletingLastPathComponent().deletingLastPathComponent()
        )
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        waitForFonts(attemptsRemaining: 50)
    }

    private func waitForFonts(attemptsRemaining: Int) {
        webView.evaluateJavaScript("document.fonts.status") { [self] result, error in
            guard error == nil else {
                fail(error!)
                return
            }

            guard result as? String == "loaded" else {
                guard attemptsRemaining > 0 else {
                    fail(RenderError.imageEncoding)
                    return
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                    self.waitForFonts(attemptsRemaining: attemptsRemaining - 1)
                }
                return
            }

            let configuration = WKSnapshotConfiguration()
            configuration.rect = webView.bounds
            // WKWebView snapshots use the current Retina backing scale. A
            // 600-point snapshot produces the required 1,200-pixel OG asset.
            configuration.snapshotWidth = 600

            webView.takeSnapshot(with: configuration) { image, error in
                if let error {
                    self.fail(error)
                    return
                }

                guard
                    let image,
                    let tiff = image.tiffRepresentation,
                    let representation = NSBitmapImageRep(data: tiff),
                    let png = representation.representation(using: .png, properties: [:])
                else {
                    self.fail(RenderError.imageEncoding)
                    return
                }

                do {
                    try png.write(to: self.outputURL, options: .atomic)
                    print(self.outputURL.path)
                    NSApplication.shared.terminate(nil)
                } catch {
                    self.fail(error)
                }
            }
        }
    }

    func webView(
        _ webView: WKWebView,
        didFail navigation: WKNavigation!,
        withError error: Error
    ) {
        fail(error)
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation navigation: WKNavigation!,
        withError error: Error
    ) {
        fail(error)
    }

    private func fail(_ error: Error) {
        FileHandle.standardError.write(Data("render-og: \(error)\n".utf8))
        exit(1)
    }
}

do {
    guard CommandLine.arguments.count == 3 else {
        throw RenderError.usage
    }

    let renderer = Renderer(
        inputURL: URL(fileURLWithPath: CommandLine.arguments[1]).standardizedFileURL,
        outputURL: URL(fileURLWithPath: CommandLine.arguments[2]).standardizedFileURL
    )
    renderer.run()
    NSApplication.shared.run()
} catch {
    FileHandle.standardError.write(Data("render-og: \(error)\n".utf8))
    exit(1)
}
