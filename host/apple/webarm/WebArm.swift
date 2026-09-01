// The Apple iframe arm: WebKit stays in this dylib, loaded at the first
// iframe commit (@ref LLP 1020 D2/D3). The presenters see only its C ABI.
import Foundation
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
    var wrapperURL: URL { URL(string: "https://exact.invalid/frame/\(id)/index.html")! }

    init(id: UInt32, context: UnsafeMutableRawPointer?, event: @escaping EventFn, reply: @escaping ReplyFn) {
        self.id = id
        self.context = context
        self.event = event
        self.reply = reply
        let configuration = WKWebViewConfiguration()
        configuration.userContentController = controller
        #if os(macOS)
        webView = ExactWebView(frame: .zero, configuration: configuration)
        #else
        webView = WKWebView(frame: .zero, configuration: configuration)
        #endif
        super.init()
        controller.add(self, name: "exact")
        controller.add(self, contentWorld: world, name: "exactAgent")
        controller.addUserScript(WKUserScript(
            source: "if (window.parent === window.top && window !== window.top) window.webkit.messageHandlers.exactAgent.postMessage('ready')",
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
        controller.removeScriptMessageHandler(forName: "exact")
        controller.removeScriptMessageHandler(forName: "exactAgent", contentWorld: world)
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

    func serve(error: String? = nil) {
        guard !invalidated else { return }
        generation += 1
        guestFrame = nil
        serving = true
        let request = URLRequest(url: wrapperURL, cachePolicy: .reloadIgnoringLocalCacheData)
        webView.loadSimulatedRequest(request, responseHTML: wrapper(error: error))
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
        <!doctype html><meta charset="utf-8">
        <style>html,body,iframe{margin:0;width:100%;height:100%;border:0;display:block}body{overflow:hidden}</style>
        <iframe id="exact-frame"\(source)\(restriction)></iframe>
        <script>
        (() => {
          const inner = document.getElementById('exact-frame');
          const castleUser = \(castleUserLiteral());
          const guestOrigin = \(expectedOrigin);
          let committed = false;
          let navigated = false;
          window.__exactRevokeGuest = () => { navigated = true; };
          addEventListener('message', event => {
            if (event.source !== inner.contentWindow) return;
            if (navigated || guestOrigin === null || event.origin !== guestOrigin) return;
            let data = event.data;
            if (typeof data === 'string') {
              try { data = JSON.parse(data); } catch { data = null; }
            }
            if (data && data.castleSdk === 1) {
              if (data.lifecycle) return;
              if (typeof data.requestId === 'string') {
                const reply = data.command === 'user.getCurrent' && guestOrigin === 'null'
                  ? { castleSdk: 1, requestId: data.requestId, ok: false, error: { code: 'UNAVAILABLE', message: 'Identity is unavailable to an opaque guest' } }
                  : data.command === 'user.getCurrent'
                    ? { castleSdk: 1, requestId: data.requestId, ok: true, data: { user: castleUser } }
                    : { castleSdk: 1, requestId: data.requestId, ok: false, error: { code: 'UNAVAILABLE', message: 'This host does not implement ' + String(data.command) } };
                inner.contentWindow.postMessage(reply, guestOrigin === 'null' ? '*' : guestOrigin);
              }
              return;
            }
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
        if local { return "https://exact.invalid" }
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

    func castleUserLiteral() -> String {
        let username = srcQuery("user")
        guard !username.isEmpty else { return "null" }
        let id = srcQuery("id")
        let userId = id.isEmpty ? username : id
        let obj: [String: String] = ["userId": userId, "username": username]
        guard let data = try? JSONSerialization.data(withJSONObject: obj),
              var json = String(data: data, encoding: .utf8) else { return "null" }
        json = json.replacingOccurrences(of: "<", with: "\\u003c")
        return json
    }

    func srcQuery(_ name: String) -> String {
        guard let src else { return "" }
        guard let q = src.split(separator: "?", maxSplits: 1).dropFirst().first else { return "" }
        let query = q.split(separator: "#", maxSplits: 1)[0]
        for pair in query.split(separator: "&") {
            let kv = pair.split(separator: "=", maxSplits: 1)
            guard kv.first.map(String.init) == name else { continue }
            let raw = kv.count > 1 ? String(kv[1]) : ""
            return raw.removingPercentEncoding ?? raw
        }
        return ""
    }

    func localDocument(_ source: String) -> String? {
        // Hosted http(s) decks keep their URL. A scheme-less src — including
        // `URL(string:)` returning nil for a leading-dot relative path — is a
        // file under EXACT_ASSETS, inlined as srcdoc. Query/hash are identity
        // for the castleSdk wrapper, not part of the path.
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
        guard var html = try? String(contentsOf: file, encoding: .utf8) else { return nil }
        // Castle playables letterbox a 5:7 card in the iframe. Weird Castle
        // asked for the deck to fill the viewport, so the card becomes the
        // iframe's box (the kernel already owns that box).
        if html.contains("CastleEmbed") || html.contains("castle-card") {
            html += """
            <style id="exact-fullbleed">
            html,body{width:100%!important;height:100%!important;margin:0!important}
            #castle-card,#root > *,[data-castle-card]{
              position:fixed!important;inset:0!important;left:0!important;top:0!important;
              transform:none!important;width:100%!important;height:100%!important;
              max-width:none!important;max-height:none!important;border-radius:0!important;
            }
            </style>
            <script>
            (function(){
              function fill(){
                document.documentElement.style.setProperty('--castle-card-w', innerWidth+'px');
                document.documentElement.style.setProperty('--castle-card-h', innerHeight+'px');
                var c=document.getElementById('castle-card');
                if(c){c.style.width=innerWidth+'px';c.style.height=innerHeight+'px';c.style.borderRadius='0';}
              }
              addEventListener('resize', fill);
              fill();
              new MutationObserver(fill).observe(document.documentElement,{childList:true,subtree:true});
            })();
            </script>
            """
        }
        return html
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
        if message.name == "exactAgent" {
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
        guard navigationAction.targetFrame?.isMainFrame == true else {
            // Once the injected guest agent identified the committed child,
            // revoke its identity capability at the start of any subsequent
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
        guard let guestFrame else {
            sendReply(token: token, kind: 2, text: "guest frame is not ready")
            return
        }
        webView.evaluateJavaScript(script, in: guestFrame, in: world) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let value):
                if let text = value as? String { self.sendReply(token: token, kind: 1, text: text) }
                else if value is NSNull { self.sendReply(token: token, kind: 1, text: "null") }
                else { self.sendReply(token: token, kind: 1, text: String(describing: value)) }
            case .failure(let error):
                self.sendReply(token: token, kind: 2, text: error.localizedDescription)
            }
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
