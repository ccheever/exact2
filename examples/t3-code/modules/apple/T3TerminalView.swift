#if os(macOS)
import AppKit
import WebKit

/// The `t3-terminal` native view (app.json `modules`): T3 Code's Ghostty terminal surface (MIT
/// reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts, its libghostty-vt WASM
/// and Canvas 2D renderer) running unchanged in a WKWebView whose page is terminal-host/src/entry.ts.
///
/// Props (strings; unknown Contract attributes pass through): `terminal` (identity, for status),
/// `scheme` ("light" | "dark"), `terminal-font` and `terminal-font-size`, `active` ("false" stops
/// paint; bytes still parse, surface.ts setVisible), `chords` (space-separated web-named chords the
/// page leaves to the app, ThreadTerminalDrawer.tsx beforeKey), `fixture` (development harness bytes:
/// "render", "loopback", "flood").
/// Events: focus / blur as the page's input gains and loses focus; `message` with the bridge's
/// JSON for ready, resize, selection, link, contextmenu, chord, error (data stays native: a session
/// owner reads it through `onData`).
/// The web view loads only `t3-terminal:` files (T3TerminalAssets), cancels every other navigation,
/// opens no window and keeps no website data.
final class T3TerminalView: ExactNativeInstance {
    static let factory = ExactNativeFactory(snapshot: true) { (owner: ExactModule, props: [String: String], events: ExactNativeEvents) in
        T3TerminalView(props: props, events: events, agent: owner.context.agent)
    }

    final class WebView: WKWebView {
        var focused: (() -> Void)?
        override func becomeFirstResponder() -> Bool {
            let became = super.becomeFirstResponder()
            if became { focused?() }
            return became
        }
    }

