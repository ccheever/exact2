// @ref llp/1106.010-mobile-browser-devices.decision.md#native-lifetime
#if os(iOS)
import UIKit
import WebKit

final class T3MobileBrowserView: ExactNativeInstance {
    private lazy var delegate = T3MobileBrowserDelegate(self)
    private weak var owner: T3MobileBrowser?
    private var audioLease: UUID?
    private let container = UIView(), shake = T3MobileDevicesShake()
    private var web: WKWebView?
    private var target: T3MobileBrowserTarget?
    private var alive = true, active = false, foreground = true, processRetried = false, started = false, starting = false, nativeOperate = false
    private var serial = 0, refusals = 0
    private var background = "#ffffff", colors: [String: String] = [:]
    private var bootstrap: DispatchWorkItem?, retry: DispatchWorkItem?, identity: Timer?
    private var observers: [NSObjectProtocol] = []
    private var files: T3MobileBrowserFiles?
    private(set) var status: [String: Any] = ["status": "connecting", "controlsVisible": true]
    override var view: UIView { container }
    init(owner: T3MobileBrowser, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        foreground = UIApplication.shared.applicationState != .background
        for name in [UIApplication.didEnterBackgroundNotification, UIApplication.willEnterForegroundNotification] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] notification in
                guard let self else { return }; foreground = notification.name != UIApplication.didEnterBackgroundNotification; reconcile()
            })
        }
    }
    override func setProps(_ props: [String: String]) throws {
        guard let json = props["stream-source"], let value = try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any] else { throw ExactNativeRefusal("Preview source is unavailable.") }
        let next = try T3MobileBrowserTarget(value), changed = target != next
        guard owner?.devices == true ? !next.device.isEmpty && !next.host.isEmpty && ["ios", "android"].contains(next.platform) : !next.tab.isEmpty else { throw ExactNativeRefusal("Preview selection is unavailable.") }
        if changed {
            if let target { owner?.unregister(self, key: target.owner) }; stop(); target = next; refusals = 0; processRetried = false
            status = ["status": "connecting", "controlsVisible": true, "tabId": next.tab, "hostId": next.host, "deviceId": next.device]; owner?.register(self, key: next.owner)
        }
        if let text = props["stream-colors"], let value = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: String] { colors = value }
        let newBackground = props["stream-background"] ?? "#ffffff"
        let colorChanged = newBackground != background; background = newBackground
        let nextActive = props["stream-active"] != "false"
        let activeChanged = active != nextActive; active = nextActive
        if colorChanged && web != nil { stop() }
        if changed || colorChanged || activeChanged || web == nil { reconcile() }
    }
    private func publish() { owner?.changed(owner?.topic ?? "t3.mobile-browser") }
    private func reconcile() {
        guard alive else { return }
        let pip = status["pipActive"] as? Bool == true
        if active && foreground || pip { if web == nil && !starting && status["status"] as? String != "error" { start() }; if owner?.devices == true && owner?.agent != true { shake.start { [weak self] in self?.toggleControls() } } }
        else { stop(); shake.stop() }
    }
    private func start() {
        guard let owner, let target else { return }
        serial += 1; let operation = serial; starting = true
        status["status"] = "connecting"; status["detail"] = ""; status.removeValue(forKey: "hostSetupCommand"); status.removeValue(forKey: "gone"); status["inputConnected"] = false; status.removeValue(forKey: "control"); publish()
        owner.access(target) { [weak self] access in
            guard let self, alive, operation == serial, self.target == target, active && foreground || status["pipActive"] as? Bool == true else { return }
            starting = false
            guard let access else { fail("This session can't open the \(owner.devices ? "device" : "browser") stream. Reconnect to try again."); return }
            do { try load(access) } catch { fail(error.localizedDescription) }
        }
    }
    private func load(_ access: [String: Any]) throws {
        guard let owner, let target else { return }
        let config = WKWebViewConfiguration(); config.allowsInlineMediaPlayback = true; config.allowsPictureInPictureMediaPlayback = !owner.devices
        config.mediaTypesRequiringUserActionForPlayback = []
        // Matches react-native-webview's explicit allowUniversalAccessFromFileURLs for its local viewer.
        config.preferences.setValue(true, forKey: "allowFileAccessFromFileURLs")
        config.setValue(true, forKey: "allowUniversalAccessFromFileURLs")
        config.userContentController.add(delegate, name: "stream")
        config.userContentController.addUserScript(WKUserScript(source: "window.ReactNativeWebView={postMessage:(message)=>window.webkit.messageHandlers.stream.postMessage(message)};", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        let next = WKWebView(frame: .zero, configuration: config); next.navigationDelegate = delegate; next.isOpaque = false
        next.scrollView.isScrollEnabled = false; next.scrollView.bounces = false; next.scrollView.contentInsetAdjustmentBehavior = .never
        next.translatesAutoresizingMaskIntoConstraints = false; container.addSubview(next)
        NSLayoutConstraint.activate([next.leadingAnchor.constraint(equalTo: container.leadingAnchor), next.trailingAnchor.constraint(equalTo: container.trailingAnchor), next.topAnchor.constraint(equalTo: container.topAnchor), next.bottomAnchor.constraint(equalTo: container.bottomAnchor)])
        web = next; started = false
        var cleanAccess = access; nativeOperate = cleanAccess.removeValue(forKey: "_operate") as? Bool == true
        var configuration: [String: Any] = ["access": cleanAccess]
        if owner.devices { configuration["deviceId"] = target.device; configuration["platform"] = target.platform; configuration["colors"] = colors; next.isUserInteractionEnabled = target.operate && nativeOperate }
        else { configuration["threadId"] = target.thread; configuration["tabId"] = target.tab; configuration["interactive"] = target.operate && nativeOperate; configuration["background"] = background }
        let name = owner.devices ? "device" : "preview"
        let json = Self.json(configuration).replacingOccurrences(of: "<", with: "\\u003c")
        let html = try Self.script(name).replacingOccurrences(of: "__T3_NATIVE_CONFIGURATION__", with: json)
        let lease = UUID(); audioLease = lease; owner.audioSession.hold(lease)
        next.loadHTMLString(html, baseURL: URL(string: "file:///"))
        files = T3MobileBrowserFiles(view: next, target: target, owner: owner)
        let task = DispatchWorkItem { [weak self] in guard let self, !started else { return }; fail("\(owner.devices ? "Device" : "Browser") viewer could not start. Reconnect to try again.") }
        bootstrap = task; DispatchQueue.main.asyncAfter(deadline: .now() + 15, execute: task)
        let checkSerial = serial
        identity = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            guard let self else { return }; owner.check(target) { [weak self] valid in guard let self, alive, serial == checkSerial, self.target == target else { return }; if !valid { fail("The connection changed. Open this preview again.") } }
        }
    }
    static func json(_ value: Any) -> String { String(data: (try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed])) ?? Data(), encoding: .utf8) ?? "null" }
    private static func script(_ name: String) throws -> String {
        let roots = [ProcessInfo.processInfo.environment["EXACT_ASSETS"].map { URL(fileURLWithPath: $0) }, Bundle.main.resourceURL].compactMap { $0 }
        guard let url = roots.map({ $0.appendingPathComponent("assets/browser-mobile/\(name).html") }).first(where: { FileManager.default.fileExists(atPath: $0.path) }) else { throw ExactNativeRefusal("The bundled preview renderer is unavailable.") }
        return try String(contentsOf: url, encoding: .utf8)
    }
    private func stop() {
        serial += 1; starting = false; bootstrap?.cancel(); bootstrap = nil; retry?.cancel(); retry = nil; identity?.invalidate(); identity = nil
        files?.destroy(); files = nil
        let lease = audioLease; audioLease = nil; let audio = owner?.audioSession
        if let web { web.setAllMediaPlaybackSuspended(true) { [web] in _ = web; if let lease { audio?.release(lease) } }; web.evaluateJavaScript("window.T3PreviewStream?.stop();window.T3DeviceStream?.stop();true;"); web.configuration.userContentController.removeScriptMessageHandler(forName: "stream"); web.navigationDelegate = nil; web.stopLoading(); web.removeFromSuperview() }
        web = nil; shake.stop(); status["inputConnected"] = false
    }
    private func fail(_ message: String) { stop(); status["status"] = "error"; status["detail"] = message; status.removeValue(forKey: "control"); publish() }
    private func toggleControls() { status["controlsVisible"] = status["controlsVisible"] as? Bool == false; publish() }
    func control(_ action: String, input: Any?, done: @escaping (String?) -> Void) {
        guard let owner, let target, alive else { return done("This preview is no longer open.") }
        owner.check(target) { [weak self] valid in
            guard let self, alive, self.target == target, valid else { return done("The connection changed. Open this preview again.") }
            if action == "reconnect" { refusals = 0; processRetried = false; stop(); status["status"] = "connecting"; reconcile(); done(nil); return }
            if action == "copy-setup", let command = status["hostSetupCommand"] as? String { UIPasteboard.general.string = command; done(nil); return }
            if action == "controls", owner.devices { toggleControls(); done(nil); return }
            if action == "pip", !owner.devices { web?.evaluateJavaScript("window.T3PreviewStream?.pictureInPicture();true;"); done(nil); return }
            guard action == "command", status["status"] as? String == "streaming", target.operate, nativeOperate else { return done("This preview cannot receive input.") }
            if owner.devices {
                guard status["inputConnected"] as? Bool == true, let command = input as? String, ["home", "back", "appSwitcher", "rotate"].contains(command) else { return done("Device input is not connected.") }
                web?.evaluateJavaScript("window.T3DeviceStream?.command(\(Self.json(command)));true;")
            } else {
                guard var command = input as? [String: Any], let type = command["type"] as? String,
                      ["history", "reload", "navigate", "takeControl", "releaseControl", "dialog"].contains(type) else { return done("Unsupported browser input.") }
                if type == "dialog" { guard files?.allowsDialogAnswer(command) == true else { return done("This page dialog changed.") }; command.removeValue(forKey: "_dialogKey"); command.removeValue(forKey: "_dialogVersion") }
                let control = status["control"] as? [String: Any] ?? [:]
                guard control["canOperate"] as? Bool == true, type == "takeControl" && control["controller"] as? String != "another-viewer" || control["controller"] as? String == "you" else { return done("This browser is not under your control.") }
                web?.evaluateJavaScript("window.T3PreviewStream?.command(\(Self.json(command)));true;")
            }
            done(nil)
        }
    }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard alive, message.webView === web, message.frameInfo.isMainFrame, let text = message.body as? String,
              let value = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any], let type = value["type"] as? String else { return }
        switch type {
        case "status":
            started = true; bootstrap?.cancel(); status["status"] = value["status"]; status["detail"] = value["detail"] ?? ""
            if value["status"] as? String == "error" { fail(value["detail"] as? String ?? "Preview stream failed."); return }
            if value["status"] as? String == "streaming" { refusals = 0; processRetried = false }
        case "input": status["inputConnected"] = value["connected"] as? Bool == true && nativeOperate; if value["connected"] as? Bool == true { status["controlsVisible"] = false }
        case "control": status["control"] = value; files?.dialog(value["dialog"] as? [String: Any], allowed: value["controller"] as? String == "you") { [weak self] input in self?.control("command", input: input) { _ in } }
        case "pictureInPicture": status["pipSupported"] = value["supported"]; status["pipActive"] = value["active"]; if let detail = value["detail"] as? String { files?.alert("Picture in picture is unavailable", detail) }; reconcile()
        case "clipboard": if let text = value["text"] as? String { UIPasteboard.general.string = text }
        case "download": files?.download(value)
        case "fileChooser": files?.choose(value["chooser"] as? [String: Any])
        case "gone": fail("This tab was closed."); status["gone"] = true
        case "hostSetup": let need = value["need"] as? String ?? "libraries", command = value["command"] as? String ?? ""; fail((need == "sandbox" ? "This server's host blocks the sandbox its browser runs in. Run this once on the host, then try again:" : "This server's host is missing libraries its browser needs. Run this once on the host, then try again:")); status["hostSetupCommand"] = command
        case "unauthorized":
            refusals += 1
            if owner?.devices != true && refusals >= 3 { fail("This session can't open the browser stream. Reconnect to try again."); return }
            stop(); let task = DispatchWorkItem { [weak self] in self?.reconcile() }; retry = task
            DispatchQueue.main.asyncAfter(deadline: .now() + (refusals <= 1 ? 0 : 2), execute: task)
        case "retry": stop(); processRetried = false; reconcile()
        default: break
        }
        publish()
    }
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard alive, webView === web else { decisionHandler(.cancel); return }
        let url = navigationAction.request.url?.absoluteString ?? ""; decisionHandler(url == "about:blank" || url == "file:///" ? .allow : .cancel)
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        guard alive, webView === web else { return }
        if processRetried { fail("\(owner?.devices == true ? "Device" : "Browser") viewer stopped. Reconnect to try again.") }
        else { processRetried = true; stop(); reconcile() }
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { guard alive, webView === web else { return }; fail("Preview viewer could not load. Reconnect to try again.") }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { guard alive, webView === web else { return }; fail("Preview viewer could not load. Reconnect to try again.") }
    override func destroy() { guard alive else { return }; alive = false; stop(); observers.forEach(NotificationCenter.default.removeObserver); observers.removeAll(); if let target { owner?.unregister(self, key: target.owner) } }
}
private final class T3MobileBrowserDelegate: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    weak var owner: T3MobileBrowserView?
    init(_ owner: T3MobileBrowserView) { self.owner = owner }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) { owner?.userContentController(controller, didReceive: message) }
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let owner else { decisionHandler(.cancel); return }; owner.webView(webView, decidePolicyFor: action, decisionHandler: decisionHandler)
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { owner?.webViewWebContentProcessDidTerminate(webView) }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { owner?.webView(webView, didFail: navigation, withError: error) }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { owner?.webView(webView, didFailProvisionalNavigation: navigation, withError: error) }
}
#endif
