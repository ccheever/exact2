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
        window.orderFrontRegardless()
        transport = T3Transport(persistent: false, signals: false, changed: { _ in })
        sessions = T3TerminalSessions(transport: transport)
        sessions.detachGrace = 0.1
        drawerMessages = []
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

    private func mount(thread: String = "thread-1", terminal: String = "term-1") -> T3TerminalView {
        let props = ["terminal": terminal, "environment": "env-a", "thread": thread, "cwd": "/repo", "worktree": "", "env": "{\"T3CODE_PROJECT_ROOT\":\"/repo\"}"]
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
        let reported = drawerMessages.map { (try? JSONSerialization.jsonObject(with: Data($0.dropFirst(2).utf8))) as? [String: String] }
        XCTAssertEqual(reported, [["terminalId": "term-1", "thread": "thread-1", "type": "exited"]])
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(drawerMessages.count, 1, "the exit is reported once")
        XCTAssertEqual(text(view).filter { $0 == "[terminal] Process exited" }.count, 1)
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
private let recordDrawerEvent: ExactNativeEventFn = { _, _, kind, bytes, length in
    let text = bytes.map { String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self) } ?? ""
    if kind == 8 { drawerMessages.append("\(kind):\(text)") }
}
