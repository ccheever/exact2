#if os(macOS)
import AppKit
import WebKit

/// The `t3-terminal` native view (app.json `modules`): T3 Code's Ghostty terminal surface (MIT
/// reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts, its libghostty-vt WASM
/// and Canvas 2D renderer) running unchanged in a WKWebView whose page is terminal-host/src/entry.ts.
///
/// Props (strings; unknown Contract attributes pass through): `terminal` (identity, for status),
/// `terminal-theme` (JSON background/foreground/cursor/selection), `scheme` ("light" | "dark"), `terminal-font` and `terminal-font-size`, `active` ("false" stops
/// paint; bytes still parse, surface.ts setVisible), `keybindings` (JSON winning chord/command pairs),
/// `close-pending` (suppress a second close), `terminal-surface` (drawer or panel identity),
/// `chords` (the development harness's passthrough chords), `fixture` (development harness bytes:
/// "render", "loopback", "flood").
/// Session props (task terminal-drawer; T3TerminalSessions.swift): `environment`, `thread` and `terminal`
/// name a server terminal session, `cwd`, `worktree` and `env` (JSON object) launch it; the view then
/// shows the session's output, sends what is typed and its grid size to the server, writes the
/// reference's `[terminal] …` lines (ThreadTerminalDrawer.tsx writeSystemMessage) and, on the tick after
/// it prints `Process exited`, reports `{"type":"exited","terminalId":…}`.
/// `focus-request` (a number) focuses the terminal whenever it changes.
/// `auth-flow`, `auth-output` and `auth-offset` instead show a provider-auth transcript; data and
/// resize use the native serial auth queue when `auth-instance` names the provider. `read-only` suppresses writes in either mode.
/// Events: native responder focus / blur and identity-tagged command, selectionAction, link,
/// selection, contextmenu and error messages. Thread PTY bytes and grid changes stay native.
/// The web view loads only `t3-terminal:` files (T3TerminalAssets), cancels every other navigation,
/// opens no window and keeps no website data.
final class T3TerminalView: ExactNativeInstance {
    static let factory = ExactNativeFactory(snapshot: true) { (owner: ExactModule, props: [String: String], events: ExactNativeEvents) in
        T3TerminalView(props: props, events: events, agent: owner.context.agent,
                       sessions: (owner as? T3TerminalSessionOwner)?.terminalSessions)
    }

