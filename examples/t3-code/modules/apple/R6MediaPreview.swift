#if os(macOS)
import AppKit
import PDFKit
import WebKit

/// Lane r6-media: the attachment preview's PDF and HTML bodies (MIT reference, see
/// LICENSE-T3: components/files/BrowserDocumentFrame.tsx). Hook `t3-media` on the body's
/// box, with `data-media-kind` ("pdf" or "html"), `data-media-url` (the signed inline asset
/// URL the surface minted) and `data-media-name`.
///
/// - PDF: the reference asks Chromium's viewer for the page alone, fitted to the panel width
///   (`#toolbar=0&view=FitH`); here a PDFView scales the continuous pages to the width, and
///   scrolling, zoom gestures, selection and find work inside it.
/// - HTML: the reference runs the page in `<iframe sandbox="allow-scripts allow-forms
///   allow-popups allow-modals">` at its signed asset URL, an opaque origin that cannot reach
///   the app's session or storage. Lane r12-render: here the same element does it. The web
///   view's document is a script-free wrapper at the server's origin whose only content is
///   that iframe, so WebKit applies the frame's own sandbox, and the page is fetched from the
///   asset URL with the server's own headers (`Content-Security-Policy: sandbox allow-scripts
///   allow-forms allow-popups allow-modals`, no fetch directive). As in the reference, the
///   page may load stylesheets, scripts, images, media, fonts and fetches from any host (its
///   siblings resolve through the token directory), a link navigates the frame in place (the
///   desktop renderer's `frame-src 'self' blob: http: https:`), the page cannot navigate the
///   window (no allow-top-navigation), and a new window (`target="_blank"`, a clicked
///   `window.open`) opens an http(s) URL in the default browser (never under the agent; the
///   desktop's setWindowOpenHandler and parseSafeExternalUrl). Alerts and confirms show as
///   sheets; `prompt()` returns null, as in Electron. A non-persistent data store.
final class R6MediaPreview: NSObject, WKNavigationDelegate, WKUIDelegate {
    static let maximumBytes = 64 * 1024 * 1024
    /// The iframe's sandbox, as BrowserDocumentFrame.tsx sets it.
    static let frameSandbox = "allow-scripts allow-forms allow-popups allow-modals"
    /// The wrapper's own policy: no script of its own; the frame may hold what the reference
    /// renderer's `frame-src` admits.
    static let wrapperPolicy = "default-src 'none'; style-src 'unsafe-inline'; frame-src 'self' blob: http: https:; base-uri 'none'; form-action 'none'"
    /// Schemes a frame inside the preview may navigate to (`frame-src`, plus about:blank / srcdoc).
    static let frameSchemes: Set<String> = ["http", "https", "blob", "about"]

    /// The wrapper's URL: the asset's origin (so the page's secure-context status follows its own
    /// URL, as under the desktop renderer), at a path that is never fetched.
    static func wrapperURL(for asset: URL) -> URL? {
        guard var parts = URLComponents(url: asset, resolvingAgainstBaseURL: false) else { return nil }
        parts.percentEncodedPath = "/api/assets/"; parts.query = nil; parts.fragment = nil; parts.user = nil; parts.password = nil
        return parts.url
    }

