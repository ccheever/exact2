// The Apple iframe arm: WebKit stays in this dylib, loaded at the first
// iframe commit (@ref LLP 1020 D2/D3). The presenters see only its C ABI.
import Foundation
import UniformTypeIdentifiers
import WebKit

#if os(macOS)
import AppKit
private typealias PlatformImage = NSImage
/// WKWebView in a `fullSizeContentView` window otherwise inherits the
/// titlebar as a safe area and insets the guest by it — a black strip the
/// height of the titlebar over the deck. The kernel already framed this
/// box; the page fills it. @ref LLP 1020 D1
private final class ExactWebView: WKWebView {
    override var safeAreaInsets: NSEdgeInsets { NSEdgeInsetsZero }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        pinInsets()
    }
    override func layout() {
        super.layout()
        pinInsets()
    }
    func pinInsets() {
        setValue(false, forKey: "automaticallyAdjustsContentInsets")
        func walk(_ v: NSView) {
            if let s = v as? NSScrollView {
                if s.automaticallyAdjustsContentInsets { s.automaticallyAdjustsContentInsets = false }
                if s.contentInsets.top != 0 || s.contentInsets.left != 0 || s.contentInsets.bottom != 0 || s.contentInsets.right != 0 {
                    s.contentInsets = NSEdgeInsetsZero
                }
            }
            v.subviews.forEach(walk)
        }
        walk(self)
    }
}
#else
import UIKit
private typealias PlatformImage = UIImage
#endif

public typealias EventFn = @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32
) -> Void
public typealias ReplyFn = @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32
) -> Void