    final class WebView: WKWebView {
        var focused: (() -> Void)?
        var mounted: (() -> Void)?
        var routeKey: ((NSEvent) -> Bool)?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            mounted?()
        }
        override func becomeFirstResponder() -> Bool {
            let became = super.becomeFirstResponder()
            if became { focused?() }
            return became
        }
        override func performKeyEquivalent(with event: NSEvent) -> Bool {
            if routeKey?(event) == true { return true }
            return super.performKeyEquivalent(with: event)
        }
        override func keyDown(with event: NSEvent) {
            if routeKey?(event) != true { super.keyDown(with: event) }
        }
        override func keyUp(with event: NSEvent) {
            if routeKey?(event) != true { super.keyUp(with: event) }
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
    private var responderObservation: NSKeyValueObservation?
    private var keyboardMonitor: Any?
    private var mountedOnce = false
    private var focusPublication: UInt64 = 0
    private var bindings: [T3TerminalKeys.Binding] = []
    private var suppressedKeys: Set<UInt16> = []
    private var authWritten = 0
    private var authFlow = ""
    private lazy var authInput = T3TerminalAuth(send: { [weak self] payload, done in
        guard let self, let sessions = self.sessions else { return done(T3Failure(kind: "Closed", message: "The server is not connected.")) }
        sessions.providerAuthCall(payload, done: done)
    }, failure: { [weak self] failed in self?.emit(["type": failed ? "auth-error" : "auth-recovered"]) })
    lazy var actions = T3TerminalActions(view: self)
    /// The server session this view shows (terminal-drawer), and the renderer's place in its output.
    private let sessions: T3TerminalSessions?
    private(set) var session: T3TerminalSession?
    private var cursor = T3TerminalOutput.Cursor.initial
    private var synchronizedStatus = "closed", handledExit = false, shownVersion = -1, shownError: String? = nil

    init(props: [String: String], events: ExactNativeEvents, agent: Bool = false, sessions: T3TerminalSessions? = nil) {
        self.sessions = sessions
        self.props = props
        self.bindings = T3TerminalKeys.bindings(props["keybindings"])
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
        web.focused = { [weak self] in
            guard let self, self.ready else { return }
            self.send(["type": "focus"])
        }
        web.mounted = { [weak self] in self?.windowChanged() }
        web.routeKey = { [weak self] event in self?.routeTerminalKey(event) ?? false }
        web.setAccessibilityLabel("Terminal")
        // Under the agent the window is often behind others or on another Space; WebKit then
        // treats the page as hidden and stops requestAnimationFrame, so output parses but never
        // paints. The agent's pictures need the paint (WebKit SPI, guarded; a person's run keeps
        // WebKit's own occlusion handling, as a background tab does).
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if agent, web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        T3Terminals.shared.add(self)
        web.load(URLRequest(url: T3TerminalAssets.pageURL))
        bindSession()
        synchronizeAuthOutput()
        configureAuthInput()
    }

    // MARK: Session (terminal-drawer)

    var authMode: Bool { !(props["auth-flow"] ?? "").isEmpty }
    var sessionMode: Bool { !authMode && !(props["thread"] ?? "").isEmpty }

    /// Native first-responder ownership is synchronous: keyboard command contexts cannot wait
    /// for WebKit's asynchronous focus message after a pane click or a command opens a dialog.
    var ownsNativeFocus: Bool {
        guard props["active"] != "false", let responder = web.window?.firstResponder as? NSView else { return false }
        return responder === web || responder.isDescendant(of: web)
    }

    private func windowChanged() {
        responderObservation = nil
        if let keyboardMonitor { NSEvent.removeMonitor(keyboardMonitor); self.keyboardMonitor = nil }
        guard let window = web.window else { focusChanged(false); return }
        responderObservation = window.observe(\.firstResponder, options: [.initial, .new]) { [weak self] _, _ in
            guard let self else { return }
            self.focusChanged(self.ownsNativeFocus)
        }
        keyboardMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp]) { [weak self] event in
            self?.routeTerminalKey(event) == true ? nil : event
        }
        if !mountedOnce {
            mountedOnce = true
            if (Int(props["focus-request"] ?? "") ?? 0) > 0 { focusTerminal() }
        }
    }

    private func focusChanged(_ value: Bool, republish: Bool = false) {
        guard !disposed, focused != value || republish else { return }
        focused = value
        // Mount can give this view the responder before Contract installs its message handler.
        // Publish after the mount batch, without delaying or replaying the actual focus request.
        focusPublication &+= 1
        let publication = focusPublication
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.disposed, self.focusPublication == publication,
                  self.focused == value, self.ownsNativeFocus == value else { return }
            // Dedicated Contract focus/blur handlers use a separate mutation from terminal
            // commands, so mounting a pane cannot cancel the operation that created it.
            value ? self.events.focus() : self.events.blur()
        }
    }

    /// One event owner before AppKit's menu equivalents. The page has the same policy as a
    /// fallback for DOM input; consumed native events never reach it a second time.
    func routeTerminalKey(_ event: NSEvent) -> Bool {
        guard !disposed, event.window === web.window, event.type == .keyDown || event.type == .keyUp else { return false }
        if event.type == .keyUp { return suppressedKeys.remove(event.keyCode) != nil }
        guard ownsNativeFocus else { return false }
        let binding = bindings.first { $0.matches(event) }
        if let binding, binding.command == "terminal.close", event.isARepeat || props["close-pending"] == "true" {
            suppressedKeys.insert(event.keyCode)
            return true
        }
        // The embedded WebKit clipboard API can synchronously wait for DOM paste access,
        // freezing its renderer. The same-app native pasteboard path needs no web permission.
        // A configured app command still wins over a terminal editing shortcut.
        if (authMode || binding == nil), T3TerminalKeys.isPaste(event) {
            suppressedKeys.insert(event.keyCode)
            if props["read-only"] != "true", let text = NSPasteboard.general.string(forType: .string) {
                send(["type": "paste", "text": text])
            }
            return true
        }
        if authMode {
            // Authentication owns plain Escape as PTY input. AppKit otherwise handles it as
            // cancelOperation before WebKit can encode it, dismissing the surrounding form.
            if event.keyCode == 53, event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty {
                suppressedKeys.insert(event.keyCode)
                receiveData("\u{1b}")
                return true
            }
            if event.keyCode == 48, event.modifierFlags.intersection([.command, .control, .option]).isEmpty {
                // Let the host's key monitor refresh its lazy key-view loop first.
                // WebKit may traverse on its own; only finish traversal if focus stayed here.
                let backwards = event.modifierFlags.contains(.shift)
                DispatchQueue.main.async { [weak self] in
                    guard let self, !self.disposed, self.authMode, self.ownsNativeFocus else { return }
                    if backwards { self.web.window?.selectKeyView(preceding: self.web) }
                    else { self.web.window?.selectKeyView(following: self.web) }
                }
                return false
            }
            return false
        }
        if let binding {
            suppressedKeys.insert(event.keyCode)
            if binding.command.hasPrefix("terminal.") || !T3TerminalCommandKey.click(command: binding.command, window: web.window) {
                emit(["type": "command", "command": binding.command, "chord": binding.chord, "repeat": event.isARepeat])
            }
            return true
        }
        // Explicit harness passthrough keeps the S1 test's unhandled-key path intact.
        if Self.chords(props["chords"]).contains(where: { text in
            guard let key = T3TerminalKeys.chord(text) else { return false }
            return T3TerminalKeys.Binding(chord: text, command: "", key: key).matches(event)
        }) { return false }
        if let data = T3TerminalKeys.input(event) {
            suppressedKeys.insert(event.keyCode)
            receiveData(data)
            return true
        }
        return false
    }

    /// Stable identities on all messages let the drawer and panel share one bridge handler.
    func emit(_ body: [String: Any]) { events.message(Self.json(identified(body))) }

    private func identified(_ body: [String: Any]) -> [String: Any] {
        var value = body
        value["environmentId"] = props["environment"] ?? ""
        value["threadId"] = props["thread"] ?? ""
        value["terminalId"] = identity
        value["surface"] = props["terminal-surface"] ?? "drawer"
        if authMode { value["authFlow"] = props["auth-flow"] ?? "" }
        return value
    }

    private func synchronizeAuthOutput() {
        guard authMode else { return }
        let output = props["auth-output"] ?? ""
        let offset = Int(props["auth-offset"] ?? "") ?? output.utf16.count
        let flow = props["auth-flow"] ?? ""
        if flow != authFlow { authFlow = flow; authWritten = offset; resetAndWrite(output); return }
        let delta = offset - authWritten
        if delta > 0, delta <= output.utf16.count {
            write(String(decoding: Array(output.utf16.suffix(delta)), as: UTF16.self))
        } else if delta != 0 { resetAndWrite(output) }
        authWritten = offset
    }

    private func configureAuthInput() {
        authInput.configure(instance: props["auth-instance"] ?? "", identity: props["auth-flow"] ?? "", readOnly: props["read-only"] == "true")
    }

    private func sendAuthInput(_ data: String, size: (cols: Int, rows: Int)? = nil) {
        guard props["read-only"] != "true" else { return }
        if sessions != nil, !(props["auth-instance"] ?? "").isEmpty { authInput.input(data, size: size) }
        else if let size { emit(["type": "auth-resize", "cols": size.cols, "rows": size.rows]) }
        else { emit(["type": "auth-input", "data": data]) } // Standalone native-view fixture, without a transport owner.
    }

    private func receiveData(_ data: String) {
        guard props["read-only"] != "true" else { return }
        dataEvents += 1; dataBytes += data.utf8.count
        log("data \(Self.json(data))")
        if authMode { sendAuthInput(data); return }
        if let session, let sessions { sessions.write(data, to: session, from: self) }
        onData?(data)
    }

    private func bindSession() {
        guard sessionMode, let sessions else { return }
        let env = props["env"].flatMap { $0.data(using: .utf8) }.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: String] } ?? [:]
        let bound = sessions.bind(self, environment: props["environment"] ?? "", thread: props["thread"] ?? "", terminal: identity,
                                  cwd: props["cwd"] ?? "", worktreePath: props["worktree"] ?? "", env: env,
                                  providerInstance: props["provider-instance"] ?? "")
        session = bound
        // TerminalViewport setup: the retained output as one reset, the error, then exit handling from "closed".
        let initial = bound.output.read(from: .initial)
        if case .reset(let data) = initial.update, !data.isEmpty { write(data) }
        cursor = initial.cursor
        if let error = bound.error { systemMessage(error) }
        shownVersion = bound.version; shownError = bound.error
        synchronizedStatus = "closed"
        synchronize(bound.status)
    }

    /// The reference's session effect: exit handling, then what is new since the cursor.
    func sessionChanged(_ session: T3TerminalSession) {
        guard !disposed, session === self.session else { return }
        synchronize(session.status)
        // A reinstalled stream starts its versions again under a new output generation.
        guard session.version != shownVersion || session.output.generation != cursor.generation else { return }
        let next = session.output.read(from: cursor)
        switch next.update {
        case .reset(let data): resetAndWrite(data)
        case .append(let data): write(data)
        case .none: break
        }
        cursor = next.cursor
        if !selection.isEmpty { send(["type": "clearSelection"]); selection = "" }
        if let error = session.error, error != shownError { systemMessage(error) }
        shownVersion = session.version; shownError = session.error
    }

    /// synchronizeTerminalStatus with shouldHandleTerminalExit (terminal-drawer.ts): print the exit once, then
    /// report it on the next tick (the drawer closes the tab without a confirmation).
    private func synchronize(_ status: String) {
        if status == "running" { handledExit = false }
        else if (status == "closed" || status == "exited"), status != synchronizedStatus, !handledExit {
            handledExit = true
            systemMessage(status == "closed" ? "Terminal closed" : "Process exited")
            DispatchQueue.main.async { [weak self] in
                guard let self, self.handledExit, !self.disposed else { return }
                self.emit(["type": "exited", "thread": self.props["thread"] ?? ""])
            }
        }
        synchronizedStatus = status
    }

    /// writeSystemMessage: `\r\n[terminal] <message>\r\n`.
    func systemMessage(_ message: String) { write("\r\n[terminal] \(message)\r\n") }

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
        let auth = !(props["auth-flow"] ?? "").isEmpty
        if auth { font["size"] = 13 }
        return ["theme": theme(props), "font": font, "visible": props["active"] != "false",
                "chords": chords(props["chords"]), "keybindings": T3TerminalKeys.bindings(props["keybindings"]).map { ["chord": $0.chord, "command": $0.command] },
                "auth": auth, "readOnly": props["read-only"] == "true", "closePending": props["close-pending"] == "true"]
    }

    static func theme(_ props: [String: String]) -> [String: Any] {
        var theme = props["terminal-theme"].flatMap { $0.data(using: .utf8) }
            .flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? [:]
        theme["dark"] = props["scheme"] == "dark"
        return theme
    }

    static func chords(_ raw: String?) -> [String] { (raw ?? "").split(whereSeparator: { $0 == " " || $0 == "," }).map(String.init) }

    override func setProps(_ next: [String: String]) throws {
        let previous = props
        props = next
        bindings = T3TerminalKeys.bindings(next["keybindings"])
        if previous["keybindings"] != next["keybindings"] || previous["read-only"] != next["read-only"] || previous["close-pending"] != next["close-pending"] {
            send(["type": "keyPolicy", "keybindings": Self.initial(next)["keybindings"] ?? [],
                  "readOnly": next["read-only"] == "true", "closePending": next["close-pending"] == "true"])
        }
        synchronizeAuthOutput()
        configureAuthInput()
        if previous["scheme"] != next["scheme"] || previous["terminal-theme"] != next["terminal-theme"] { send(["type": "theme", "theme": Self.theme(next)]) }
        if previous["terminal-font"] != next["terminal-font"] || previous["terminal-font-size"] != next["terminal-font-size"] {
            send(["type": "font", "font": Self.initial(next)["font"] ?? [:]])
        }
        if previous["active"] != next["active"] { send(["type": "visible", "visible": next["active"] != "false"]) }
        if previous["chords"] != next["chords"] { send(["type": "chords", "chords": Self.chords(next["chords"])]) }
        if previous["fixture"] != next["fixture"], ready { T3TerminalFixtures.start(next["fixture"] ?? "", on: self) }
        if previous["focus-request"] != next["focus-request"], (Int(next["focus-request"] ?? "") ?? 0) > 0 { focusTerminal() }
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

    /// A focus request (the drawer opened, a terminal was activated): the web view and then the page's input.
    func focusTerminal() {
        if let window = web.window, window.firstResponder !== web { window.makeFirstResponder(web) }
        // Do not enqueue a DOM focus while WASM loads: it could outlive the user's native
        // focus. The ready callback completes it only if this pane still owns the responder.
        if ready { send(["type": "focus"]) }
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
            receiveData(body["data"] as? String ?? "")
            return
        case "ready":
            ready = true
            // Recover a mount-time focus fact if the host had not installed the handler yet.
            if ownsNativeFocus { focusChanged(true, republish: true) }
            cols = body["cols"] as? Int ?? 0; rows = body["rows"] as? Int ?? 0
            T3TerminalFixtures.start(props["fixture"] ?? "", on: self)
            if let session, let sessions { sessions.resize(session, cols: cols, rows: rows) }
            if authMode { sendAuthInput("", size: (cols, rows)) }
            // Finish a pending focus only while it still belongs to this terminal. The user may
            // have moved to the composer while WebKit/WASM loaded (TerminalViewport's mount check).
            if props["active"] != "false", let responder = web.window?.firstResponder as? NSView,
               responder === web || responder.isDescendant(of: web) { send(["type": "focus"]) }
        case "resize":
            cols = body["cols"] as? Int ?? cols; rows = body["rows"] as? Int ?? rows
            if let session, let sessions { sessions.resize(session, cols: cols, rows: rows) }
            if authMode { sendAuthInput("", size: (cols, rows)) }
        case "selection": selection = body["text"] as? String ?? ""
        case "link" where authMode:
            guard let text = body["text"] as? String, let url = URL(string: text),
                  ["https", "http"].contains(url.scheme?.lowercased() ?? "") else { return }
            if !NSWorkspace.shared.open(url) { emit(["type": "link-error"]) }
            return
        case "focus":
            focusChanged(ownsNativeFocus)
            return
        case "command":
            guard ownsNativeFocus, let command = body["command"] as? String else { return }
            if command == "terminal.close", body["repeat"] as? Bool == true || props["close-pending"] == "true" { return }
            if !command.hasPrefix("terminal."), T3TerminalCommandKey.click(command: command, window: web.window) { return }
        case "chord":
            let names = [body["metaKey"] as? Bool == true ? "Meta" : nil, body["ctrlKey"] as? Bool == true ? "Control" : nil,
                         body["altKey"] as? Bool == true ? "Alt" : nil, body["shiftKey"] as? Bool == true ? "Shift" : nil].compactMap { $0 }
            declined.append((names + [body["code"] as? String ?? ""]).joined(separator: "+"))
            declined = Array(declined.suffix(16))
        case "error": lastError = body["message"] as? String ?? "error"
        default: break
        }
        log("\(type) \(Self.json(body))")
        if !authMode { actions.receive(body) }
        // Only semantic actions enter the app's command mutation. In particular, selection
        // changes caused by Add to chat's clearSelection must not supersede its insertion.
        // Selection/menu/error facts already belong to this native view and its diagnostics.
        if !(sessionMode || authMode) || ["link", "command"].contains(type) { emit(body) }
    }

    private func log(_ line: String) {
        bridgeLog.append(String(line.prefix(160)))
        if bridgeLog.count > 32 { bridgeLog.removeFirst(bridgeLog.count - 32) }
    }

    // MARK: Page lifecycle

    fileprivate func pageLoaded() {
        loaded = true
        // Props can change while WebKit loads. The document-start script has the old values.
        let current = Self.initial(props)
        send(["type": "theme", "theme": current["theme"] ?? [:]])
        send(["type": "font", "font": current["font"] ?? [:]])
        send(["type": "visible", "visible": current["visible"] ?? true])
        send(["type": "chords", "chords": current["chords"] ?? []])
        send(["type": "keyPolicy", "keybindings": current["keybindings"] ?? [], "readOnly": current["readOnly"] ?? false,
              "closePending": current["closePending"] ?? false])
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
        authInput.clear()
        guard !disposed else { return }
        actions.dismiss(supersede: true)
        focusChanged(false)
        responderObservation = nil
        if let keyboardMonitor { NSEvent.removeMonitor(keyboardMonitor); self.keyboardMonitor = nil }
        send(["type": "dispose"])
        disposed = true
        T3TerminalFixtures.stop(self)
        if let session, let sessions { sessions.unbind(self, from: session) }
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
            return ["terminal": view.identity, "thread": view.props["thread"] ?? "", "session": view.session?.status ?? "", "ready": view.ready, "cols": view.cols, "rows": view.rows, "focused": view.focused,
                    "selection": view.selection, "error": view.lastError, "declined": view.declined, "delivery": view.agentDelivery,
                    "selectionMenu": view.actions.status,
                    "typed": view.dataBytes, "sent": view.sentBytes, "received": page["received"] ?? 0,
                    "text": page["text"] ?? [], "served": view.assets.served.count, "refused": view.assets.refusedURLs, "bridge": Array(view.bridgeLog.suffix(6))] // an Array: an ArraySlice is not JSON, and the status reply would fail
        }
        return ["terminals": entries.sorted { ($0["terminal"] as? String ?? "") < ($1["terminal"] as? String ?? "") },
                "terminalSessions": T3TerminalSessions.all.flatMap(\.status)] // terminal-drawer: streams, buffers, acknowledgements
    }
}
#endif
