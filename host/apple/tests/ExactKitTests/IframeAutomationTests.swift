import Network
import WebKit
import XCTest
#if os(macOS)
import AppKit
#else
import UIKit
#endif

#if os(macOS) || os(iOS)
/// A partitioned frame driven through the web arm's `exact_web_automation`
/// (LLP 1020.002 stage 4, D9), as `@exact/browser-automation`'s package calls
/// it once the runner admitted the call: scripts in their worlds, preload
/// scripts, input and whether the page trusts it, prompts, downloads, a
/// person taking the frame back, and calls ending with their document or
/// frame, against pages a loopback server in the test serves.
final class IframeAutomationTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias DoneFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias AutomateFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, DoneFn?) -> Void

    private final class Seen { var records: [(event: String, record: [String: Any])] = [] }
    private final class Answer { var value: [String: Any]? }

    private struct Arm {
        let create: CreateFn, view: ViewFn, setSrc: SetFn, configure: PostFn, destroy: DestroyFn, automate: AutomateFn
    }

    private func arm() throws -> Arm {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        return Arm(create: try symbol("exact_web_create", CreateFn.self), view: try symbol("exact_web_platform_view", ViewFn.self),
                   setSrc: try symbol("exact_web_set_src", SetFn.self), configure: try symbol("exact_web_configure", PostFn.self),
                   destroy: try symbol("exact_web_destroy", DestroyFn.self), automate: try symbol("exact_web_automation", AutomateFn.self))
    }

    private static let event: EventFn = { context, _, kind, bytes, length in
        let seen = Unmanaged<Seen>.fromOpaque(context!).takeUnretainedValue()
        let data = Data(bytes: bytes ?? UnsafePointer(bitPattern: 1)!, count: Int(length))
        guard kind == 4 || kind == 5, let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let name = value["event"] as? String, let record = value["record"] as? [String: Any] else { return }
        seen.records.append((name, record))
    }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }
    private static let done: DoneFn = { context, bytes, length in
        let answer = Unmanaged<Answer>.fromOpaque(context!).takeRetainedValue()
        answer.value = (try? JSONSerialization.jsonObject(with: Data(bytes: bytes!, count: Int(length)))) as? [String: Any] ?? [:]
    }

    private func json(_ value: [String: Any], _ fn: PostFn, _ handle: UnsafeMutableRawPointer) {
        let data = try! JSONSerialization.data(withJSONObject: value)
        data.withUnsafeBytes { fn(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    private func spin(until done: () -> Bool, _ seconds: TimeInterval = 15) {
        let deadline = Date(timeIntervalSinceNow: seconds)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
    }

    private func pause(_ seconds: TimeInterval) { spin(until: { false }, seconds) }

    private func say(_ text: String) {
        print(text)
        XCTContext.runActivity(named: text) { _ in }
    }

    private static let owner = "/tmp/iframe-automation-owner"

    /// A call as the package makes it; its answer once the arm gives one.
    private func call(_ arm: Arm, _ op: String, _ options: [String: Any] = [:], id: String = "tab", owner: String = owner) -> Answer {
        let answer = Answer()
        var request = options
        request["op"] = op; request["id"] = id; request["owner"] = owner
        let data = try! JSONSerialization.data(withJSONObject: request)
        let context = Unmanaged.passRetained(answer).toOpaque()
        data.withUnsafeBytes { arm.automate($0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count), context, Self.done) }
        return answer
    }

    private func answered(_ answer: Answer, _ seconds: TimeInterval = 15) -> [String: Any] {
        spin(until: { answer.value != nil }, seconds)
        return answer.value ?? ["error": "timed out"]
    }

    private func run(_ arm: Arm, _ op: String, _ options: [String: Any] = [:], id: String = "tab", owner: String = owner) -> [String: Any] {
        answered(call(arm, op, options, id: id, owner: owner))
    }

    private func value(_ arm: Arm, _ expression: String, sandbox: String? = nil) -> Any? {
        var options: [String: Any] = ["expression": expression]
        if let sandbox { options["sandbox"] = sandbox }
        return run(arm, "evaluate", options)["result"]
    }

    #if os(macOS)
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows = [] }
    #else
    private var windows: [UIWindow] = []
    override func tearDown() { windows.forEach { $0.isHidden = true }; windows = [] }
    #endif

    private func show(_ view: AnyObject) {
        #if os(macOS)
        let view = view as! NSView
        view.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(view)
        window.makeKeyAndOrderFront(nil)
        windows.append(window)
        #else
        let view = view as! UIView
        view.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let window = hostedWindowScene().map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        window.addSubview(view)
        window.isHidden = false
        windows.append(window)
        #endif
    }

    private func mount(_ arm: Arm, _ seen: Seen, partition: String? = "memory-automation", path: String = "/input", id: String = "tab") -> (UnsafeMutableRawPointer, WKWebView) {
        let handle = arm.create(11, Unmanaged.passUnretained(seen).toOpaque(), Self.event, Self.reply)!
        var configuration: [String: Any] = ["element": id, "owner": Self.owner]
        if let partition { configuration["partition"] = partition }
        json(configuration, arm.configure, handle)
        let web = Unmanaged<AnyObject>.fromOpaque(arm.view(handle)!).takeUnretainedValue() as! WKWebView
        show(web)
        Array("\(baseURL)\(path)".utf8).withUnsafeBufferPointer { arm.setSrc(handle, $0.baseAddress, UInt32($0.count), 1) }
        if partition != nil { spin { seen.records.contains { $0.event == "navigatesuccess" } } } else { pause(1) }
        pause(0.2)
        return (handle, web)
    }

    nonisolated(unsafe) private static var server: AutomationPages?
    private var baseURL: String { "http://127.0.0.1:\(Self.server!.port)" }
    override func setUpWithError() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        if Self.server == nil { Self.server = try AutomationPages() }
    }

    private func controller(_ seen: Seen) -> String? { seen.records.last { $0.event == "currententrychange" }?.record["controller"] as? String }

    /// `evaluate` and `callFunction` in the page's world or a sandbox's, a
    /// value as its JSON, an exception by its own name; a read leaves the
    /// frame the person's, `userActivation` takes it.
    func testScriptsRunInTheirWorlds() throws {
        let arm = try arm(), seen = Seen()
        let (handle, _) = mount(arm, seen)
        defer { arm.destroy(handle) }
        XCTAssertEqual(value(arm, "document.title") as? String, "Input")
        XCTAssertNil(run(arm, "evaluate", ["expression": "window.mark = 1"])["error"])
        XCTAssertEqual(value(arm, "typeof window.mark") as? String, "number")
        XCTAssertEqual(value(arm, "typeof window.mark", sandbox: "t3") as? String, "undefined", "a sandbox is an isolated world")
        XCTAssertEqual(value(arm, "document.title", sandbox: "t3") as? String, "Input", "with the page's DOM")
        XCTAssertEqual(run(arm, "callFunction", ["functionDeclaration": "(a, b) => ({ sum: a + b.n })", "arguments": [2, ["n": 3]]])["result"] as? [String: Int], ["sum": 5])
        XCTAssertEqual(value(arm, "new Promise(r => setTimeout(() => r(7), 50))") as? Int, 7)
        XCTAssertEqual(run(arm, "evaluate", ["expression": "new Promise(() => {})", "awaitPromise": false])["result"] as? [String: Int], [:])
        XCTAssert(value(arm, "undefined") is NSNull)
        let thrown = run(arm, "evaluate", ["expression": "nope()"])
        XCTAssertEqual(thrown["error"] as? String, "ReferenceError", "\(thrown)")
        XCTAssertEqual(run(arm, "evaluate", ["expression": "'x'.repeat(70000)"])["error"] as? String, "QuotaExceededError")
        XCTAssertEqual(run(arm, "evaluate", ["expression": "1", "sandbox": "exact.browser"])["error"] as? String, "SyntaxError", "the arm's own worlds are refused")
        let activation = value(arm, "String(navigator.userActivation?.isActive)") as? String
        say("iframe automation: navigator.userActivation.isActive in an evaluation = \(activation ?? "nil")")
        XCTAssertNotEqual(controller(seen), "agent", "a read leaves the frame the person's")
        XCTAssertNil(run(arm, "evaluate", ["expression": "1", "userActivation": true])["error"])
        spin { self.controller(seen) == "agent" }
        XCTAssertEqual(controller(seen), "agent", "userActivation takes the frame")
        XCTAssertEqual(run(arm, "evaluate", ["expression": "1"], owner: "/tmp/another-app")["error"] as? String, "NotFoundError", "another app's frames are not found")
        XCTAssertEqual(run(arm, "evaluate", ["expression": "1"], id: "other")["error"] as? String, "NotFoundError")
    }

    /// A preload script runs at document start in each new document until
    /// it is removed; the arm's own scripts stay. Adding or removing one is a
    /// read: the frame stays the person's.
    func testPreloadScriptsRunAtDocumentStart() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen)
        defer { arm.destroy(handle) }
        let own = web.configuration.userContentController.userScripts.count
        let added = run(arm, "addPreloadScript", ["functionDeclaration": "() => { window.early = document.readyState }"])
        let script = try XCTUnwrap(added["script"] as? String, "\(added)")
        let reload = { seen.records.removeAll(); web.reload(); self.spin { seen.records.contains { $0.event == "navigatesuccess" } } }
        reload()
        XCTAssertEqual(value(arm, "window.early") as? String, "loading")
        XCTAssertNil(run(arm, "addPreloadScript", ["functionDeclaration": "() => { window.isolated = 1 }", "sandbox": "t3"])["error"])
        XCTAssertNil(run(arm, "removePreloadScript", ["script": script])["error"])
        XCTAssertEqual(run(arm, "removePreloadScript", ["script": script])["error"] as? String, "NotFoundError")
        reload()
        XCTAssertEqual(value(arm, "typeof window.early") as? String, "undefined")
        XCTAssertEqual(value(arm, "window.isolated", sandbox: "t3") as? Int, 1)
        XCTAssertEqual(web.configuration.userContentController.userScripts.count, own + 1)
        XCTAssertEqual(controller(seen), "person", "a preload script took the frame")
    }

    /// Input reaches the page: a click, typed text, a key with a modifier
    /// held, and a wheel. Whether the page trusts it is measured (D9).
    func testInputReachesThePage() throws {
        #if os(iOS)
        guard hostedWindowScene() != nil else { throw missing("scene", "WebKit lays out a page only in an app's window scene") }
        #endif
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen)
        defer { arm.destroy(handle) }
        #if os(macOS)
        let window = try XCTUnwrap(web.window)
        final class Focusable: NSView { override var acceptsFirstResponder: Bool { true } }
        let focus = Focusable()
        window.contentView!.addSubview(focus)
        window.makeFirstResponder(focus)
        #endif
        let click: [[String: Any]] = [["type": "pointer", "id": "mouse", "actions": [
            ["type": "pointerMove", "x": 60, "y": 20], ["type": "pointerDown", "button": 0], ["type": "pointerUp", "button": 0]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": click]) as NSDictionary, [:] as NSDictionary)
        spin { (self.value(arm, "log.filter(e => e.type === 'click').length") as? Int ?? 0) > 0 }
        let clicked = value(arm, "JSON.stringify(log.filter(e => ['pointerdown', 'mousedown', 'mouseup', 'click'].includes(e.type)))") as? String ?? ""
        say("iframe automation: a click's events \(clicked)")
        XCTAssert(clicked.contains("\"click\""), clicked)
        let typing: [[String: Any]] = [["type": "pointer", "id": "mouse", "actions": [
            ["type": "pointerMove", "x": 100, "y": 70], ["type": "pointerDown", "button": 0], ["type": "pointerUp", "button": 0]]],
            ["type": "key", "id": "keys", "actions": [["type": "pause"], ["type": "pause"], ["type": "pause"],
            ["type": "keyDown", "value": "H"], ["type": "keyUp", "value": "H"], ["type": "keyDown", "value": "i"], ["type": "keyUp", "value": "i"],
            ["type": "keyDown", "value": "!"], ["type": "keyUp", "value": "!"]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": typing]) as NSDictionary, [:] as NSDictionary)
        spin { self.value(arm, "document.getElementById('field').value") as? String == "Hi!" }
        XCTAssertEqual(value(arm, "document.getElementById('field').value") as? String, "Hi!", value(arm, "JSON.stringify(log.filter(e => e.type.startsWith('key') || e.type === 'input'))") as? String ?? "")
        let keyed = value(arm, "JSON.stringify(log.filter(e => e.type === 'keydown' || e.type === 'input').slice(0, 2))") as? String ?? ""
        say("iframe automation: typed events \(keyed)")
        let chord: [[String: Any]] = [["type": "key", "id": "keys", "actions": [
            ["type": "keyDown", "value": "\u{E008}"], ["type": "keyDown", "value": "\u{E012}"], ["type": "keyUp", "value": "\u{E012}"], ["type": "keyUp", "value": "\u{E008}"]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": chord]) as NSDictionary, [:] as NSDictionary)
        spin { (self.value(arm, "log.filter(e => e.type === 'keydown' && e.key === 'ArrowLeft').length") as? Int ?? 0) > 0 }
        XCTAssertEqual(value(arm, "log.filter(e => e.type === 'keydown' && e.key === 'ArrowLeft').map(e => e.shift).join()") as? String, "true", "Shift held for ArrowLeft")
        let scroll: [[String: Any]] = [["type": "wheel", "id": "wheel", "actions": [["type": "scroll", "x": 100, "y": 200, "deltaX": 0, "deltaY": 120]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": scroll]) as NSDictionary, [:] as NSDictionary)
        spin { (self.value(arm, "document.getElementById('list').scrollTop") as? Double ?? 0) > 0 }
        let wheel = value(arm, "JSON.stringify([document.getElementById('list').scrollTop, log.filter(e => e.type === 'wheel').slice(0, 1)])") as? String ?? ""
        say("iframe automation: a wheel \(wheel)")
        XCTAssertGreaterThan(value(arm, "document.getElementById('list').scrollTop") as? Double ?? 0, 0, wheel)
        let trusted = value(arm, "[...new Set(log.map(e => e.type + ':' + e.trusted))].join(' ')") as? String ?? ""
        say("iframe automation: isTrusted by event type: \(trusted)")
        #if os(macOS)
        XCTAssert(window.firstResponder === focus, "the focus went back to what had it: \(String(describing: window.firstResponder))")
        #else
        // The events the arm dispatches are untrusted; the `input` that its `execCommand('insertText')` makes is the editor's own.
        XCTAssertFalse(trusted.split(separator: " ").contains { $0.hasSuffix(":true") && !$0.hasPrefix("input:") }, "iOS input is script-dispatched: \(trusted)")
        #endif
        XCTAssertNil(run(arm, "releaseActions")["error"])
    }

    /// An agent-controlled frame's dialog fires `userpromptopened` and waits
    /// for `handleUserPrompt`; with no dialog open the call is refused.
    func testPromptsOfAnAgentControlledFrame() throws {
        let arm = try arm(), seen = Seen()
        let (handle, _) = mount(arm, seen)
        defer { arm.destroy(handle) }
        XCTAssertEqual(run(arm, "handleUserPrompt")["error"] as? String, "NotFoundError")
        XCTAssertNil(run(arm, "releaseActions")["error"], "an acting call takes the frame")
        // WebKit sends a script's answer after the task it ran in, and a dialog
        // that task's timer opened holds it: the call is answered once the dialog is.
        let asking = call(arm, "evaluate", ["expression": "setTimeout(() => { window.answer = prompt('Name?', 'Ada') }, 0), 1"])
        spin { seen.records.contains { $0.event == "userpromptopened" } }
        let opened = try XCTUnwrap(seen.records.last { $0.event == "userpromptopened" }?.record)
        XCTAssertEqual(opened["type"] as? String, "prompt")
        XCTAssertEqual(opened["message"] as? String, "Name?")
        XCTAssertEqual(opened["defaultValue"] as? String, "Ada")
        XCTAssertNil(run(arm, "handleUserPrompt", ["accept": true, "userText": "Grace"])["error"])
        XCTAssertEqual(answered(asking)["result"] as? Int, 1)
        spin { self.value(arm, "window.answer") as? String == "Grace" }
        XCTAssertEqual(value(arm, "window.answer") as? String, "Grace")
        let confirming = call(arm, "evaluate", ["expression": "setTimeout(() => { window.sure = confirm('Sure?') }, 0), 1"])
        spin { seen.records.filter { $0.event == "userpromptopened" }.count == 2 }
        XCTAssertNil(run(arm, "handleUserPrompt", ["accept": false])["error"])
        XCTAssertNil(answered(confirming)["error"])
        spin { self.value(arm, "String(window.sure)") as? String == "false" }
        XCTAssertEqual(value(arm, "String(window.sure)") as? String, "false")
    }

    /// An agent-controlled frame's download goes to the folder, with no panel.
    func testDownloadsGoToTheFolder() throws {
        let arm = try arm(), seen = Seen()
        let (handle, _) = mount(arm, seen)
        defer { arm.destroy(handle) }
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-automation-\(ProcessInfo.processInfo.processIdentifier)/downloads", isDirectory: true)
        try? FileManager.default.removeItem(at: folder)
        XCTAssertNil(run(arm, "setDownloadBehavior", ["destinationFolder": "app:/tmp/downloads", "destinationFolderFile": folder.path])["error"])
        XCTAssertNil(run(arm, "evaluate", ["expression": "location.href = '/download', 1"])["error"])
        spin { seen.records.contains { $0.event == "download" && $0.record["state"] as? String == "completed" } }
        let record = try XCTUnwrap(seen.records.last { $0.event == "download" }?.record)
        XCTAssertEqual(record["path"] as? String, "app:/tmp/downloads/report.txt", "\(record)")
        XCTAssertEqual(try? String(contentsOf: folder.appendingPathComponent("report.txt"), encoding: .utf8), "a report\n")
    }

    /// A call ends with its document (`AbortError`) or its frame
    /// (`NotFoundError`); an unpartitioned frame is not driven, but any
    /// frame's picture is taken.
    func testCallsEndWithTheirDocumentOrFrame() throws {
        let arm = try arm(), seen = Seen()
        let (handle, _) = mount(arm, seen)
        let navigated = call(arm, "evaluate", ["expression": "new Promise(r => { window.held = r })"])
        pause(0.2)
        XCTAssertNil(navigated.value)
        XCTAssertNil(run(arm, "evaluate", ["expression": "location.href = '/other', 1"])["error"])
        XCTAssertEqual(answered(navigated)["error"] as? String, "AbortError")
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-automation-\(ProcessInfo.processInfo.processIdentifier)/shot.png")
        let shot = run(arm, "captureScreenshot", ["path": "app:/tmp/shot.png", "pathFile": file.path, "clip": ["x": 0, "y": 0, "width": 100, "height": 50]])
        XCTAssertEqual(shot["path"] as? String, "app:/tmp/shot.png", "\(shot)")
        XCTAssertGreaterThan(shot["width"] as? Int ?? 0, 99, "\(shot)")
        // A promise the page holds: WebKit drops a call whose promise is collected.
        let unmounted = call(arm, "evaluate", ["expression": "new Promise(r => { window.held = r })"])
        pause(0.2)
        arm.destroy(handle)
        let ended = answered(unmounted)
        XCTAssertEqual(ended["error"] as? String, "NotFoundError", "\(ended)")
        XCTAssertEqual(run(arm, "evaluate", ["expression": "1"])["error"] as? String, "NotFoundError")

        let plain = Seen()
        let (unpartitioned, _) = mount(arm, plain, partition: nil, id: "plain")
        defer { arm.destroy(unpartitioned) }
        XCTAssertEqual(run(arm, "evaluate", ["expression": "1"], id: "plain")["error"] as? String, "NotSupportedError")
    }

    #if os(macOS)
    /// A key the page leaves unhandled, which WebKit sends on through
    /// `NSApp.sendEvent`, is neither a person's (the frame stays the agent's)
    /// nor the app's menus' (⌘W does not close).
    func testAKeyThePageLeavesIsNotAPersonsNorTheMenus() throws {
        final class Sink: NSObject { var fired = 0; @objc func close(_ sender: Any?) { fired += 1 } }
        let sink = Sink(), menu = NSMenu(), top = NSMenuItem(), sub = NSMenu()
        let close = NSMenuItem(title: "Close", action: #selector(Sink.close(_:)), keyEquivalent: "w")
        close.target = sink
        sub.addItem(close); top.submenu = sub; menu.addItem(top)
        let before = NSApp.mainMenu
        NSApp.mainMenu = menu
        defer { NSApp.mainMenu = before }
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen)
        defer { arm.destroy(handle) }
        XCTAssertNil(run(arm, "releaseActions")["error"])
        spin { self.controller(seen) == "agent" }
        let escape: [[String: Any]] = [["type": "key", "id": "k", "actions": [["type": "keyDown", "value": "\u{E00C}"], ["type": "keyUp", "value": "\u{E00C}"]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": escape]) as NSDictionary, [:] as NSDictionary)
        pause(0.5)
        XCTAssertEqual(controller(seen), "agent", "an unhandled Escape is not a person's")
        let closeChord: [[String: Any]] = [["type": "key", "id": "k", "actions": [["type": "keyDown", "value": "\u{E03D}"], ["type": "keyDown", "value": "w"],
                                                                                ["type": "keyUp", "value": "w"], ["type": "keyUp", "value": "\u{E03D}"]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": closeChord]) as NSDictionary, [:] as NSDictionary)
        pause(0.5)
        XCTAssertEqual(sink.fired, 0, "⌘W reached the app's menu")
        XCTAssertTrue(web.window?.isVisible ?? false)
        XCTAssertEqual(controller(seen), "agent")
        let outside: [[String: Any]] = [["type": "pointer", "id": "m", "actions": [["type": "pointerMove", "x": 60, "y": 515], ["type": "pointerDown", "button": 0]]]]
        XCTAssertEqual(run(arm, "performActions", ["actions": outside])["error"] as? String, "move target out of bounds")
    }
    #endif

    #if os(macOS)
    /// A person's click in the frame takes it back: input in flight ends
    /// with `AbortError`, and a held dialog becomes the person's sheet.
    func testAPersonTakesTheFrameBack() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen)
        defer { arm.destroy(handle) }
        let window = try XCTUnwrap(web.window)
        let long: [[String: Any]] = [["type": "none", "id": "wait", "actions": [["type": "pause", "duration": 5000]]]]
        let acting = call(arm, "performActions", ["actions": long])
        spin { self.controller(seen) == "agent" }
        XCTAssertEqual(controller(seen), "agent")
        _ = call(arm, "evaluate", ["expression": "setTimeout(() => alert('Held'), 0), 1"])
        spin { seen.records.contains { $0.event == "userpromptopened" } }
        XCTAssertNil(window.attachedSheet, "no sheet while the agent has the frame")
        let point = web.convert(NSPoint(x: 200, y: 150), to: nil)
        let person = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                      windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        NSApp.sendEvent(person)
        XCTAssertEqual(answered(acting)["error"] as? String, "AbortError")
        spin { self.controller(seen) == "person" }
        XCTAssertEqual(controller(seen), "person")
        spin { window.attachedSheet != nil }
        let sheet = try XCTUnwrap(window.attachedSheet, "the held dialog, shown to the person")
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        pause(0.3)
        XCTAssertEqual(value(arm, "log.filter(e => e.type === 'mousedown').length") as? Int, 0, "the click that brought the dialog up is the dialog's, not the page's")
    }
    #endif
}

/// Pages on a loopback port: `/input`, a page that logs its events (type,
/// `isTrusted`, key, Shift) into `window.log`, with a button, a field and a
/// scrolling list; `/download`, an attachment; `/other`, a second document.
final class AutomationPages {
    private let listener: NWListener

    var port: UInt16 { listener.port!.rawValue }

    init() throws { listener = try loopbackListener(Self.serve) }

    static let input = """
    <!doctype html><meta name="viewport" content="width=device-width, initial-scale=1"><title>Input</title>
    <style>body{margin:0;font:16px system-ui}button{position:absolute;left:20px;top:5px;width:80px;height:30px}
    input{position:absolute;left:20px;top:55px;width:200px;height:30px}
    #list{position:absolute;left:20px;top:120px;width:200px;height:150px;overflow:auto}#list div{height:40px}</style>
    <button id="button">Button</button><input id="field"><div id="list"></div>
    <script>
    window.log = [];
    for (let i = 0; i < 30; i++) list.append(Object.assign(document.createElement('div'), { textContent: 'Row ' + i }));
    for (const type of ['pointerdown', 'mousedown', 'mouseup', 'click', 'keydown', 'keyup', 'input', 'wheel'])
      addEventListener(type, e => log.push({ type, trusted: e.isTrusted, key: e.key, shift: e.shiftKey }), true);
    </script>
    """

    private static func serve(_ connection: NWConnection) {
        connection.start(queue: .main)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, _, _ in
            guard let data, let line = String(decoding: data, as: UTF8.self).split(separator: "\r\n").first else { return connection.cancel() }
            let path = line.split(separator: " ").dropFirst().first.map(String.init) ?? "/"
            let (status, type, extra, body): (String, String, String, String) = switch path {
            case "/input": ("200 OK", "text/html; charset=utf-8", "", input)
            case "/other": ("200 OK", "text/html; charset=utf-8", "", "<!doctype html><title>Other</title><p>Other")
            case "/download": ("200 OK", "text/plain", "Content-Disposition: attachment; filename=\"report.txt\"\r\n", "a report\n")
            default: ("404 Not Found", "text/plain", "", "")
            }
            let response = "HTTP/1.1 \(status)\r\nContent-Type: \(type)\r\n\(extra)Content-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n\(body)"
            connection.send(content: Data(response.utf8), completion: .contentProcessed { _ in connection.cancel() })
        }
    }
}
#endif
