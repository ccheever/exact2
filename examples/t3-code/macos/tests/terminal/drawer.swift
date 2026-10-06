import AppKit
import XCTest

// Task terminal-drawer: the native output buffer against the shared vectors (also replayed by
// terminal-output.test.ts against the reference port), the attach stream reducer, and a session view
// writing what is new, the reference's `[terminal] …` lines and its exit report once.
final class TerminalDrawerTests: XCTestCase {
    private var window: NSWindow!
    private var views: [T3TerminalView] = []
    private var transport: T3Transport!
    private var sessions: T3TerminalSessions!

    override func setUpWithError() throws {
        let app = try XCTUnwrap(ProcessInfo.processInfo.environment["T3_APP_DIR"], "set T3_APP_DIR to examples/t3-code")
        setenv("EXACT_ASSETS", app, 1)
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        if ProcessInfo.processInfo.environment["T3_TERMINAL_HIDDEN"] != "1" { window.orderFrontRegardless() }
        transport = T3Transport(persistent: false, signals: false, changed: { _ in })
        sessions = T3TerminalSessions.of(transport)
        sessions.detachGrace = 0.1
        drawerMessages = []
        drawerFocusEvents = []
        drawerListenerReady = true
        drawerDropFocus = 0
    }

    override func tearDown() {
        for view in views { view.destroy() }
        views = []
        transport.destroy()
        window.orderOut(nil)
    }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 15) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }

    private func mount(thread: String = "thread-1", terminal: String = "term-1", focusRequest: String = "0", extra: [String: String] = [:]) -> T3TerminalView {
        let props = ["terminal": terminal, "environment": "env-a", "thread": thread, "cwd": "/repo", "worktree": "", "env": "{\"T3CODE_PROJECT_ROOT\":\"/repo\"}", "focus-request": focusRequest].merging(extra) { _, next in next }
        let view = T3TerminalView(props: props, events: ExactNativeEvents(fn: recordDrawerEvent, ctx: nil, nonce: 1), agent: true, sessions: sessions)
        view.web.frame = window.contentView!.bounds
        window.contentView!.addSubview(view.web)
        views.append(view)
        return view
    }

    private func text(_ view: T3TerminalView) -> [String] {
        var result: [String: Any]?
        view.debug { result = $0 }
        spin(until: { result != nil }, timeout: 5)
        return (result?["text"] as? [String] ?? []).filter { !$0.isEmpty }
    }

    private func messages(_ type: String) -> [[String: Any]] {
        drawerMessages.compactMap { (try? JSONSerialization.jsonObject(with: Data($0.dropFirst(2).utf8))) as? [String: Any] }
            .filter { $0["type"] as? String == type }
    }

    private func key(_ code: UInt16, _ flags: NSEvent.ModifierFlags, repeat repeating: Bool = false, characters: String = "") -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                        windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: repeating, keyCode: code)!
    }

    func testTerminalNativeTextInputPreventsComposerTypeToFocus() {
        let view = mount(focusRequest: "1")
        XCTAssertTrue(window.firstResponder === view.web)
        XCTAssertTrue(view.web is NSTextInputClient)
        XCTAssertFalse(T3Composer.redirectable(window.firstResponder, in: window),
                       "a normal NSApplication key monitor must leave terminal input with its native responder")
        XCTAssertTrue(T3Composer.redirectable(window.contentView, in: window),
                      "typing on the ordinary chat background still focuses the composer")
    }

    func testLoadingTerminalDoesNotStealComposerFocus() throws {
        let view = mount(focusRequest: "1")
        XCTAssertFalse(view.ready)
        let composer = NSTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 80))
        window.contentView!.addSubview(composer)
        XCTAssertTrue(window.makeFirstResponder(composer))
        composer.insertText("focus probe", replacementRange: NSRange(location: NSNotFound, length: 0))
        spin(until: { view.ready })
        XCTAssertTrue(view.ready)
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertTrue(window.firstResponder === composer)
        XCTAssertEqual(composer.string, "focus probe")
        var next = view.props
        next["focus-request"] = "2"
        try view.setProps(next)
        XCTAssertTrue(window.firstResponder === view.web, "a new explicit request still focuses the terminal")
    }

    func testMountFocusPublishesAfterTheContractListenerIsInstalled() {
        drawerListenerReady = false
        let view = mount(focusRequest: "1")
        XCTAssertTrue(view.focused)
        XCTAssertTrue(drawerFocusEvents.isEmpty)
        drawerListenerReady = true
        spin(until: { drawerFocusEvents.contains(true) })
        XCTAssertEqual(drawerFocusEvents.first, true)
        XCTAssertTrue(messages("focus").isEmpty, "focus must use its dedicated Contract event, never the command mutation")
    }

    func testReadyRepublishesFocusWhenTheHostDroppedTheMountFact() {
        drawerDropFocus = 1
        let view = mount(focusRequest: "1")
        spin(until: { view.ready && drawerFocusEvents.contains(true) })
        XCTAssertEqual(drawerDropFocus, 0)
        XCTAssertEqual(drawerFocusEvents.last, true)
        XCTAssertTrue(window.firstResponder === view.web)
    }

    func testOpeningTerminalFocusesItBeforeAndAfterLoading() {
        let view = mount(focusRequest: "1")
        XCTAssertTrue(window.firstResponder === view.web)
        spin(until: { view.ready && view.focused })
        XCTAssertTrue(view.ready)
        XCTAssertTrue(view.focused)
        XCTAssertTrue(window.firstResponder === view.web)
    }

    func testNativeFocusAndConfiguredCommandsKeepPaneIdentityAndBlockRepeatedClose() throws {
        let bindings = T3TerminalView.json([["chord": "Meta+Alt+T", "command": "terminal.toggle"],
                                           ["chord": "Meta+D", "command": "terminal.split"], ["chord": "Meta+W", "command": "terminal.close"]])
        let view = mount(focusRequest: "1", extra: ["keybindings": bindings, "terminal-surface": "terminal:term-1"])
        XCTAssertTrue(view.focused)
        spin(until: { drawerFocusEvents.last == true })
        XCTAssertEqual(drawerFocusEvents.last, true)
        XCTAssertTrue(view.routeTerminalKey(key(2, .command, characters: "ㅇ")), "Korean physical D still splits")
        XCTAssertFalse(view.routeTerminalKey(key(2, .command, characters: "e")), "a Latin remap must not also trigger the physical D shortcut")
        XCTAssertEqual(messages("command").last?["command"] as? String, "terminal.split")
        XCTAssertEqual(messages("command").last?["terminalId"] as? String, "term-1")
        XCTAssertEqual(messages("command").last?["surface"] as? String, "terminal:term-1")
        XCTAssertTrue(view.routeTerminalKey(key(17, [.command, .option])))
        XCTAssertEqual(messages("command").last?["command"] as? String, "terminal.toggle")
        let delivered = messages("command").count
        try view.agentInput(.key("Meta+Alt+T", phase: nil))
        spin(until: { self.messages("command").count > delivered })
        XCTAssertEqual(messages("command").count, delivered + 1, "a real native key dispatch emits once, not in both monitor and page")
        XCTAssertEqual(view.dataEvents, 0, "an app command must never also enter the shell")
        XCTAssertTrue(view.routeTerminalKey(key(13, .command)))
        let before = messages("command").count
        XCTAssertTrue(view.routeTerminalKey(key(13, .command, repeat: true)))
        XCTAssertEqual(messages("command").count, before)
        var props = view.props; props["close-pending"] = "true"
        try view.setProps(props)
        let composer = NSTextView(frame: .zero)
        window.contentView!.addSubview(composer)
        XCTAssertTrue(window.makeFirstResponder(composer))
        XCTAssertFalse(view.focused, "native responder ownership changes synchronously")
        spin(until: { drawerFocusEvents.last == false })
        XCTAssertEqual(drawerFocusEvents.last, false, "focus facts publish without waiting for page blur")
        XCTAssertFalse(view.routeTerminalKey(key(13, .command)), "a background terminal cannot suppress a modal or composer shortcut")
        XCTAssertEqual(messages("command").count, before)
        XCTAssertFalse(view.routeTerminalKey(key(2, .command)), "another terminal cannot dispatch while composer owns focus")
    }

    func testTerminalReadlineKeysSendTheReferenceBytesWithoutMenuDispatch() {
        let view = mount(focusRequest: "1")
        var data = ""; view.onData = { data += $0 }
        let inputs: [(UInt16, NSEvent.ModifierFlags, String)] = [(123, .option, "\u{1b}b"), (124, .option, "\u{1b}f"),
            (123, .command, "\u{1}"), (124, .command, "\u{5}"), (51, .command, "\u{15}"), (40, .command, "\u{c}"), (37, .control, "\u{c}")]
        for (code, flags, bytes) in inputs {
            let before = data
            XCTAssertTrue(view.routeTerminalKey(key(code, flags)))
            XCTAssertEqual(data, before + bytes)
            XCTAssertFalse(view.routeTerminalKey(key(code, flags.union(.shift))))
        }
        XCTAssertTrue(messages("command").isEmpty)
    }

    func testTerminalSelectionMenusCaptureLinesAndSupersededActionsStaySilent() throws {
        let view = mount()
        let selection = try XCTUnwrap(T3TerminalActions.Selection(["text": "\r\nfirst\r\nsecond\r\n", "position": ["start": ["x": 0, "y": 2], "end": ["x": 5, "y": 3]]]))
        XCTAssertEqual(selection.text, "first\nsecond")
        XCTAssertEqual(selection.lineStart, 3); XCTAssertEqual(selection.lineEnd, 4)
        let context = view.actions.makeMenu(T3TerminalActions.menuItems(context: true, hasSelection: false, canAddToChat: true))
        XCTAssertEqual(context.items.map(\.title), ["Add to chat", "Copy", "Paste"])
        XCTAssertEqual(context.items.map(\.isEnabled), [false, false, true])
        let popup = view.actions.makeMenu(T3TerminalActions.menuItems(context: false, hasSelection: true, canAddToChat: false))
        XCTAssertEqual(popup.items.map(\.title), ["Copy"])
        view.actions.perform("add-to-chat", selection: selection, request: 0)
        XCTAssertEqual(messages("selectionAction").count, 1)
        XCTAssertEqual(messages("selectionAction").first?["text"] as? String, "first\nsecond")
        XCTAssertEqual(messages("selectionAction").first?["lineStart"] as? Int, 3)
        view.actions.dismiss(supersede: true)
        let composer = NSTextView(frame: .zero); window.contentView!.addSubview(composer)
        window.makeFirstResponder(composer)
        view.actions.perform("add-to-chat", selection: selection, request: 0)
        XCTAssertEqual(messages("selectionAction").count, 1, "a superseded menu cannot emit a chip")
        XCTAssertTrue(window.firstResponder === composer, "a superseded menu cannot steal focus")
    }

    func testAddToChatDoesNotForwardSelectionClearOrFocusAsAnotherCommand() throws {
        let view = mount()
        spin(until: { view.ready })
        drawerMessages = []; drawerFocusEvents = []
        let selection = try XCTUnwrap(T3TerminalActions.Selection(["text": "selected text", "position": ["start": ["x": 0, "y": 1], "end": ["x": 13, "y": 1]]]))
        view.actions.perform("add-to-chat", selection: selection, request: 0)
        // The real page emits this when the menu action clears its selection. Deliver it
        // through the actual WK script-message bridge after the semantic action.
        view.web.evaluateJavaScript("window.webkit.messageHandlers.t3terminal.postMessage({type:'selection',text:''})")
        spin(until: { view.bridgeLog.contains { $0.hasPrefix("selection ") } && !drawerFocusEvents.isEmpty })
        XCTAssertTrue(view.bridgeLog.contains { $0.hasPrefix("selection ") })
        XCTAssertEqual(messages("selectionAction").count, 1)
        XCTAssertEqual(drawerMessages.count, 1, "passive selection and focus facts cannot supersede Add to chat's command mutation")
        XCTAssertEqual(drawerFocusEvents.last, true, "focus uses the distinct Contract focus event")
        XCTAssertEqual(view.selection, "")
    }

    func testProviderAuthEscapeReachesItsPTYWithoutDismissingTheForm() throws {
        let view = mount(focusRequest: "1", extra: ["auth-flow": "flow:escape"])
        spin(until: { view.ready })
        try view.agentInput(.key("Escape", phase: nil))
        XCTAssertEqual(messages("auth-input").compactMap { $0["data"] as? String }.joined(), "\u{1b}")
        XCTAssertEqual(messages("auth-input").last?["authFlow"] as? String, "flow:escape")
        XCTAssertTrue(window.firstResponder === view.web)
        XCTAssertTrue(messages("command").isEmpty)
        var props = view.props; props["read-only"] = "true"; try view.setProps(props)
        try view.agentInput(.key("Escape", phase: nil))
        XCTAssertEqual(messages("auth-input").count, 1, "read-only auth still consumes Escape without writing")
    }

    func testProviderAuthTabWaitsForHostKeyLoopWithoutDoubleTraversal() {
        let view = mount(focusRequest: "1", extra: ["auth-flow": "flow:tab"])
        spin(until: { view.ready })
        XCTAssertTrue(view.ready)
        let next = NSTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 30))
        window.contentView!.addSubview(next)
        let previous = NSTextView(frame: NSRect(x: 0, y: 40, width: 100, height: 30))
        window.contentView!.addSubview(previous)
        var repaired = 0
        let hostMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard event.window === self.window, event.keyCode == 48 else { return event }
            repaired += 1
            previous.nextKeyView = view.web; view.web.nextKeyView = next; next.nextKeyView = previous
            return event
        }
        defer { if let hostMonitor { NSEvent.removeMonitor(hostMonitor) } }
        view.web.nextKeyView = nil
        NSApp.sendEvent(key(48, [], characters: "\t"))
        spin(until: { self.window.firstResponder === next })
        XCTAssertEqual(repaired, 1, "Tab reaches the host monitor that repairs stale traversal links")
        XCTAssertTrue(window.firstResponder === next, "Tab navigates the auth form rather than typing into its PTY")
        window.makeFirstResponder(view.web)
        NSApp.sendEvent(key(48, [.shift], characters: "\t"))
        spin(until: { self.window.firstResponder === previous })
        XCTAssertTrue(window.firstResponder === previous, "Shift-Tab follows the host's previous stop")
        window.makeFirstResponder(view.web)
        XCTAssertFalse(view.routeTerminalKey(key(48, [])))
        window.makeFirstResponder(next)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertTrue(window.firstResponder === next, "deferred traversal must not advance again after native traversal")
        XCTAssertTrue(messages("auth-input").isEmpty, "Tab must not write to the writable auth PTY")
    }

    func testProviderAuthOutputOffsetsInputReadonlyAndTabTraversal() throws {
        let view = mount(focusRequest: "1", extra: ["auth-flow": "flow:1", "auth-output": "first\r\n", "auth-offset": "7"])
        XCTAssertNil(view.session, "provider authentication never opens a thread shell")
        spin(until: { view.ready && self.text(view).contains("first") })
        XCTAssertTrue(text(view).contains("first"))
        var props = view.props
        props["auth-output"] = "first\r\nsecond\r\n"; props["auth-offset"] = "15"
        try view.setProps(props)
        props["auth-output"] = "third\r\n"; props["auth-offset"] = "22"
        try view.setProps(props)
        spin(until: { self.text(view).contains("third") })
        XCTAssertEqual(text(view), ["first", "second", "third"])
        try view.agentInput(.text("token"))
        spin(until: { self.messages("auth-input").count >= 5 })
        XCTAssertEqual(messages("auth-input").compactMap { $0["data"] as? String }.joined(), "token")
        XCTAssertFalse(messages("auth-resize").isEmpty)
        props["read-only"] = "true"; try view.setProps(props)
        try view.agentInput(.text("ignored"))
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertEqual(messages("auth-input").compactMap { $0["data"] as? String }.joined(), "token")
        let next = NSTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 30))
        window.contentView!.addSubview(next); view.web.nextKeyView = next
        window.makeFirstResponder(view.web)
        XCTAssertFalse(view.routeTerminalKey(key(48, [])))
        spin(until: { self.window.firstResponder === next })
        XCTAssertTrue(window.firstResponder === next)
        props["auth-flow"] = "flow:2"; props["auth-output"] = "new\r\n"; props["auth-offset"] = "5"
        try view.setProps(props)
        spin(until: { self.text(view) == ["new"] })
        XCTAssertEqual(text(view), ["new"])
    }

    func testOutputBufferMatchesTheSharedVectors() throws {
        let app = try XCTUnwrap(ProcessInfo.processInfo.environment["T3_APP_DIR"])
        let data = try Data(contentsOf: URL(fileURLWithPath: app).appendingPathComponent("macos/tests/terminal/vectors.json"))
        // JSONDecoder, not JSONSerialization: the latter drops a string's leading U+FEFF (a vector keeps one).
        let cases = try XCTUnwrap((try JSONDecoder().decode(VectorJSON.self, from: data).any as? [String: Any])?["cases"] as? [[String: Any]])
        XCTAssertGreaterThanOrEqual(cases.count, 11)
        func same(_ actual: String, _ expected: Any?, _ label: String) {
            if let string = expected as? String { XCTAssertEqual(actual, string, label); return }
            let summary = expected as? [String: Any] ?? [:]
            XCTAssertEqual(actual.utf16.count, summary["length"] as? Int, "\(label) length")
            XCTAssertEqual(String(String.UnicodeScalarView(actual.unicodeScalars.prefix(64))), summary["head"] as? String, "\(label) head")
            XCTAssertEqual(String(String.UnicodeScalarView(actual.unicodeScalars.suffix(64))), summary["tail"] as? String, "\(label) tail")
        }
        func render(_ update: T3TerminalOutput.Update) -> (String, String) {
            switch update { case .none: return ("none", ""); case .reset(let text): return ("reset", text); case .append(let text): return ("append", text) }
        }
        for vector in cases {
            let name = vector["name"] as? String ?? "?"
            var output = T3TerminalOutput(), cursor = T3TerminalOutput.Cursor.initial
            for step in vector["steps"] as? [[String: Any]] ?? [] {
                let parts = (step["parts"] as? [[Any]])?.map { String(repeating: $0[0] as? String ?? "", count: $0[1] as? Int ?? 0) }.joined()
                let input = parts ?? step["data"] as? String ?? ""
                let max = step["max"] as? Int ?? T3TerminalOutput.defaultMaxBytes
                let expect = step["expect"] as? [String: Any] ?? [:]
                var types: [String] = [], received = ""
                for _ in 0..<(step["repeat"] as? Int ?? 1) {
                    if step["op"] as? String == "reset" { output.reset(input, maxBytes: max) } else { output.append(input, maxBytes: max) }
                    if step["readEach"] as? Bool == true {
                        let next = output.read(from: cursor), (type, text) = render(next.update)
                        if !types.contains(type) { types.append(type) }
                        received += text; cursor = next.cursor
                    }
                }
                same(output.text, expect["text"], "\(name) text")
                XCTAssertEqual(output.retainedBytes, expect["retainedBytes"] as? Int, "\(name) retainedBytes")
                XCTAssertEqual(output.chunks.count, expect["chunks"] as? Int, "\(name) chunks")
                XCTAssertEqual(output.nextOffset, expect["nextOffset"] as? Int, "\(name) nextOffset")
                XCTAssertEqual(output.resetVersion, expect["resetVersion"] as? Int, "\(name) resetVersion")
                let read = expect["read"] as? [String: Any] ?? [:]
                if step["readEach"] as? Bool == true {
                    XCTAssertEqual(types.joined(separator: ","), read["type"] as? String, "\(name) read types")
                    same(received, read["data"], "\(name) received")
                } else if step["readEach"] == nil {
                    let next = output.read(from: cursor), (type, text) = render(next.update)
                    XCTAssertEqual(type, read["type"] as? String, "\(name) read type")
                    same(text, read["data"], "\(name) read data")
                    cursor = next.cursor
                }
                if let stale = expect["staleRead"] as? [String: Any] {
                    let (type, text) = render(output.read(from: .initial).update)
                    XCTAssertEqual(type, stale["type"] as? String, "\(name) stale type")
                    same(text, stale["data"], "\(name) stale data")
                }
            }
        }
    }

    func testAttachEventsFoldAsTheReferenceReducer() {
        let session = T3TerminalSession(key: "k", threadId: "thread-1", terminalId: "term-1")
        sessions.apply(["type": "snapshot", "snapshot": ["history": "hello", "status": "running", "label": "Terminal 1"]], to: session)
        XCTAssertEqual([session.status, "\(session.version)", "\(session.lifecycleVersion)", session.output.text], ["running", "1", "0", "hello"])
        sessions.apply(["type": "output", "data": " world"], to: session)
        sessions.apply(["type": "snapshot", "snapshot": ["history": "again", "status": "running"]], to: session)
        XCTAssertEqual([session.output.text, "\(session.version)", "\(session.lifecycleVersion)"], ["again", "3", "1"])
        sessions.apply(["type": "restarted", "snapshot": ["history": "", "status": "running"]], to: session)
        XCTAssertEqual(session.lifecycleVersion, 2)
        sessions.apply(["type": "error", "message": "Terminal disconnected."], to: session)
        XCTAssertEqual([session.status, session.error ?? ""], ["error", "Terminal disconnected."])
        sessions.apply(["type": "output", "data": "x"], to: session)
        XCTAssertNil(session.error)
        sessions.apply(["type": "activity", "hasRunningSubprocess": true, "label": "bun"], to: session)
        XCTAssertEqual([session.label, "\(session.hasRunningSubprocess)", "\(session.version)"], ["bun", "true", "6"])
        sessions.apply(["type": "exited", "exitCode": 0, "exitSignal": NSNull()], to: session)
        XCTAssertEqual(session.status, "exited")
    }

    func testASessionViewShowsTheBufferWritesErrorsAndReportsTheExitOnce() throws {
        try XCTSkipUnless(T3TerminalAssets.fileURL("terminal-host.js") != nil, "run terminal-host/build.mjs first")
        let view = mount()
        let session = try XCTUnwrap(view.session)
        XCTAssertEqual(session.cwd, "/repo")
        XCTAssertEqual(session.env, ["T3CODE_PROJECT_ROOT": "/repo"])
        spin(until: { view.ready })
        sessions.apply(["type": "snapshot", "snapshot": ["history": "$ echo hi\r\nhi\r\n$ ", "status": "running"]], to: session)
        view.sessionChanged(session)
        sessions.apply(["type": "output", "data": "ls\r\n"], to: session)
        view.sessionChanged(session)
        spin(until: { self.text(view).contains("$ ls") })
        XCTAssertEqual(text(view), ["$ echo hi", "hi", "$ ls"])
        sessions.apply(["type": "error", "message": "Failed to write to terminal"], to: session)
        view.sessionChanged(session)
        sessions.apply(["type": "exited", "exitCode": 0, "exitSignal": NSNull()], to: session)
        view.sessionChanged(session)
        view.sessionChanged(session)
        spin(until: { self.text(view).contains("[terminal] Process exited") && !drawerMessages.isEmpty })
        XCTAssertTrue(text(view).contains("[terminal] Failed to write to terminal"), "\(text(view))")
        let reported = messages("exited")
        XCTAssertEqual(reported.count, 1)
        XCTAssertEqual(reported.first?["terminalId"] as? String, "term-1")
        XCTAssertEqual(reported.first?["threadId"] as? String, "thread-1")
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(messages("exited").count, 1, "the exit is reported once")
        XCTAssertEqual(text(view).filter { $0 == "[terminal] Process exited" }.count, 1)
    }

    /// The module's status reply carries every terminal's status; with a view shown it must stay JSON
    /// (an ArraySlice there failed the whole status read: "the reply was not JSON").
    func testTheStatusOfShownTerminalsIsJSON() {
        let view = mount()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let status = T3Terminals.shared.status
        XCTAssertTrue(JSONSerialization.isValidJSONObject(status), "\(status)")
        XCTAssertEqual((status["terminalSessions"] as? [[String: Any]])?.first?["terminalId"] as? String, "term-1")
        XCTAssertNotNil(view.session)
    }

    func testAMountedThreadKeepsItsSessionWithoutAViewAndAnUnmountedOneLetsGo() {
        let first = mount(thread: "thread-1"), second = mount(thread: "thread-2")
        let kept = first.session!.key, dropped = second.session!.key
        sessions.retain([kept])
        first.destroy(); second.destroy(); views = []
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertNotNil(sessions.sessions[kept], "a hidden mounted thread keeps its session")
        XCTAssertNil(sessions.sessions[dropped], "a thread that is no longer mounted lets its session go")
        sessions.retain([])
        XCTAssertNil(sessions.sessions[kept])
    }
}

