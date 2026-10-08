#if os(macOS)
import AppKit
import WebKit

/// A Browser tab's captured favicon (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/desktop/src/preview/FaviconCapture.ts `selectFaviconCandidates`, `captureFavicon`; Manager.ts
/// `faviconUpdated`, `publishFavicon`, `clearFavicon`). Chromium hands the reference the page's icon URLs
/// (`page-favicon-updated`); WebKit has no such event, so a user script in the app's own content world (the
/// page cannot see or call its handler) reads the document's `<link rel~=icon>` URLs, or `/favicon.ico` when
/// it declares none, after each load and whenever the head changes. The candidates are bounded as the
/// reference bounds them (eight at most, http(s) or an inline image, 2,048 characters for a URL), fetched
/// without following redirects, refused over 100,000 bytes, drawn into 32×32 and kept as a PNG data URL of at
/// most 8,192 characters for the page's origin. The reference fetches a same-origin icon with the tab's
/// cookies; this fetch sends none (an icon behind a sign-in falls back to the public favicon service, then the
/// globe). An icon belongs to its origin: a committed document of another origin drops it.
final class T3BrowserFavicon: NSObject, WKScriptMessageHandler, URLSessionTaskDelegate {
    static let maxCandidates = 8
    static let maxResponseBytes = 100_000
    static let maxHTTPURLLength = 2_048
    static let maxDataURLLength = 8_192
    static let handlerName = "t3BrowserFavicon"
    static let script = """
    (() => {
      const post = () => {
        try {
          const links = [...document.querySelectorAll('link[rel]')].filter(l => /(^|\\s)(icon|apple-touch-icon)(\\s|$)/i.test(l.rel)).map(l => l.href).filter(Boolean);
          const icons = links.length ? links : [new URL('/favicon.ico', location.href).href];
          window.webkit.messageHandlers.\(handlerName).postMessage({ page: location.href, icons });
        } catch (_) {}
      };
      post();
      if (document.head) new MutationObserver(post).observe(document.head, { childList: true, subtree: true, attributes: true, attributeFilter: ['href', 'rel'] });
    })();
    """