    /// The wrapper document: the frame, filling the view, on the reference's `bg-white`.
    static func wrapperHTML(src: URL, title: String) -> String {
        func attribute(_ value: String) -> String {
            value.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "\"", with: "&quot;")
                .replacingOccurrences(of: "<", with: "&lt;").replacingOccurrences(of: ">", with: "&gt;")
        }
        return "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\">"
            + "<style>html,body{margin:0;height:100%;overflow:hidden;background:#fff}iframe{display:block;box-sizing:border-box;border:0;width:100%;height:100%;background:#fff}</style>"
            + "</head><body><iframe sandbox=\"\(frameSandbox)\" src=\"\(attribute(src.absoluteString))\" title=\"\(attribute(title))\"></iframe></body></html>"
    }

    private final class Entry {
        weak var host: NSView?
        let identity: String
        let view: NSView
        var task: URLSessionDataTask?
        var document: URL?
        /// The preview frame (the iframe), once WebKit has asked to load it.
        var frame: WKFrameInfo?
        var asset: URL?
        init(host: NSView, identity: String, view: NSView) { self.host = host; self.identity = identity; self.view = view }
    }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private let agent: Bool
    private let session: URLSession
    /// What the bodies did (state.presentation for the agent): loaded documents, refused loads.
    private(set) var loaded: [String: String] = [:]
    private(set) var refused: [String] = []

    init(agent: Bool) {
        self.agent = agent
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        session = URLSession(configuration: configuration)
    }

    var status: [String: Any] { ["mediaViews": entries.count, "mediaLoaded": loaded, "mediaRefused": Array(refused.suffix(8))] }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Media, let host = element.view else { return }
        mount(host: host, kind: element.data[.mediaKind] ?? "", raw: element.data[.mediaUrl] ?? "", name: element.data[.mediaName] ?? "")
    }

    /// The body for one hooked box (also what the AppKit test drives).
    func mount(host: NSView, kind: String, raw: String, name: String) {
        let key = ObjectIdentifier(host), identity = "\(kind)\n\(raw)"
        if entries[key]?.identity == identity { return }
        detach(key)
        guard let url = T3AttachmentFiles.assetURL(raw), kind == "pdf" || kind == "html" else { return }
        let view: NSView = kind == "pdf" ? Self.pdfView(name: name) : webView(name: name)
        view.frame = host.bounds
        view.autoresizingMask = [.width, .height]
        host.addSubview(view)
        let entry = Entry(host: host, identity: identity, view: view)
        entries[key] = entry
        entry.task = session.dataTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 60)) { [weak self, weak entry] data, response, error in
            let http = response as? HTTPURLResponse
            let bytes = error == nil && http.map { (200..<300).contains($0.statusCode) } == true ? data : nil
            DispatchQueue.main.async {
                guard let self, let entry, self.entries[key] === entry else { return }
                entry.task = nil
                guard let bytes, bytes.count <= Self.maximumBytes else { return self.fail(entry, "Could not load this file.") }
                if kind == "pdf" { self.showPDF(entry, bytes, name: name) } else { self.showPage(entry, name: name, asset: url) }
            }
        }
        entry.task?.resume()
    }

    func remove(_ element: ExactElement) {
        if let host = element.view { detach(ObjectIdentifier(host)) }
        for (key, entry) in entries where entry.host == nil { entries[key] = nil; release(entry) }
    }

    func destroy() { for key in Array(entries.keys) { detach(key) } }

    func unmount(host: NSView) { detach(ObjectIdentifier(host)) }

    private func detach(_ key: ObjectIdentifier) {
        guard let entry = entries.removeValue(forKey: key) else { return }
        release(entry)
    }

    private func release(_ entry: Entry) {
        entry.task?.cancel()
        if let web = entry.view as? WKWebView { web.stopLoading(); web.navigationDelegate = nil; web.uiDelegate = nil }
        entry.view.removeFromSuperview()
    }

    // MARK: PDF

    static func pdfView(name: String) -> PDFView {
        let view = PDFView()
        view.displayMode = .singlePageContinuous
        view.displaysPageBreaks = true
        view.autoScales = true
        // Chromium's PDF viewer surface (#282828, measured on the reference) around and between the pages.
        view.backgroundColor = NSColor(srgbRed: 0x28 / 255, green: 0x28 / 255, blue: 0x28 / 255, alpha: 1)
        view.setAccessibilityLabel(name)
        return view
    }

    private func showPDF(_ entry: Entry, _ bytes: Data, name: String) {
        guard let pdf = entry.view as? PDFView, let document = PDFDocument(data: bytes) else { return fail(entry, "Could not load this file.") }
        pdf.document = document
        pdf.autoScales = true
        loaded[name] = "pdf:\(document.pageCount)"
    }

    // MARK: HTML

    private func webView(name: String) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        // The page keeps painting and running while the window is covered (as the reference's frame does).
        if #available(macOS 14.0, *) { configuration.preferences.inactiveSchedulingPolicy = .none }
        let web = WKWebView(frame: .zero, configuration: configuration)
        web.navigationDelegate = self
        web.uiDelegate = self
        // A page that declares no color-scheme renders light in a dark window, as the frame does.
        web.appearance = NSAppearance(named: .aqua)
        web.underPageBackgroundColor = .white
        web.setAccessibilityLabel(name)
        return web
    }

    /// The wrapper's response: an HTML document under the wrapper policy.
    static func wrapperResponse(url: URL) -> HTTPURLResponse {
        HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: [
            "Content-Type": "text/html; charset=utf-8",
            "Content-Security-Policy": wrapperPolicy,
            "Cache-Control": "no-store",
        ])!
    }

    private func showPage(_ entry: Entry, name: String, asset: URL) {
        guard let web = entry.view as? WKWebView, let url = Self.wrapperURL(for: asset) else { return fail(entry, "Could not load this file.") }
        entry.document = url; entry.asset = asset
        web.loadSimulatedRequest(URLRequest(url: url), response: Self.wrapperResponse(url: url), responseData: Data(Self.wrapperHTML(src: asset, title: name).utf8))
        loaded[name] = "html"
    }

    /// The preview frame of a mounted web view (the AppKit test evaluates the page in it).
    func documentFrame(of web: WKWebView) -> WKFrameInfo? { entry(for: web)?.frame }

    private func entry(for web: WKWebView) -> Entry? { entries.values.first { $0.view === web } }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let target = navigationAction.request.url, entry = entry(for: webView)
        if let frame = navigationAction.targetFrame, !frame.isMainFrame {
            // Inside the preview: the frame (and frames in it) navigate in place, as the reference's iframe does.
            if let target, Self.frameSchemes.contains(target.scheme?.lowercased() ?? "") {
                if let entry, entry.frame == nil, target == entry.asset { entry.frame = frame }
                return decisionHandler(.allow)
            }
            decisionHandler(.cancel)
            if let target { refused.append(target.absoluteString) }
            return
        }
        // The window shows the wrapper and nothing else (the sandbox has no allow-top-navigation).
        if let document = entry?.document, let target, Self.sameDocument(target, document) {
            return decisionHandler(.allow)
        }
        decisionHandler(.cancel)
        guard let target else { return }
        refused.append(target.absoluteString)
    }

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        // allow-popups: WebKit asks only for a window the person opened (javaScriptCanOpenWindowsAutomatically
        // is off); the desktop's setWindowOpenHandler opens an http(s) URL externally and denies the window.
        if let target = navigationAction.request.url {
            refused.append(target.absoluteString)
            openExternally(target)
        }
        return nil
    }

    func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        guard let window = webView.window else { return completionHandler() }
        let alert = NSAlert()
        alert.messageText = message
        alert.addButton(withTitle: "OK")
        alert.beginSheetModal(for: window) { _ in completionHandler() }
    }

    func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
        guard let window = webView.window else { return completionHandler(false) }
        let alert = NSAlert()
        alert.messageText = message
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { completionHandler($0 == .alertFirstButtonReturn) }
    }

    static func sameDocument(_ target: URL, _ document: URL) -> Bool {
        var a = URLComponents(url: target, resolvingAgainstBaseURL: false), b = URLComponents(url: document, resolvingAgainstBaseURL: false)
        a?.fragment = nil; b?.fragment = nil
        return a?.url == b?.url
    }

    private func openExternally(_ url: URL) {
        guard !agent, let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return }
        NSWorkspace.shared.open(url)
    }

    // MARK: Failure

    private func fail(_ entry: Entry, _ message: String) {
        let label = NSTextField(labelWithString: message)
        label.font = .systemFont(ofSize: T3RootFont.rem(12))
        label.textColor = NSColor(srgbRed: 0x71 / 255, green: 0x71 / 255, blue: 0x7b / 255, alpha: 1)
        label.alignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        entry.view.addSubview(label)
        NSLayoutConstraint.activate([label.centerXAnchor.constraint(equalTo: entry.view.centerXAnchor), label.centerYAnchor.constraint(equalTo: entry.view.centerYAnchor)])
        refused.append("load-failed")
    }
}
#endif