    /// The script message handler holds the view weakly: WKUserContentController retains it.
    private final class Relay: NSObject, WKScriptMessageHandler {
        weak var owner: T3TerminalView?
        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any] else { return }
            owner?.received(body)
        }
    }

    /// Navigation and window requests (WebKit delegates must be NSObjects; this one holds the view weakly).
    private final class Delegate: NSObject, WKNavigationDelegate, WKUIDelegate {
        weak var owner: T3TerminalView?
        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
            let allowed = action.request.url?.scheme == T3TerminalAssets.scheme && action.request.url?.path == T3TerminalAssets.pageURL.path
            if !allowed { owner?.assets.refusedNavigation(action.request.url) }
            decisionHandler(allowed ? .allow : .cancel)
        }
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { owner?.pageLoaded() }
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { owner?.lastError = error.localizedDescription }
        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { owner?.lastError = error.localizedDescription }
        func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { owner?.processStopped() }
        func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
            owner?.assets.refusedNavigation(action.request.url)
            return nil
        }
    }

    let web: WebView
    let assets = T3TerminalAssets()
    private let delegate = Delegate()
    private(set) var props: [String: String]
    /// What the page reported (the agent's status).
    private(set) var ready = false, cols = 0, rows = 0, focused = false
    fileprivate(set) var lastError = "", selection = "", declined: [String] = [], loaded = false
    private(set) var dataEvents = 0, dataBytes = 0, bridgeLog: [String] = []
    /// Bytes for the page, sent once per frame (one evaluateJavaScript per 16 ms at most).
    private var pending = "", pendingReset = false, flushScheduled = false
    private(set) var sentChunks = 0, sentBytes = 0
    /// Typed bytes, for a session owner (the drawer); the loopback fixture echoes them.
    var onData: ((String) -> Void)?
    private var disposed = false

    init(props: [String: String], events: ExactNativeEvents, agent: Bool = false) {
        self.props = props
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        if #available(macOS 14.0, *) { configuration.preferences.inactiveSchedulingPolicy = .none }
        configuration.setURLSchemeHandler(assets, forURLScheme: T3TerminalAssets.scheme)
        let relay = Relay()
        configuration.userContentController.add(relay, name: "t3terminal")
        configuration.userContentController.addUserScript(WKUserScript(source: "window.t3TerminalInit = \(Self.json(Self.initial(props)));", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        web = WebView(frame: NSRect(x: 0, y: 0, width: 480, height: 240), configuration: configuration)
        super.init(events: events)
        relay.owner = self
        delegate.owner = self
        web.navigationDelegate = delegate
        web.uiDelegate = delegate
        web.setValue(false, forKey: "drawsBackground")
        web.allowsMagnification = false
        web.allowsBackForwardNavigationGestures = false
        // EXACT2-GAPS X2 option 3: a development run's terminal is inspectable from Safari.
        if #available(macOS 13.3, *), Self.inspectable { web.isInspectable = true }
        web.focused = { [weak self] in self?.send(["type": "focus"]) }
        web.setAccessibilityLabel("Terminal")
        // Under the agent the window is often behind others or on another Space; WebKit then
        // treats the page as hidden and stops requestAnimationFrame, so output parses but never
        // paints. The agent's pictures need the paint (WebKit SPI, guarded; a person's run keeps
        // WebKit's own occlusion handling, as a background tab does).
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if agent, web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        T3Terminals.shared.add(self)
        web.load(URLRequest(url: T3TerminalAssets.pageURL))
    }

    /// Inspectable in development runs only: the host's `--run` and the agent set EXACT_ASSETS.
    static var inspectable: Bool {
        let environment = ProcessInfo.processInfo.environment
        return environment["EXACT_ASSETS"] != nil || environment["T3_TERMINAL_INSPECTABLE"] == "1"
    }

    override var view: ExactNativeView { web }
    override var focusTarget: ExactNativeView? { web }

    var identity: String { props["terminal"] ?? "" }

    static func initial(_ props: [String: String]) -> [String: Any] {
        var font: [String: Any] = [:]
        if let family = props["terminal-font"], !family.isEmpty { font["family"] = family }
        if let size = props["terminal-font-size"].flatMap(Double.init) { font["size"] = size }
        return ["theme": ["dark": props["scheme"] == "dark"], "font": font, "visible": props["active"] != "false",
                "chords": chords(props["chords"])]
    }

    static func chords(_ raw: String?) -> [String] { (raw ?? "").split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init) }

    override func setProps(_ next: [String: String]) throws {
        let previous = props
        props = next
        if previous["scheme"] != next["scheme"] { send(["type": "theme", "theme": ["dark": next["scheme"] == "dark"]]) }
        if previous["terminal-font"] != next["terminal-font"] || previous["terminal-font-size"] != next["terminal-font-size"] {
            send(["type": "font", "font": Self.initial(next)["font"] ?? [:]])
        }
        if previous["active"] != next["active"] { send(["type": "visible", "visible": next["active"] != "false"]) }
        if previous["chords"] != next["chords"] { send(["type": "chords", "chords": Self.chords(next["chords"])]) }
        if previous["fixture"] != next["fixture"], ready { T3TerminalFixtures.start(next["fixture"] ?? "", on: self) }
    }

    // MARK: Native → page

    /// Output for the terminal; batched to one bridge call per frame.
    func write(_ data: String) {
        guard !disposed, !data.isEmpty else { return }
        pending += data
        scheduleFlush()
    }

    /// Replace the screen with `data` (a replay); drops output not yet sent.
    func resetAndWrite(_ data: String) {
        guard !disposed else { return }
        pending = data; pendingReset = true
        scheduleFlush()
    }

    private func scheduleFlush() {
        guard !flushScheduled else { return }
        flushScheduled = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0 / 60) { [weak self] in self?.flush() }
    }

    private func flush() {
        flushScheduled = false
        guard loaded, !disposed, !pending.isEmpty || pendingReset else { return }
        let data = pending, reset = pendingReset
        pending = ""; pendingReset = false
        sentChunks += 1; sentBytes += data.utf8.count
        send(["type": reset ? "resetAndWrite" : "write", "data": data])
    }

    /// One message to the page. Output still waiting for its frame goes first, so a paste or a
    /// mode change never overtakes bytes written before it.
    func send(_ message: [String: Any], reply: ((Any?) -> Void)? = nil) {
        guard loaded, !disposed else { reply?(nil); return }
        if let type = message["type"] as? String, type != "write", type != "resetAndWrite", !pending.isEmpty || pendingReset { flush() }
        web.evaluateJavaScript("t3Terminal.receive(\(Self.json(message)))") { value, _ in reply?(value) }
    }

    func readSelection(_ done: @escaping (String) -> Void) { send(["type": "readSelection"]) { done($0 as? String ?? "") } }

    /// The page's own view of itself (ready, grid, visible text, selection, error).
    func debug(_ done: @escaping ([String: Any]) -> Void) {
        guard loaded, !disposed else { return done(["ready": false, "error": lastError]) }
        web.evaluateJavaScript("t3Terminal.debug()") { value, error in
            var result = value as? [String: Any] ?? [:]
            if let error { result["evaluateError"] = error.localizedDescription }
            done(result)
        }
    }

    static func json(_ value: Any) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]) else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }

    // MARK: Page → native

    fileprivate func received(_ body: [String: Any]) {
        guard !disposed, let type = body["type"] as? String else { return }
        switch type {
        case "data":
            let data = body["data"] as? String ?? ""
            dataEvents += 1; dataBytes += data.utf8.count
            log("data \(Self.json(data))")
            onData?(data)
            return
        case "ready":
            ready = true
            cols = body["cols"] as? Int ?? 0; rows = body["rows"] as? Int ?? 0
            T3TerminalFixtures.start(props["fixture"] ?? "", on: self)
        case "resize":
            cols = body["cols"] as? Int ?? cols; rows = body["rows"] as? Int ?? rows
        case "selection": selection = body["text"] as? String ?? ""
        case "focus":
            focused = body["focused"] as? Bool ?? false
            focused ? events.focus() : events.blur()
        case "chord":
            let names = [body["metaKey"] as? Bool == true ? "Meta" : nil, body["ctrlKey"] as? Bool == true ? "Control" : nil,
                         body["altKey"] as? Bool == true ? "Alt" : nil, body["shiftKey"] as? Bool == true ? "Shift" : nil].compactMap { $0 }
            declined.append((names + [body["code"] as? String ?? ""]).joined(separator: "+"))
            declined = Array(declined.suffix(16))
        case "error": lastError = body["message"] as? String ?? "error"
        default: break
        }
        log("\(type) \(Self.json(body))")
        events.message(Self.json(body))
    }

    private func log(_ line: String) {
        bridgeLog.append(String(line.prefix(160)))
        if bridgeLog.count > 32 { bridgeLog.removeFirst(bridgeLog.count - 32) }
    }

    // MARK: Page lifecycle

    fileprivate func pageLoaded() {
        loaded = true
        if !pending.isEmpty || pendingReset { flush() }
    }

    fileprivate func processStopped() {
        lastError = "The terminal process stopped"
        ready = false; loaded = false
    }

    // MARK: Agent

    /// The agent's `type` / `press`: real key events through the app's own dispatch (local
    /// monitors, key equivalents, then the web view as first responder), so a chord the page
    /// declines reaches the app's key path as a person's would (spike S1, S5).
    override func agentInput(_ input: ExactNativeInput) throws {
        guard let window = web.window else { throw ExactNativeRefusal("the terminal is not in a window") }
        if window.firstResponder !== web { window.makeFirstResponder(web) }
        switch input {
        case .text(let text):
            for character in text {
                let key = T3TerminalKeys.event(for: character)
                try deliver(window, key.code, characters: String(character), plain: key.plain, flags: key.flags, phase: nil)
            }
        case .key(let chord, let phase):
            guard let key = T3TerminalKeys.chord(chord) else { throw ExactNativeRefusal("unsupported key \(chord)") }
            try deliver(window, key.code, characters: key.characters, plain: key.plain, flags: key.flags, phase: phase)
        }
    }

    private(set) var agentDelivery = ""

    private func deliver(_ window: NSWindow, _ code: UInt16, characters: String, plain: String, flags: NSEvent.ModifierFlags, phase: String?) throws {
        for type in phase == "up" ? [NSEvent.EventType.keyUp] : phase == "down" ? [.keyDown] : [.keyDown, .keyUp] {
            guard let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, characters: characters,
                                               charactersIgnoringModifiers: plain, isARepeat: false, keyCode: code) else {
                throw ExactNativeRefusal("no key event for \(characters)")
            }
            if NSApp.keyWindow === window { NSApp.sendEvent(event); agentDelivery = "application" }
            else { window.sendEvent(event); agentDelivery = "window" }
        }
    }

    override func snapshot() throws -> Data {
        var png: Data?, failure: String?
        web.takeSnapshot(with: nil) { image, error in
            if let image, let tiff = image.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) { png = rep.representation(using: .png, properties: [:]) }
            failure = error?.localizedDescription ?? (image == nil ? "no snapshot" : nil)
        }
        let deadline = Date().addingTimeInterval(3)
        while png == nil, failure == nil, Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        guard let png else { throw ExactNativeRefusal(failure ?? "terminal snapshot timed out") }
        return png
    }

    override func destroy() {
        guard !disposed else { return }
        send(["type": "dispose"])
        disposed = true
        T3TerminalFixtures.stop(self)
        web.stopLoading()
        web.navigationDelegate = nil; web.uiDelegate = nil
        web.configuration.userContentController.removeScriptMessageHandler(forName: "t3terminal")
        web.removeFromSuperview()
        T3Terminals.shared.remove(self)
    }
}