    /// The icon and the origin it belongs to.
    private(set) var current: (dataUrl: String, pageUrl: String, capturedAt: Double)?
    var captured: (() -> Void)?
    var currentURL: (() -> URL?)?
    private var lastKey = ""
    private var task: URLSessionDataTask?
    private var request = 0
    private weak var controller: WKUserContentController?
    private lazy var session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: .main)

    /// The script message handler holds this object weakly: WKUserContentController retains it.
    private final class Relay: NSObject, WKScriptMessageHandler {
        weak var owner: T3BrowserFavicon?
        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) { owner?.userContentController(controller, didReceive: message) }
    }

    init(configuration: WKWebViewConfiguration) {
        super.init()
        let relay = Relay(); relay.owner = self
        let content = configuration.userContentController
        content.addUserScript(WKUserScript(source: Self.script, injectionTime: .atDocumentEnd, forMainFrameOnly: true, in: .defaultClient))
        content.add(relay, contentWorld: .defaultClient, name: Self.handlerName)
        controller = content
    }

    func cancel() {
        task?.cancel(); task = nil
        controller?.removeScriptMessageHandler(forName: Self.handlerName, contentWorld: .defaultClient)
        session.invalidateAndCancel()
    }

    /// Manager.ts clearFavicon: a document of another origin drops the old icon.
    func navigated(to url: URL?) {
        guard let current, Self.origin(url) != current.pageUrl else { return }
        self.current = nil
        lastKey = ""
        captured?()
    }

    /// The script runs at each load's end; nothing more to ask for here.
    func collect(in web: WKWebView) {}

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any], let page = body["page"] as? String, let raw = body["icons"] as? [Any] else { return }
        let candidates = Self.selectCandidates(raw.compactMap { $0 as? String })
        guard let origin = Self.origin(URL(string: page)), !candidates.isEmpty else { return }
        let key = ([page] + candidates).joined(separator: "\n")
        guard key != lastKey else { return }
        lastKey = key
        task?.cancel()
        request += 1
        fetch(candidates[...], origin: origin, request: request)
    }

    private func fetch(_ candidates: ArraySlice<String>, origin: String, request: Int) {
        guard request == self.request, let candidate = candidates.first else { return }
        let next = candidates.dropFirst()
        if let image = Self.inlineImage(candidate) { return finish(image, origin: origin, request: request, rest: next) }
        guard let url = URL(string: candidate) else { return fetch(next, origin: origin, request: request) }
        var urlRequest = URLRequest(url: url, cachePolicy: .returnCacheDataElseLoad, timeoutInterval: 5)
        urlRequest.httpShouldHandleCookies = false
        task = session.dataTask(with: urlRequest) { [weak self] data, response, _ in
            guard let self, request == self.request else { return }
            let ok = (response as? HTTPURLResponse).map { (200..<300).contains($0.statusCode) } ?? false
            guard ok, let data, data.count <= Self.maxResponseBytes, let image = NSImage(data: data) else { return self.fetch(next, origin: origin, request: request) }
            self.finish(image, origin: origin, request: request, rest: next)
        }
        task?.resume()
    }

    private func finish(_ image: NSImage, origin: String, request: Int, rest: ArraySlice<String>) {
        guard let dataUrl = Self.dataURL(image) else { return fetch(rest, origin: origin, request: request) }
        // publishFavicon: only while the page is still on that origin.
        guard Self.origin(currentURL?()) == origin else { return }
        current = (dataUrl, origin, Date().timeIntervalSince1970 * 1000)
        captured?()
    }

    /// No redirects (`redirect: "error"`).
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }

    /// selectFaviconCandidates: http(s) URLs up to 2,048 characters and inline images, deduplicated, eight at most.
    static func selectCandidates(_ candidates: [String]) -> [String] {
        var selected: [String] = [], seen = Set<String>()
        for candidate in candidates where supported(candidate) && !seen.contains(candidate) {
            seen.insert(candidate); selected.append(candidate)
            if selected.count == maxCandidates { break }
        }
        return selected
    }

    static func supported(_ url: String) -> Bool {
        if url.lowercased().hasPrefix("data:") { return url.range(of: "^data:image/[a-z0-9.+-]+(;[^,]*)?,", options: [.regularExpression, .caseInsensitive]) != nil && url.count <= 140_000 }
        guard let parsed = URL(string: url), ["http", "https"].contains(parsed.scheme?.lowercased() ?? "") else { return false }
        return url.count <= maxHTTPURLLength
    }

    static func origin(_ url: URL?) -> String? {
        guard let url, let scheme = url.scheme?.lowercased(), ["http", "https"].contains(scheme), let host = url.host else { return nil }
        let port = url.port.map { ":\($0)" } ?? ""
        return "\(scheme)://\(host.lowercased())\(port)"
    }

    static func inlineImage(_ candidate: String) -> NSImage? {
        guard candidate.lowercased().hasPrefix("data:"), let comma = candidate.firstIndex(of: ",") else { return nil }
        let header = candidate[..<comma], payload = String(candidate[candidate.index(after: comma)...])
        let data = header.lowercased().contains(";base64") ? Data(base64Encoded: payload) : payload.removingPercentEncoding.map { Data($0.utf8) }
        guard let data, data.count <= maxResponseBytes else { return nil }
        return NSImage(data: data)
    }

    /// The icon drawn into 32×32 (aspect kept, centred), as a PNG data URL within the reference's 8,192 characters.
    static func dataURL(_ image: NSImage) -> String? {
        let size = image.size
        guard size.width > 0, size.height > 0, let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 32, pixelsHigh: 32, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { return nil }
        let scale = min(32 / size.width, 32 / size.height), width = size.width * scale, height = size.height * scale
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        NSGraphicsContext.current?.imageInterpolation = .high
        image.draw(in: NSRect(x: (32 - width) / 2, y: (32 - height) / 2, width: width, height: height), from: .zero, operation: .copy, fraction: 1)
        NSGraphicsContext.restoreGraphicsState()
        guard let png = rep.representation(using: .png, properties: [:]) else { return nil }
        let url = "data:image/png;base64," + png.base64EncodedString()
        return url.count <= maxDataURLLength ? url : nil
    }
}
#endif
