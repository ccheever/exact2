// The Apple iframe arm: WebKit stays in this dylib, loaded at the first
// webview commit (@ref LLP 1020 D2/D3). The presenters see only its C ABI.
import Foundation
import WebKit

#if os(macOS)
import AppKit
private typealias PlatformImage = NSImage
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
    var served = false
    var servePending = false
    var generation = 0
    var guestFrame: WKFrameInfo?
    var suppressLoad = false
    var recovering = false
    var wrapperURL: URL { URL(string: "https://exact.invalid/frame/\(id)/index.html")! }

    init(id: UInt32, context: UnsafeMutableRawPointer?, event: @escaping EventFn, reply: @escaping ReplyFn) {
        self.id = id
        self.context = context
        self.event = event
        self.reply = reply
        let configuration = WKWebViewConfiguration()
        configuration.userContentController = controller
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()
        controller.add(self, name: "exact")
        controller.add(self, contentWorld: world, name: "exactAgent")
        controller.addUserScript(WKUserScript(
            source: "window.webkit.messageHandlers.exactAgent.postMessage('ready')",
            injectionTime: .atDocumentStart,
            forMainFrameOnly: false,
            in: world))
        webView.navigationDelegate = self
        #if os(macOS)
        webView.setValue(false, forKey: "drawsBackground")
        #else
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        #endif
    }

    deinit {
        controller.removeScriptMessageHandler(forName: "exact")
        controller.removeScriptMessageHandler(forName: "exactAgent", contentWorld: world)
        webView.navigationDelegate = nil
    }

    func setSrc(_ value: String?) {
        guard !srcInitialized || src != value else { return }
        srcInitialized = true
        src = value
        markLoading()
        if !served || servePending {
            scheduleServe()
            return
        }
        let script = navigationScript()
        webView.evaluateJavaScript(script) { [weak self] _, error in
            guard let self, error != nil else { return }
            self.scheduleServe()
        }
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
        guard !servePending else { return }
        servePending = true
        DispatchQueue.main.async { [weak self] in
            guard let self, self.servePending else { return }
            self.servePending = false
            self.serve()
        }
    }

    func serve(error: String? = nil) {
        generation += 1
        guestFrame = nil
        served = true
        let request = URLRequest(url: wrapperURL, cachePolicy: .reloadIgnoringLocalCacheData)
        webView.loadSimulatedRequest(request, responseHTML: wrapper(error: error))
    }

    func wrapper(error: String?) -> String {
        let source = src.map { " src=\"\(attribute($0))\"" } ?? ""
        let restriction = sandbox.map { " sandbox=\"\(attribute($0))\"" } ?? ""
        let local = error.map(errorDocument) ?? src.flatMap(localDocument)
        let localScript = local.map { document in
            let encoded = Data(document.utf8).base64EncodedString()
            return "inner.srcdoc = new TextDecoder().decode(Uint8Array.from(atob('\(encoded)'), c => c.charCodeAt(0)));"
        } ?? ""
        return """
        <!doctype html><meta charset="utf-8">
        <style>html,body,iframe{margin:0;width:100%;height:100%;border:0;display:block}body{overflow:hidden}</style>
        <iframe id="exact-frame"\(source)\(restriction)></iframe>
        <script>
        (() => {
          const inner = document.getElementById('exact-frame');
          addEventListener('message', event => {
            if (event.source !== inner.contentWindow) return;
            let payload = event.data;
            if (typeof payload !== 'string') {
              try { payload = JSON.stringify(payload); } catch { return; }
            }
            if (typeof payload !== 'string') return;
            webkit.messageHandlers.exact.postMessage(JSON.stringify({kind:'message', generation:\(generation), payload}));
          });
          inner.addEventListener('load', () => webkit.messageHandlers.exact.postMessage(JSON.stringify({kind:'load', generation:\(generation)})));
          \(localScript)
        })();
        </script>
        """
    }

    func navigationScript() -> String {
        let setSource: String
        if let src {
            setSource = "inner.setAttribute('src', \(javascriptString(src)));"
        } else {
            setSource = "inner.removeAttribute('src');"
        }
        let local = src.flatMap(localDocument).map { document -> String in
            let encoded = Data(document.utf8).base64EncodedString()
            return "inner.srcdoc = new TextDecoder().decode(Uint8Array.from(atob('\(encoded)'), c => c.charCodeAt(0)));"
        } ?? ""
        return """
        (() => {
          const inner = document.getElementById('exact-frame');
          if (!inner) return false;
          inner.removeAttribute('srcdoc');
          \(setSource)
          \(local)
          return true;
        })()
        """
    }

    func localDocument(_ source: String) -> String? {
        guard URL(string: source)?.scheme == nil else { return nil }
        let path = source.split(separator: "?", maxSplits: 1)[0].split(separator: "#", maxSplits: 1)[0]
        let relative = path.drop(while: { $0 == "/" })
        let base = ProcessInfo.processInfo.environment["EXACT_ASSETS"]
            .map { URL(fileURLWithPath: $0) }
            ?? Bundle.main.resourceURL
            ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let root = base.standardizedFileURL.path
        let file = base.appendingPathComponent(String(relative)).standardizedFileURL
        guard file.path == root || file.path.hasPrefix(root.hasSuffix("/") ? root : root + "/") else { return nil }
        return try? String(contentsOf: file, encoding: .utf8)
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

    func javascriptString(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: [value]),
              var text = String(data: data, encoding: .utf8)
        else { return "\"\"" }
        text.removeFirst()
        text.removeLast()
        return text
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        if message.name == "exactAgent" {
            if !message.frameInfo.isMainFrame { guestFrame = message.frameInfo }
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
        if navigationAction.targetFrame?.isMainFrame == true,
           served, navigationAction.request.url != wrapperURL {
            // An allow-top-navigation escape damages only the wrapper; restore
            // the iframe topology instead of turning the arm into a browser.
            decisionHandler(.cancel)
            markLoading()
            scheduleServe()
        } else {
            decisionHandler(.allow)
        }
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
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
        recovering = false
    }

    func snapshot(token: UInt32) {
        guard webView.bounds.width > 0, webView.bounds.height > 0 else {
            sendReply(token: token, kind: 2, text: "webview has no snapshot box")
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
        let data = Data(text.utf8)
        data.withUnsafeBytes { bytes in
            event(context, id, kind, bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count))
        }
    }

    func sendReply(token: UInt32, kind: UInt32, text: String) {
        sendReply(token: token, kind: kind, data: Data(text.utf8))
    }

    func sendReply(token: UInt32, kind: UInt32, data: Data) {
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
    let arm = Unmanaged<WebArm>.fromOpaque(handle).takeRetainedValue()
    arm.webView.removeFromSuperview()
}