extension T3TerminalAssets {
    func refusedNavigation(_ url: URL?) { if let url { refusedURLs.append(url.absoluteString) } }
}

/// Every live terminal view, for the module's status (state.presentation.terminals).
final class T3Terminals {
    static let shared = T3Terminals()
    private var views: [ObjectIdentifier: T3TerminalView] = [:]
    /// Last debug read per view, refreshed after each status read (the page answers asynchronously).
    private var pages: [ObjectIdentifier: [String: Any]] = [:]
    func add(_ view: T3TerminalView) { views[ObjectIdentifier(view)] = view }
    func remove(_ view: T3TerminalView) { views[ObjectIdentifier(view)] = nil; pages[ObjectIdentifier(view)] = nil }
    var all: [T3TerminalView] { Array(views.values) }

    var status: [String: Any] {
        let entries = views.map { key, view -> [String: Any] in
            view.debug { [weak self] page in self?.pages[key] = page }
            let page = pages[key] ?? [:]
            return ["terminal": view.identity, "ready": view.ready, "cols": view.cols, "rows": view.rows, "focused": view.focused,
                    "selection": view.selection, "error": view.lastError, "declined": view.declined, "delivery": view.agentDelivery,
                    "typed": view.dataBytes, "sent": view.sentBytes, "received": page["received"] ?? 0,
                    "text": page["text"] ?? [], "served": view.assets.served.count, "refused": view.assets.refusedURLs, "bridge": view.bridgeLog.suffix(6)]
        }
        return ["terminals": entries.sorted { ($0["terminal"] as? String ?? "") < ($1["terminal"] as? String ?? "") }]
    }
}
#endif