/// Any JSON value, decoded by JSONDecoder.
private enum VectorJSON: Decodable {
    case value(Any)
    var any: Any { if case .value(let value) = self { return value }; return NSNull() }
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .value(NSNull()) }
        else if let flag = try? container.decode(Bool.self) { self = .value(flag) }
        else if let number = try? container.decode(Int.self) { self = .value(number) }
        else if let number = try? container.decode(Double.self) { self = .value(number) }
        else if let text = try? container.decode(String.self) { self = .value(text) }
        else if let list = try? container.decode([VectorJSON].self) { self = .value(list.map(\.any)) }
        else { self = .value(try container.decode([String: VectorJSON].self).mapValues(\.any)) }
    }
}

private var drawerMessages: [String] = []
private var drawerFocusEvents: [Bool] = []
private var drawerListenerReady = true
private var drawerDropFocus = 0
private let recordDrawerEvent: ExactNativeEventFn = { _, _, kind, bytes, length in
    let text = bytes.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self) } ?? ""
    if kind == 3 || kind == 4 {
        if !drawerListenerReady { return }
        if drawerDropFocus > 0 { drawerDropFocus -= 1; return }
        drawerFocusEvents.append(kind == 3)
    }
    if kind == 8 { drawerMessages.append("\(kind):\(text)") }
}