private final class WebArm: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    let id: UInt32
    let context: UnsafeMutableRawPointer?
    let event: EventFn
    let reply: ReplyFn
    let controller = WKUserContentController()
    let world = WKContentWorld.world(name: "exact.agent")
    let webView: WKWebView
    var src: String?
    /// A local `src`'s bytes, as the host read them (`exact_web_set_document`).
    var suppliedDocument: Data?
    var sandbox: String?
    var srcInitialized = false
    var sandboxInitialized = false
    var servePending = false
    var serving = false
    var generation = 0
    var guestFrame: WKFrameInfo?
    var suppressLoad = false
    var recovering = false
    var invalidated = false
    /// The guest is the web view's own document, not a frame in a wrapper
    /// (see `serve`).
    var direct = false
    /// A direct guest navigated itself: its later documents' messages are
    /// not the app's (the wrapper's `navigated`).
    var revoked = false
    /// The origin a direct guest's messages must come from.
    var expectedOrigin: String?
    var pageBridge = false
    /// The guest is a bundled page: its failed sub-resources are logged.
    var localGuest = false
    /// A bundled page's origin: loopback HTTP, a secure context (as the web
    /// dev loop's `http://127.0.0.1` page is) that WebKit holds to no
    /// mixed-content blocking. Under an `https:` origin WebKit refuses every
    /// `http:` sub-resource, loopback included, which Chrome loads (#135).
    static let localOrigin = "http://exact.localhost"
    static let template: WKWebViewConfiguration = {
        let template = WKWebViewConfiguration()
        _ = template.preferences
        _ = template.websiteDataStore
        _ = template.defaultWebpagePreferences
        return template
    }()
    var wrapperURL: URL

    init(id: UInt32, context: UnsafeMutableRawPointer?, event: @escaping EventFn, reply: @escaping ReplyFn) {
        self.id = id
        self.context = context
        self.event = event
        self.reply = reply
        self.wrapperURL = URL(string: "https://exact.invalid/frame/\(id)/index.html")!
        // A copy of one shared configuration, not a fresh one: a fresh
        // configuration builds its preferences, visited-link store and
        // page defaults again for every row (`ensureLazyInitializedRefs`,
        // 3–4 ms a view). The copy shares only what every iframe on one web
        // page shares: the default website data store (same-origin storage
        // and cookies) and default preferences. The script bridge stays per
        // view (@ref LLP 1020 D2, LLP 1068 §0.1 Q3: views are not pooled).
        let configuration = WebArm.template.copy() as! WKWebViewConfiguration
        configuration.userContentController = controller
        #if os(macOS)
        webView = ExactWebView(frame: .zero, configuration: configuration)
        #else
        webView = WKWebView(frame: .zero, configuration: configuration)
        #endif
        super.init()
        controller.add(self, contentWorld: world, name: "exactAgent")
        controller.add(self, contentWorld: world, name: "exactFrame")
        // A direct guest's `parent` is its own window: what it posts to
        // `parent` arrives as a message from itself. The agent world takes
        // it before the page's listeners, as an iframe's post never reaches
        // its own window, and hands it to the app (@ref LLP 1020 D2).
        controller.addUserScript(WKUserScript(
            source: "addEventListener('message', e => { if (e.source !== window) return; e.stopImmediatePropagation(); let p = e.data; if (typeof p !== 'string') { try { p = JSON.stringify(p) } catch { return } } if (typeof p === 'string') window.webkit.messageHandlers.exactFrame.postMessage(p) }, true)",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: true,
            in: world))
        controller.addUserScript(WKUserScript(
            source: "if (window.parent === window.top && window !== window.top) window.webkit.messageHandlers.exactAgent.postMessage('ready')",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: false,
            in: world))
        // A sub-resource WebKit refuses or fails to load fires `error` on its
        // element and is otherwise silent; for a bundled page the host logs it.
        controller.addUserScript(WKUserScript(
            source: "addEventListener('error', e => { const t = e.target; if (!(t instanceof Element)) return; const u = t.currentSrc || t.src || t.href; if (typeof u === 'string' && u) window.webkit.messageHandlers.exactAgent.postMessage({ failed: u }) }, true)",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: false,
            in: world))
        webView.navigationDelegate = self
        // The guest is a leaf the kernel already framed. WKWebView's default
        // is to inset itself for the titlebar / safe area, which leaves a
        // black strip of `underPageBackgroundColor` over the top of the
        // deck and over siblings (the account mark). Off: the iframe fills
        // the node's box (@ref LLP 1020 D1).
        #if os(macOS)
        webView.setValue(false, forKey: "drawsBackground")
        if #available(macOS 12.0, *) { webView.underPageBackgroundColor = .clear }
        (webView as? ExactWebView)?.pinInsets()
        #else
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.contentInset = .zero
        if #available(iOS 15.0, *) { webView.underPageBackgroundColor = .clear }
        #endif
    }

    deinit {
        invalidate()
    }

    func invalidate() {
        guard !invalidated else { return }
        invalidated = true
        webView.stopLoading()
        setPageBridge(false)
        controller.removeScriptMessageHandler(forName: "exactAgent", contentWorld: world)
        controller.removeScriptMessageHandler(forName: "exactFrame", contentWorld: world)
        webView.navigationDelegate = nil
    }

    func setSrc(_ value: String?) {
        guard !srcInitialized || src != value else { return }
        srcInitialized = true
        src = value
        markLoading()
        scheduleServe()
    }

    func setSandbox(_ value: String?) {
        guard !sandboxInitialized || sandbox != value else { return }
        sandboxInitialized = true
        sandbox = value
        markLoading()
        // Sandbox is immutable per mount in v1 (@ref LLP 1020 D2).
        scheduleServe()
    }

    func markLoading() {
        guestFrame = nil
        emit(kind: 3)
    }

    func scheduleServe() {
        guard !invalidated, !servePending else { return }
        servePending = true
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.invalidated, self.servePending else { return }
            self.servePending = false
            self.serve()
        }
    }

    /// One document per row, as a platform web view is used: a local
    /// document is the web view's own document at the synthetic origin
    /// (its `sandbox` as a CSP `sandbox` header, which WebKit enforces on
    /// a top-level document, opaque origin included), and an unsandboxed
    /// http(s) `src` is loaded as itself. Only a sandboxed remote `src`
    /// keeps the wrapper and its inner `<iframe>`: WebKit cannot sandbox a
    /// top-level network load, and approximating it is the fail-open
    /// class D2 removed (@ref LLP 1020 D2, §10).
    func serve(error: String? = nil) {
        guard !invalidated else { return }
        generation += 1
        guestFrame = nil
        serving = true
        revoked = false
        if error == nil, let file = src.flatMap(localFile) {
            serveFile(file.0, type: file.1)
            return
        }
        let local = error.map(errorDocument) ?? src.flatMap(localDocument)
        let remote = local == nil ? src.flatMap(remoteSource) : nil
        let web = remote.flatMap(URL.init(string:)).flatMap { ["http", "https"].contains($0.scheme?.lowercased() ?? "") ? $0 : nil }
        // An HTTPS wrapper would block an HTTP guest as mixed content even
        // when the app explicitly allows it through ATS. Match the remote
        // HTTP guest's scheme; WebKit still enforces ATS and the iframe's
        // sandbox. A local document is served at `localOrigin`.
        let origin = local != nil ? WebArm.localOrigin
            : web?.scheme?.lowercased() == "http" ? "http://exact.invalid" : "https://exact.invalid"
        wrapperURL = URL(string: "\(origin)/frame/\(id)/index.html")!
        localGuest = local != nil && error == nil
        #if os(iOS)
        // iOS lays a top-level document out by its viewport `<meta>` (980
        // CSS px without one); a frame ignores it and takes its box. Only a
        // document that asks for its box's width is laid out the same
        // either way, so only such a local document goes direct here.
        direct = local.map(WebArm.fitsItsBox) ?? (remote == nil)
        #else
        // macOS ignores the viewport `<meta>`, as a frame does.
        direct = local != nil || (web != nil && sandbox == nil) || (local == nil && remote == nil)
        #endif
        setPageBridge(!direct)
        guard direct else {
            let request = URLRequest(url: wrapperURL, cachePolicy: .reloadIgnoringLocalCacheData)
            webView.loadSimulatedRequest(request, responseHTML: wrapper(error: error))
            return
        }
        expectedOrigin = guestOrigin(remote: remote, local: local != nil)
        if let web, local == nil {
            webView.load(URLRequest(url: web))
            return
        }
        var headers = ["Content-Type": "text/html; charset=utf-8"]
        if let sandbox { headers["Content-Security-Policy"] = "sandbox \(sandbox)" }
        let response = HTTPURLResponse(url: wrapperURL, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!
        webView.loadSimulatedRequest(URLRequest(url: wrapperURL, cachePolicy: .reloadIgnoringLocalCacheData),
                                     response: response, responseData: Data((local ?? "").utf8))
    }

    /// A local file that is not text (a PDF, an image) is the web view's
    /// own document under its content type, as Chrome shows such
    /// a frame by the type its server sent: WebKit's PDF view for a PDF, not
    /// its bytes as HTML text (#115, @ref LLP 1020 §10).
    func serveFile(_ bytes: Data, type: String) {
        let url = URL(string: "\(WebArm.localOrigin)/frame/\(id)/index.html")!
        wrapperURL = url
        localGuest = true
        direct = true
        setPageBridge(false)
        expectedOrigin = guestOrigin(remote: nil, local: true)
        var headers = ["Content-Type": type.hasPrefix("text/") ? "\(type); charset=utf-8" : type]
        if let sandbox { headers["Content-Security-Policy"] = "sandbox \(sandbox)" }
        let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!
        webView.loadSimulatedRequest(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData), response: response, responseData: bytes)
    }

    /// Whether a document's viewport `<meta>` asks for the device width at
    /// scale 1: what a frame of its box gives it.
    static func fitsItsBox(_ html: String) -> Bool {
        guard let tag = html.range(of: #"<meta[^>]*name\s*=\s*["']?viewport["']?[^>]*>"#, options: [.regularExpression, .caseInsensitive]),
              let content = html[tag].range(of: #"content\s*=\s*("[^"]*"|'[^']*')"#, options: [.regularExpression, .caseInsensitive])
        else { return false }
        let pairs = html[tag][content].lowercased().filter { !$0.isWhitespace && $0 != "\"" && $0 != "'" }
            .dropFirst("content=".count).split(whereSeparator: { $0 == "," || $0 == ";" })
        var width: Substring?, scale: Substring?
        for pair in pairs {
            let kv = pair.split(separator: "=", maxSplits: 1)
            guard kv.count == 2 else { continue }
            if kv[0] == "width" { width = kv[1] } else if kv[0] == "initial-scale" { scale = kv[1] }
        }
        return width == "device-width" && (scale == nil || Double(scale!) == 1)
    }

    /// The wrapper's page-world bridge exists only while a wrapper is
    /// served: a direct guest is the main frame and must not reach it.
    func setPageBridge(_ on: Bool) {
        guard on != pageBridge else { return }
        pageBridge = on
        if on { controller.add(self, name: "exact") } else { controller.removeScriptMessageHandler(forName: "exact") }
    }

    /// An origin as `guestOrigin` spells it.
    static func spelled(_ o: WKSecurityOrigin) -> String {
        guard !o.protocol.isEmpty else { return "null" }
        let host = o.host.contains(":") ? "[\(o.host)]" : o.host
        let port = o.port == 0 || (o.protocol == "http" && o.port == 80) || (o.protocol == "https" && o.port == 443) ? "" : ":\(o.port)"
        return "\(o.protocol)://\(host)\(port)"
    }

    func wrapper(error: String?) -> String {
        let local = error.map(errorDocument) ?? src.flatMap(localDocument)
        let remote = local == nil ? src.flatMap(remoteSource) : nil
        let source = local.map { " srcdoc=\"\(attribute($0))\"" }
            ?? remote.map { " src=\"\(attribute($0))\"" }
            ?? ""
        let restriction = sandbox.map { " sandbox=\"\(attribute($0))\"" } ?? ""
        let expectedOrigin = javascript(guestOrigin(remote: remote, local: local != nil))
        return """
        <!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
        <style>html,body,iframe{margin:0;width:100%;height:100%;border:0;display:block}body{overflow:hidden}</style>
        <iframe id="exact-frame"\(source)\(restriction)></iframe>
        <script>
        (() => {
          const inner = document.getElementById('exact-frame');
          const guestOrigin = \(expectedOrigin);
          let committed = false;
          let navigated = false;
          window.__exactRevokeGuest = () => { navigated = true; };
          addEventListener('message', event => {
            if (event.source !== inner.contentWindow) return;
            if (navigated || guestOrigin === null || event.origin !== guestOrigin) return;
            let payload = event.data;
            if (typeof payload !== 'string') {
              try { payload = JSON.stringify(payload); } catch { return; }
            }
            if (typeof payload !== 'string') return;
            webkit.messageHandlers.exact.postMessage(JSON.stringify({kind:'message', generation:\(generation), payload}));
          });
          inner.addEventListener('load', () => {
            if (committed) navigated = true;
            committed = true;
            webkit.messageHandlers.exact.postMessage(JSON.stringify({kind:'load', generation:\(generation)}));
          });
        })();
        </script>
        """
    }

    func remoteSource(_ source: String) -> String? {
        guard let url = URL(string: source), let scheme = url.scheme?.lowercased() else { return nil }
        return ["http", "https", "data", "about", "blob"].contains(scheme) ? source : nil
    }

    func guestOrigin(remote: String?, local: Bool) -> String? {
        let tokens = Set((sandbox ?? "").split(whereSeparator: { $0.isWhitespace }).map(String.init))
        if sandbox != nil, !tokens.contains("allow-same-origin") { return "null" }
        if local { return WebArm.localOrigin }
        guard let remote, let url = URL(string: remote),
              let scheme = url.scheme?.lowercased(),
              (scheme == "http" || scheme == "https"),
              let host = url.host?.lowercased() else { return nil }
        let shownHost = host.contains(":") ? "[\(host)]" : host
        let defaultPort = scheme == "http" ? 80 : 443
        let port = url.port.flatMap { $0 == defaultPort ? nil : ":\($0)" } ?? ""
        return "\(scheme)://\(shownHost)\(port)"
    }

    func javascript(_ value: String?) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value as Any, options: .fragmentsAllowed),
              let text = String(data: data, encoding: .utf8) else { return "null" }
        return text.replacingOccurrences(of: "<", with: "\\u003c")
    }

    /// A local `src` whose file type is not text: its bytes and content type.
    /// Text (HTML, XHTML, XML, plain text) keeps the box-width document an
    /// iOS frame needs (`fitsItsBox`); no extension, or an unknown type, too.
    func localFile(_ source: String) -> (Data, String)? {
        let path = source.split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false)[0]
            .split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)[0]
        let ext = (String(path) as NSString).pathExtension
        guard !ext.isEmpty, let file = UTType(filenameExtension: ext), !file.conforms(to: .text),
              let type = file.preferredMIMEType?.lowercased(), let bytes = localBytes(source) else { return nil }
        return (bytes, type)
    }

    func localDocument(_ source: String) -> String? {
        localBytes(source).map { String(decoding: $0, as: UTF8.self) }
    }

    func localBytes(_ source: String) -> Data? {
        if let suppliedDocument { return suppliedDocument }
        // Hosted http(s) decks keep their URL. A scheme-less src — including
        // `URL(string:)` returning nil for a leading-dot relative path — is a
        // file under EXACT_ASSETS: an HTML one inlined as srcdoc, another
        // served under its type. Query and fragment
        // are URL metadata, not part of the filesystem path.
        if let scheme = URL(string: source)?.scheme?.lowercased(),
           scheme == "http" || scheme == "https" || scheme == "data" || scheme == "about" || scheme == "blob" {
            return nil
        }
        let path = source.split(separator: "?", maxSplits: 1)[0].split(separator: "#", maxSplits: 1)[0]
        let relative = path.drop(while: { $0 == "/" })
        let base = ProcessInfo.processInfo.environment["EXACT_ASSETS"]
            .map { URL(fileURLWithPath: $0) }
            ?? Bundle.main.resourceURL
            ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        // Only single-file fixtures materialize this way: a multi-file
        // bundle's subresources do not resolve under srcdoc. Hosted https
        // decks have a scheme and never enter this path.
        let root = base.resolvingSymlinksInPath().standardizedFileURL.path
        let file = base.appendingPathComponent(String(relative))
            .resolvingSymlinksInPath().standardizedFileURL
        guard file.path == root || file.path.hasPrefix(root.hasSuffix("/") ? root : root + "/") else { return nil }
        return try? Data(contentsOf: file)
    }

    /// What most likely refused a bundled page's sub-resource. The page sees
    /// only that it failed; ATS applies in an app bundle (not the bare
    /// executable the agent runs) to `http:` on a named, non-local host.
    static func whyFailed(_ source: String) -> String {
        guard let url = URL(string: source), url.scheme?.lowercased() == "http",
              let host = url.host?.lowercased(), host.contains("."), !host.hasSuffix(".local"), !host.hasSuffix(".localhost"),
              host.rangeOfCharacter(from: CharacterSet(charactersIn: "0123456789.:").inverted) != nil,
              Bundle.main.bundleURL.pathExtension == "app"
        else { return "the request failed; is its server running?" }
        let ats = Bundle.main.object(forInfoDictionaryKey: "NSAppTransportSecurity") as? [String: Any]
        if ats?["NSAllowsArbitraryLoadsInWebContent"] as? Bool == true || ats?["NSAllowsArbitraryLoads"] as? Bool == true {
            return "the request failed; is its server running?"
        }
        #if os(macOS)
        let os = "macos"
        #else
        let os = "ios"
        #endif
        return "App Transport Security refuses http: to a named host; app.json's host.\(os).appTransportSecurity can allow it"
    }

    func errorDocument(_ message: String) -> String {
        "<!doctype html><meta charset=utf-8><style>body{font:14px system-ui;padding:16px;color:#6b1d1d;background:#fff3f3}</style><p>\(attribute(message))</p>"
    }

    func attribute(_ value: String) -> String {
        value.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard !invalidated else { return }
        if message.name == "exactFrame" {
            // A direct guest's post to `parent`, from the document `src`
            // committed and not one it navigated to.
            guard direct, !revoked, message.frameInfo.isMainFrame, let payload = message.body as? String,
                  let expectedOrigin, WebArm.spelled(message.frameInfo.securityOrigin) == expectedOrigin else { return }
            emit(kind: 1, text: payload)
            return
        }
        if message.name == "exactAgent" {
            if let body = message.body as? [String: Any] {
                if localGuest, let failed = body["failed"] as? String {
                    FileHandle.standardError.write(Data("exact: iframe \(src ?? ""): \(failed) did not load (\(WebArm.whyFailed(failed)))\n".utf8))
                }
                return
            }
            if !message.frameInfo.isMainFrame, guestFrame == nil { guestFrame = message.frameInfo }
            return
        }
        guard message.name == "exact", message.frameInfo.isMainFrame,
              let text = message.body as? String,
              let data = text.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              object["generation"] as? Int == generation,
              let kind = object["kind"] as? String
        else { return }
        if kind == "message", let payload = object["payload"] as? String {
            emit(kind: 1, text: payload)
        } else if kind == "load" {
            emit(kind: 2)
            if suppressLoad { suppressLoad = false } else { emit(kind: 0) }
        }
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        guard !invalidated else { return }
        // Process death is a silent wrapper re-serve (@ref LLP 1020 §5).
        suppressLoad = true
        markLoading()
        serve()
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard !invalidated else { decisionHandler(.cancel); return }
        if direct {
            // The guest navigates itself, as a frame may; its later
            // documents are not the committed `src` (@ref LLP 1020 D2).
            if navigationAction.targetFrame?.isMainFrame == true, !serving {
                revoked = true
                guestFrame = nil
            }
            decisionHandler(.allow)
            return
        }
        guard navigationAction.targetFrame?.isMainFrame == true else {
            // Once the injected guest agent identified the committed child,
            // revoke its message channel at the start of any subsequent
            // child navigation, before the replacement document can run.
            if guestFrame != nil { webView.evaluateJavaScript("window.__exactRevokeGuest?.()") }
            decisionHandler(.allow)
            return
        }
        if serving, navigationAction.request.url == wrapperURL {
            decisionHandler(.allow)
            return
        }
        // An allow-top-navigation escape or wrapper self-reload damages the
        // topology. Only the navigation begun by `serve` may reach the main
        // frame; every other one restores the wrapper from its source string.
        decisionHandler(.cancel)
        markLoading()
        scheduleServe()
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        serving = false
        guard !invalidated else { return }
        guard !recovering else {
            recovering = false
            emit(kind: 2)
            emit(kind: 0)
            return
        }
        recovering = true
        markLoading()
        serve(error: error.localizedDescription)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        serving = false
        recovering = false
        guard direct, !invalidated else { return }
        // A direct guest's document loaded: the iframe's `load`.
        emit(kind: 2)
        if suppressLoad { suppressLoad = false } else { emit(kind: 0) }
    }

    func snapshot(token: UInt32) {
        guard webView.bounds.width > 0, webView.bounds.height > 0 else {
            sendReply(token: token, kind: 2, text: "iframe has no snapshot box")
            return
        }
        let configuration = WKSnapshotConfiguration()
        configuration.rect = webView.bounds
        configuration.afterScreenUpdates = true
        webView.takeSnapshot(with: configuration) { [weak self] image, error in
            guard let self else { return }
            guard let image, let data = self.png(image) else {
                self.sendReply(token: token, kind: 2, text: error?.localizedDescription ?? "WebKit returned no snapshot")
                return
            }
            self.sendReply(token: token, kind: 0, data: data)
        }
    }

    func evaluate(token: UInt32, script: String) {
        if direct {
            guard !serving else { sendReply(token: token, kind: 2, text: "guest frame is not ready"); return }
            webView.evaluateJavaScript(script, in: nil, in: world) { [weak self] result in self?.evaluated(token: token, result) }
            return
        }
        guard let guestFrame else {
            sendReply(token: token, kind: 2, text: "guest frame is not ready")
            return
        }
        webView.evaluateJavaScript(script, in: guestFrame, in: world) { [weak self] result in self?.evaluated(token: token, result) }
    }

    func evaluated(token: UInt32, _ result: Result<Any, Error>) {
        switch result {
        case .success(let value):
            if let text = value as? String { sendReply(token: token, kind: 1, text: text) }
            else if value is NSNull { sendReply(token: token, kind: 1, text: "null") }
            else { sendReply(token: token, kind: 1, text: String(describing: value)) }
        case .failure(let error):
            sendReply(token: token, kind: 2, text: error.localizedDescription)
        }
    }

    func png(_ image: PlatformImage) -> Data? {
        #if os(macOS)
        guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        return NSBitmapImageRep(cgImage: cg).representation(using: .png, properties: [:])
        #else
        return image.pngData()
        #endif
    }

    func emit(kind: UInt32, text: String = "") {
        guard !invalidated else { return }
        let data = Data(text.utf8)
        data.withUnsafeBytes { bytes in
            event(context, id, kind, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
        }
    }

    func sendReply(token: UInt32, kind: UInt32, text: String) {
        sendReply(token: token, kind: kind, data: Data(text.utf8))
    }

    func sendReply(token: UInt32, kind: UInt32, data: Data) {
        guard !invalidated else { return }
        data.withUnsafeBytes { bytes in
            reply(context, id, token, kind, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
        }
    }
}

private func arm(_ handle: UnsafeMutableRawPointer?) -> WebArm? {
    handle.map { Unmanaged<WebArm>.fromOpaque($0).takeUnretainedValue() }
}

@_cdecl("exact_web_create")
public func exactWebCreate(_ id: UInt32, _ context: UnsafeMutableRawPointer?, _ event: EventFn?, _ reply: ReplyFn?) -> UnsafeMutableRawPointer? {
    guard let event, let reply else { return nil }
    return Unmanaged.passRetained(WebArm(id: id, context: context, event: event, reply: reply)).toOpaque()
}

@_cdecl("exact_web_platform_view")
public func exactWebPlatformView(_ handle: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    arm(handle).map { Unmanaged.passUnretained($0.webView).toOpaque() }
}

@_cdecl("exact_web_set_src")
public func exactWebSetSrc(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.setSrc(present == 0 ? nil : String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self))
}

@_cdecl("exact_web_set_document")
public func exactWebSetDocument(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.suppliedDocument = present == 0 ? nil : Data(UnsafeBufferPointer(start: bytes, count: Int(length)))
}

@_cdecl("exact_web_set_sandbox")
public func exactWebSetSandbox(_ handle: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32, _ present: UInt32) {
    arm(handle)?.setSandbox(present == 0 ? nil : String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self))
}

@_cdecl("exact_web_snapshot")
public func exactWebSnapshot(_ handle: UnsafeMutableRawPointer?, _ token: UInt32) {
    arm(handle)?.snapshot(token: token)
}

@_cdecl("exact_web_agent_eval")
public func exactWebAgentEval(_ handle: UnsafeMutableRawPointer?, _ token: UInt32, _ bytes: UnsafePointer<UInt8>?, _ length: UInt32) {
    let script = String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self)
    arm(handle)?.evaluate(token: token, script: script)
}

@_cdecl("exact_web_destroy")
public func exactWebDestroy(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    let retained = Unmanaged<WebArm>.fromOpaque(handle)
    let arm = retained.takeUnretainedValue()
    arm.invalidate()
    arm.webView.removeFromSuperview()
    retained.release()
}
