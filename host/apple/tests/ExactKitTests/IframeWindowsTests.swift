import Network
import WebKit
import XCTest
#if os(macOS)
import AppKit
#else
import UIKit
#endif

#if os(macOS) || os(iOS)
/// An iframe's new windows, dialogs, file inputs, downloads, permissions and
/// other apps' schemes through the web arm's C ABI (LLP 1020.002 stage 2,
/// D5 and §10), against pages a loopback server in the test serves. A
/// script the test runs with `evaluateJavaScript` carries a person's
/// gesture, as WebKit runs it.
final class IframeWindowsTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    /// What one frame reported: its `newwindow` and `download` records (kind
    /// 5), its navigation events (kind 4), and the requests it held for the
    /// agent (kind 6).
    private final class Seen {
        var records: [(event: String, record: [String: Any])] = []
        var navigation: [(event: String, record: [String: Any])] = []
        var asked: [[String: Any]] = []
        var messages: [String] = []
    }

    private struct Arm {
        let create: CreateFn, view: ViewFn, setSrc: SetFn, setSandbox: SetFn, configure: PostFn, command: PostFn, answer: PostFn, destroy: DestroyFn
    }

    private func arm() throws -> Arm {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        return Arm(create: try symbol("exact_web_create", CreateFn.self), view: try symbol("exact_web_platform_view", ViewFn.self),
                   setSrc: try symbol("exact_web_set_src", SetFn.self), setSandbox: try symbol("exact_web_set_sandbox", SetFn.self), configure: try symbol("exact_web_configure", PostFn.self),
                   command: try symbol("exact_web_command", PostFn.self), answer: try symbol("exact_web_answer", PostFn.self),
                   destroy: try symbol("exact_web_destroy", DestroyFn.self))
    }

    private static let event: EventFn = { context, _, kind, bytes, length in
        let seen = Unmanaged<Seen>.fromOpaque(context!).takeUnretainedValue()
        let data = Data(bytes: bytes ?? UnsafePointer(bitPattern: 1)!, count: Int(length))
        if kind == 1 { return seen.messages.append(String(decoding: data, as: UTF8.self)) }
        guard let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        if kind == 6 { return seen.asked.append(value) }
        guard let name = value["event"] as? String, let record = value["record"] as? [String: Any] else { return }
        if kind == 5 { seen.records.append((name, record)) } else if kind == 4 { seen.navigation.append((name, record)) }
    }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }

    private func json(_ value: [String: Any], _ fn: PostFn, _ handle: UnsafeMutableRawPointer) {
        let data = try! JSONSerialization.data(withJSONObject: value)
        data.withUnsafeBytes { fn(handle, $0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count)) }
    }

    private func spin(until done: () -> Bool, _ seconds: TimeInterval = 10) {
        let deadline = Date(timeIntervalSinceNow: seconds)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
    }

    private func pause(_ seconds: TimeInterval) { spin(until: { false }, seconds) }

    /// The page's answer to a script, run with a person's gesture.
    private func run(_ web: WKWebView, _ script: String) -> Any? {
        var result: Any?, done = false
        web.evaluateJavaScript(script) { value, error in result = value ?? error.map { "error: \($0.localizedDescription)" }; done = true }
        spin { done }
        return result
    }

    /// A script started and left running (it waits on a dialog or a panel).
    private func start(_ web: WKWebView, _ script: String) {
        web.callAsyncJavaScript(script, arguments: [:], in: nil, in: .page) { _ in }
    }

    private func loaded(_ seen: Seen, _ web: WKWebView, _ path: String) {
        spin { web.url?.path == path && !web.isLoading && seen.navigation.contains { $0.event == "navigatesuccess" } }
        pause(0.2)
    }

    private func mount(_ arm: Arm, _ seen: Seen, configuration: [String: Any], src: String, sandbox: String? = nil) -> (UnsafeMutableRawPointer, WKWebView) {
        let handle = arm.create(9, Unmanaged.passUnretained(seen).toOpaque(), Self.event, Self.reply)!
        json(configuration, arm.configure, handle)
        let web = Unmanaged<WKWebView>.fromOpaque(arm.view(handle)!).takeUnretainedValue()
        web.frame = CGRect(x: 0, y: 0, width: 480, height: 360)
        #if os(macOS)
        let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 480, height: 360), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(web)
        window.orderFrontRegardless()
        windows.append(window)
        #else
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 480, height: 360))
        let root = UIViewController()
        root.view.addSubview(web)
        window.rootViewController = root
        window.isHidden = false
        windows.append(window)
        #endif
        if let sandbox { Array(sandbox.utf8).withUnsafeBufferPointer { arm.setSandbox(handle, $0.baseAddress, UInt32($0.count), 1) } }
        Array(src.utf8).withUnsafeBufferPointer { arm.setSrc(handle, $0.baseAddress, UInt32($0.count), 1) }
        return (handle, web)
    }

    #if os(macOS)
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows = [] }
    /// The popup windows the arm opened: their web views, in no test window.
    private func popups() -> [WKWebView] {
        NSApp.windows.filter { $0.isVisible && !windows.contains($0) }.compactMap { $0.contentView as? WKWebView }
    }
    #else
    private var windows: [UIWindow] = []
    override func tearDown() { windows.forEach { $0.isHidden = true }; windows = [] }
    #endif

    nonisolated(unsafe) private static var server: WindowPages?
    private var baseURL: String { "http://127.0.0.1:\(Self.server!.port)" }
    override func setUpWithError() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        if Self.server == nil { Self.server = try WindowPages() }
    }

    private func events(_ seen: Seen) -> [String] { seen.records.map(\.event) }

    // MARK: New windows (D5's popup table)

    func testWindowRequestsFollowThePopupTable() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "windows", "newwindow": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")

        // `target=_blank`: a foreground tab, the app's to open; the frame stays.
        _ = run(web, "document.getElementById('blank').click()")
        spin { !seen.records.isEmpty }
        XCTAssertEqual(seen.records.first?.event, "newwindow")
        XCTAssertEqual(seen.records.first?.record["url"] as? String, "\(baseURL)/b")
        XCTAssertEqual(seen.records.first?.record["disposition"] as? String, "foreground-tab")
        XCTAssertEqual(web.url?.path, "/opener")

        // `window.open` without features: a foreground tab too.
        seen.records = []
        _ = run(web, "window.open('/b'); 0")
        spin { !seen.records.isEmpty }
        XCTAssertEqual(seen.records.first?.record["disposition"] as? String, "foreground-tab")

        #if os(macOS)
        // A ⌘-click on a link with no target: a background tab, never this
        // frame. WebKit takes a click's keys from the platform event, not a
        // script's `metaKey`, so the click is AppKit's, sent to the view.
        seen.records = []
        click(web, "plain", modifiers: .command)
        spin { !seen.records.isEmpty }
        XCTAssertEqual(seen.records.first?.record["disposition"] as? String, "background-tab", "\(seen.records)")
        pause(0.5)
        XCTAssertEqual(web.url?.path, "/opener")
        #endif

        // `about:blank` is refused: `window.open()` answers null, no event.
        seen.records = []
        XCTAssertEqual(run(web, "window.open('about:blank') === null") as? Bool, true)
        XCTAssertEqual(run(web, "window.open() === null") as? Bool, true)
        pause(0.3)
        XCTAssertEqual(events(seen), [])

        // A form POST to a new tab loads in the frame itself.
        seen.navigation = []
        _ = run(web, "document.getElementById('post').submit(); 0")
        loaded(seen, web, "/posted")
        XCTAssertEqual(run(web, "document.body.textContent") as? String, "POST")
        XCTAssertEqual(events(seen), [], "a POST opens no tab")
    }

    func testAScriptedWindowIsThePlatformsPopupAndPostsToItsOpener() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "popups", "newwindow": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        XCTAssertEqual(run(web, "window.open('/popup', 'signin', 'width=320,height=240') !== null") as? Bool, true)
        spin { !seen.records.isEmpty }
        XCTAssertEqual(seen.records.first?.event, "newwindow")
        XCTAssertEqual(seen.records.first?.record["disposition"] as? String, "new-window")
        XCTAssertEqual(seen.records.first?.record["url"] as? String, "\(baseURL)/popup")
        // The popup posts to its opener on load (OAuth's way back): the opener retitles itself.
        spin { web.title == "got signed-in" }
        XCTAssertEqual(web.title, "got signed-in")
        #if os(macOS)
        let popup = try XCTUnwrap(popups().first, "a popup window opened")
        XCTAssertEqual(popup.window?.title, "Popup")
        XCTAssertEqual(popup.window?.contentLayoutRect.size, CGSize(width: 320, height: 240))
        // Its opener's partition, none of its opener's scripts.
        XCTAssertTrue(popup.configuration.websiteDataStore === web.configuration.websiteDataStore)
        XCTAssertTrue(popup.configuration.userContentController.userScripts.isEmpty)
        XCTAssertFalse(popup.configuration.userContentController === web.configuration.userContentController)
        // Its own popups are refused: one level.
        XCTAssertEqual(run(popup, "window.open('/b', 'x', 'width=100,height=100') === null") as? Bool, true)
        // The app's `exact-app` origin is refused in it.
        _ = run(popup, "location.href = 'exact-app://app/assets/index.html'; 0")
        pause(0.5)
        XCTAssertEqual(popup.url?.path, "/popup")
        // `window.close()` closes it.
        _ = run(popup, "window.close(); 0")
        spin { self.popups().isEmpty }
        XCTAssertTrue(popups().isEmpty)
        #endif
    }

    func testAFrameWithoutNewwindowOpensNothing() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "closed"], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        _ = run(web, "document.getElementById('blank').click(); 0")
        XCTAssertEqual(run(web, "window.open('/popup', 'signin', 'width=320,height=240') === null") as? Bool, true)
        pause(0.5)
        XCTAssertEqual(events(seen), [])
        XCTAssertEqual(web.url?.path, "/opener")
        #if os(macOS)
        XCTAssertTrue(popups().isEmpty)
        #endif
    }

    #if os(macOS)
    /// A `mailto:` link with `target=_blank`: the platform's confirmation,
    /// never a popup or a `newwindow` (LLP 1020.002 §11, stage 2's case).
    func testAnotherAppsSchemeAsksFirst() throws {
        guard NSWorkspace.shared.urlForApplication(toOpen: URL(string: "mailto:a@b.test")!) != nil else {
            throw XCTSkip("no app opens mailto: on this Mac")
        }
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "schemes", "newwindow": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        _ = run(web, "document.getElementById('mail').click(); 0")
        let window = try XCTUnwrap(web.window)
        spin { window.attachedSheet != nil }
        let sheet = try XCTUnwrap(window.attachedSheet, "a confirmation sheet")
        let texts = sheet.contentView.map(allText) ?? []
        XCTAssertTrue(texts.contains { $0.hasPrefix("Do you want to allow this page to open “") }, "\(texts)")
        XCTAssertTrue(texts.contains("mailto:a@b.test"), "\(texts)")
        window.endSheet(sheet, returnCode: .alertSecondButtonReturn)
        pause(0.3)
        XCTAssertEqual(events(seen), [], "never a newwindow")
        XCTAssertTrue(popups().isEmpty, "never a popup")
        XCTAssertEqual(web.url?.path, "/opener")
        // `navigate(id, "mailto:…")` asks the same way, and loads nothing.
        json(["op": "navigate", "url": "mailto:c@d.test"], arm.command, handle)
        spin { window.attachedSheet != nil }
        XCTAssertNotNil(window.attachedSheet)
        if let again = window.attachedSheet { window.endSheet(again, returnCode: .alertSecondButtonReturn) }
        XCTAssertEqual(web.url?.path, "/opener")
    }

    // MARK: Dialogs, as sheets titled with the guest's origin

    func testDialogsAreSheetsTitledWithTheOrigin() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "dialogs"], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        let window = try XCTUnwrap(web.window)
        start(web, "window.alerted = (alert('Saved.'), 'done')")
        spin { window.attachedSheet != nil }
        var sheet = try XCTUnwrap(window.attachedSheet)
        XCTAssertEqual(Set(sheet.contentView.map(allText) ?? []).intersection(["127.0.0.1:\(Self.server!.port)", "Saved."]).count, 2)
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        spin { self.run(web, "window.alerted") as? String == "done" }

        start(web, "window.confirmed = confirm('Delete?')")
        spin { window.attachedSheet != nil }
        sheet = try XCTUnwrap(window.attachedSheet)
        window.endSheet(sheet, returnCode: .alertSecondButtonReturn)
        spin { self.run(web, "typeof window.confirmed") as? String == "boolean" }
        XCTAssertEqual(run(web, "window.confirmed") as? Bool, false, "Cancel")

        // `prompt` is answered: the platform's field, with the default text.
        start(web, "window.named = prompt('Name?', 'Ada')")
        spin { window.attachedSheet != nil }
        sheet = try XCTUnwrap(window.attachedSheet)
        let field = try XCTUnwrap(fields(sheet.contentView!).first)
        XCTAssertEqual(field.stringValue, "Ada")
        field.stringValue = "Grace"
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        spin { self.run(web, "typeof window.named") as? String == "string" }
        XCTAssertEqual(run(web, "window.named") as? String, "Grace")
    }

    /// A click at an element's centre, as AppKit hands one to the web view.
    private func click(_ web: WKWebView, _ id: String, modifiers: NSEvent.ModifierFlags) {
        let box = run(web, "(r => [r.x + r.width / 2, r.y + r.height / 2])(document.getElementById('\(id)').getBoundingClientRect())") as? [Double] ?? [0, 0]
        let point = web.convert(NSPoint(x: box[0], y: web.isFlipped ? box[1] : web.bounds.height - box[1]), to: nil)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            guard let event = NSEvent.mouseEvent(with: type, location: point, modifierFlags: modifiers, timestamp: ProcessInfo.processInfo.systemUptime,
                                                 windowNumber: web.window?.windowNumber ?? 0, context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)
            else { continue }
            if type == .leftMouseDown { web.mouseDown(with: event) } else { web.mouseUp(with: event) }
        }
    }

    private func allText(_ view: NSView) -> [String] {
        ((view as? NSTextField).map { [$0.stringValue] } ?? []) + view.subviews.flatMap(allText)
    }

    private func fields(_ view: NSView) -> [NSTextField] {
        ((view as? NSTextField).flatMap { $0.isEditable ? [$0] : nil } ?? []) + view.subviews.flatMap(fields)
    }
    #endif

    // MARK: Under the agent: requests a drive answers (§10)

    func testUnderTheAgentDialogsFilesAndDownloadsAreRequests() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "agent", "agent": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        let answer = { (fields: [String: Any]) in
            guard let token = seen.asked.last?["token"] else { return XCTFail("no request") }
            self.json(fields.merging(["token": token]) { a, _ in a }, arm.answer, handle)
        }

        start(web, "window.alerted = (alert('Saved.'), 'done')")
        spin { !seen.asked.isEmpty }
        XCTAssertEqual(seen.asked.last?["capability"] as? String, "dialog")
        let args = try XCTUnwrap(seen.asked.last?["args"] as? [String: Any])
        XCTAssertEqual(args["type"] as? String, "alert")
        XCTAssertEqual(args["message"] as? String, "Saved.")
        XCTAssertEqual(args["origin"] as? String, "127.0.0.1:\(Self.server!.port)")
        #if os(macOS)
        XCTAssertNil(web.window?.attachedSheet, "no sheet under the agent")
        #endif
        answer([:])
        spin { self.run(web, "window.alerted") as? String == "done" }

        seen.asked = []
        start(web, "window.named = prompt('Name?', 'Ada')")
        spin { !seen.asked.isEmpty }
        XCTAssertEqual((seen.asked.last?["args"] as? [String: Any])?["defaultText"] as? String, "Ada")
        answer(["text": "Grace"])
        spin { self.run(web, "typeof window.named") as? String == "string" }
        XCTAssertEqual(run(web, "window.named") as? String, "Grace")

        seen.asked = []
        start(web, "window.confirmed = confirm('Delete?')")
        spin { !seen.asked.isEmpty }
        answer(["cancel": true])
        spin { self.run(web, "typeof window.confirmed") as? String == "boolean" }
        XCTAssertEqual(run(web, "window.confirmed") as? Bool, false)

        // A file input: `open-file`, answered with a path.
        #if os(macOS)
        let picked = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-windows-picked.txt")
        try Data("picked".utf8).write(to: picked)
        seen.asked = []
        _ = run(web, "document.getElementById('file').click(); 0")
        spin { !seen.asked.isEmpty }
        XCTAssertEqual(seen.asked.last?["capability"] as? String, "open-file")
        XCTAssertEqual((seen.asked.last?["args"] as? [String: Any])?["multiple"] as? Bool, false)
        answer(["paths": [picked.path]])
        spin { self.run(web, "document.getElementById('file').files.length") as? Int == 1 }
        XCTAssertEqual(run(web, "document.getElementById('file').files[0].name") as? String, "iframe-windows-picked.txt")
        #endif

        // A download: `save-file`, then `progressing` and `completed` with the answered place.
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-windows-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: folder) }
        seen.asked = []
        json(["op": "navigate", "url": "\(baseURL)/report"], arm.command, handle)
        spin { !seen.asked.isEmpty }
        XCTAssertEqual(seen.asked.last?["capability"] as? String, "save-file")
        XCTAssertEqual((seen.asked.last?["args"] as? [String: Any])?["suggestedFilename"] as? String, "report.txt")
        let target = folder.appendingPathComponent("report.txt")
        answer(["path": target.path, "shown": "app:/tmp/downloads/report.txt"])
        spin { self.events(seen).filter { $0 == "download" }.count >= 2 }
        XCTAssertEqual(seen.records.map { $0.record["state"] as? String }, ["progressing", "completed"], "\(seen.records)")
        XCTAssertEqual(seen.records.last?.record["path"] as? String, "app:/tmp/downloads/report.txt")
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "\(baseURL)/report")
        XCTAssertEqual(try? String(contentsOf: target, encoding: .utf8), "report body")
        // The download's provisional URL shows until WebKit ends that navigation.
        spin { web.url?.path == "/opener" }
        XCTAssertEqual(web.url?.path, "/opener", "a download leaves the page")

        // A cancelled destination: `cancelled`.
        seen.records = []
        seen.asked = []
        json(["op": "navigate", "url": "\(baseURL)/report"], arm.command, handle)
        spin { !seen.asked.isEmpty }
        answer(["cancel": true])
        spin { self.events(seen).count >= 2 }
        XCTAssertEqual(seen.records.map { $0.record["state"] as? String }, ["progressing", "cancelled"])
    }

    /// A request still held when its frame goes ends unanswered: the page's
    /// `confirm` gets Cancel.
    func testAHeldRequestEndsWithItsFrame() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "gone", "agent": true], src: "\(baseURL)/opener")
        loaded(seen, web, "/opener")
        var answered: Any?
        web.evaluateJavaScript("confirm('Leave?')") { value, _ in answered = value ?? "none" }
        spin { !seen.asked.isEmpty }
        arm.destroy(handle)
        spin { answered != nil }
        XCTAssertEqual(answered as? Bool, false)
    }

    // MARK: Permissions (§12 Q8)

    /// Each permission the arm's delegate is asked for is denied. WebKit asks
    /// it only in a process that may use the device (a usage string, a
    /// location authorization), which this test process is not, so the test
    /// asks the delegate itself, with a frame the page really has; the
    /// fixture drives measured what a guest sees (RFC §11).
    func testPermissionsAGuestAsksForAreDenied() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "permissions"], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        let probe = FrameProbe()
        web.configuration.userContentController.add(probe, name: "probe")
        defer { web.configuration.userContentController.removeScriptMessageHandler(forName: "probe") }
        _ = run(web, "webkit.messageHandlers.probe.postMessage(1); 0")
        spin { probe.frame != nil }
        let frame = try XCTUnwrap(probe.frame)
        let ui = try XCTUnwrap(web.uiDelegate)
        var decisions: [String: WKPermissionDecision] = [:]
        ui.webView?(web, requestMediaCapturePermissionFor: frame.securityOrigin, initiatedByFrame: frame, type: .camera) { decisions["camera"] = $0 }
        ui.webView?(web, requestMediaCapturePermissionFor: frame.securityOrigin, initiatedByFrame: frame, type: .microphone) { decisions["microphone"] = $0 }
        if #available(macOS 27.0, iOS 27.0, *) {
            ui.webView?(web, requestGeolocationPermissionFor: frame.securityOrigin, initiatedByFrame: frame) { decisions["location"] = $0 }
            XCTAssertEqual(decisions["location"], .deny)
        }
        #if os(iOS)
        ui.webView?(web, requestDeviceOrientationAndMotionPermissionFor: frame.securityOrigin, initiatedByFrame: frame) { decisions["motion"] = $0 }
        XCTAssertEqual(decisions["motion"], .deny)
        #endif
        XCTAssertEqual(decisions["camera"], .deny)
        XCTAssertEqual(decisions["microphone"], .deny)
        // What the page itself sees of notifications and fullscreen, which no delegate decides.
        var notification = "", done = false
        web.callAsyncJavaScript("return typeof Notification === 'undefined' ? 'none' : Notification.permission + ' ' + await Notification.requestPermission()",
                                arguments: [:], in: nil, in: .page) { if case .success(let v) = $0 { notification = "\(v)" }; done = true }
        spin { done }
        #if os(macOS)
        XCTAssertEqual(notification, "default denied")
        #else
        XCTAssertEqual(notification, "none")
        #endif
        XCTAssertEqual(run(web, "document.fullscreenEnabled") as? Bool, true, "element fullscreen is on")
    }

    /// A guest the wrapper's inner frame holds (a sandboxed remote page, and
    /// on iOS any remote one) is allowed fullscreen, as a top-level guest is
    /// (LLP 1020.002 D5); without `allow="fullscreen *"` its
    /// `fullscreenEnabled` is false.
    func testAGuestInTheWrapperIsAllowedFullscreen() throws {
        let arm = try arm()
        for sandbox in ["allow-scripts", "allow-scripts allow-forms allow-popups allow-downloads", nil] {
            let seen = Seen()
            let (handle, web) = mount(arm, seen, configuration: [:], src: "\(baseURL)/fullscreen", sandbox: sandbox)
            defer { arm.destroy(handle) }
            spin { !seen.messages.isEmpty || web.url?.path == "/fullscreen" && !web.isLoading }
            if web.url?.path == "/fullscreen" {
                XCTAssertNil(sandbox, "only an unsandboxed remote page is direct")
                XCTAssertEqual(run(web, "document.fullscreenEnabled") as? Bool, true)
            } else {
                XCTAssertEqual(seen.messages, ["fullscreen true"], sandbox ?? "unsandboxed")
            }
        }
    }

    // MARK: Review round 1

    /// A download's destination that is a folder is refused, never replaced:
    /// an agent's typed `/Users/me/Downloads` or `/` must not delete it.
    func testADownloadNeverReplacesAFolder() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "folders", "agent": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-windows-folder-\(UUID().uuidString)")
        let kept = folder.appendingPathComponent("kept.txt")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try Data("kept".utf8).write(to: kept)
        defer { try? FileManager.default.removeItem(at: folder) }
        for (path, shown) in [(folder.path, folder.path), (folder.path + "/", folder.path + "/"), ("/", "/")] {
            seen.records = []
            seen.asked = []
            json(["op": "navigate", "url": "\(baseURL)/report"], arm.command, handle)
            spin { !seen.asked.isEmpty }
            json(["token": seen.asked.last?["token"] ?? 0, "path": path, "shown": shown], arm.answer, handle)
            spin { seen.records.count >= 2 }
            XCTAssertEqual(seen.records.map { $0.record["state"] as? String }, ["progressing", "cancelled"], path)
            XCTAssertEqual(try? String(contentsOf: kept, encoding: .utf8), "kept", "\(path) left the folder as it was")
        }
    }

    #if os(macOS)
    /// Another app's scheme asks only for a link a person follows, and a
    /// page whose confirmation was cancelled asks no more until it navigates.
    func testAnotherAppsSchemeCannotAskInALoop() throws {
        guard NSWorkspace.shared.urlForApplication(toOpen: URL(string: "mailto:a@b.test")!) != nil else {
            throw XCTSkip("no app opens mailto: on this Mac")
        }
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "loops"], src: "\(baseURL)/timer-scheme")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/timer-scheme")
        let window = try XCTUnwrap(web.window)
        // The page sets `location` to `mailto:` every 200 ms, with no person's gesture.
        pause(1)
        XCTAssertNil(window.attachedSheet, "a script's navigation never asks")
        _ = run(web, "document.getElementById('mail').click(); 0")
        spin { window.attachedSheet != nil }
        let sheet = try XCTUnwrap(window.attachedSheet)
        window.endSheet(sheet, returnCode: .alertSecondButtonReturn)
        pause(0.3)
        _ = run(web, "document.getElementById('mail').click(); 0")
        pause(0.8)
        XCTAssertNil(window.attachedSheet, "a page whose confirmation was cancelled asks no more")
        // A reload keeps the refusal; the app's own navigation clears it.
        json(["op": "reload"], arm.command, handle)
        pause(1)
        _ = run(web, "document.getElementById('mail').click(); 0")
        pause(0.8)
        XCTAssertNil(window.attachedSheet, "a reload does not start over")
        seen.navigation = []
        json(["op": "navigate", "url": "\(baseURL)/timer-scheme"], arm.command, handle)
        loaded(seen, web, "/timer-scheme")
        _ = run(web, "document.getElementById('mail').click(); 0")
        spin { window.attachedSheet != nil }
        XCTAssertNotNil(window.attachedSheet)
        if let again = window.attachedSheet { window.endSheet(again, returnCode: .alertSecondButtonReturn) }
    }

    /// From a page's second dialog on, the sheet offers to prevent more; once
    /// chosen, the page's dialogs answer at once with no sheet.
    func testAPageCanBeStoppedFromRaisingMoreDialogs() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "suppress"], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        let window = try XCTUnwrap(web.window)
        start(web, "alert('one')")
        spin { window.attachedSheet != nil }
        var sheet = try XCTUnwrap(window.attachedSheet)
        XCTAssertFalse(suppression(sheet) != nil, "the first dialog offers no suppression")
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        pause(0.3)
        start(web, "alert('two')")
        spin { window.attachedSheet != nil }
        sheet = try XCTUnwrap(window.attachedSheet)
        let box = try XCTUnwrap(suppression(sheet), "the second dialog offers it")
        XCTAssertEqual(box.title, "Prevent this page from creating additional dialogs")
        box.state = .on
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        pause(0.3)
        XCTAssertEqual(run(web, "confirm('three')") as? Bool, false, "suppressed: Cancel at once")
        XCTAssertNil(window.attachedSheet)
    }

    /// A page that reloads after each dialog cannot start over: its second
    /// dialog offers the box, and once ticked the reloads raise nothing.
    func testAReloadingPageCannotRaiseDialogsForever() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "reload-dialogs"], src: "\(baseURL)/reload-alert")
        defer { arm.destroy(handle) }
        let window = try XCTUnwrap(web.window)
        spin { window.attachedSheet != nil }
        var sheet = try XCTUnwrap(window.attachedSheet)
        XCTAssertNil(suppression(sheet), "the first dialog offers no suppression")
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        spin { window.attachedSheet != nil }
        sheet = try XCTUnwrap(window.attachedSheet, "the page reloaded and asked again")
        let box = try XCTUnwrap(suppression(sheet), "the reloaded page's dialog is its second")
        box.state = .on
        window.endSheet(sheet, returnCode: .alertFirstButtonReturn)
        pause(1.5)
        XCTAssertNil(window.attachedSheet, "suppressed across reloads")
    }

    /// A page that clicks a link to another app and reloads cannot ask again
    /// once a person cancelled.
    func testAReloadingPageCannotAskForAnotherAppAgain() throws {
        guard NSWorkspace.shared.urlForApplication(toOpen: URL(string: "mailto:a@b.test")!) != nil else {
            throw XCTSkip("no app opens mailto: on this Mac")
        }
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "reload-schemes"], src: "\(baseURL)/reload-scheme")
        defer { arm.destroy(handle) }
        let window = try XCTUnwrap(web.window)
        spin { window.attachedSheet != nil }
        let sheet = try XCTUnwrap(window.attachedSheet)
        window.endSheet(sheet, returnCode: .alertSecondButtonReturn)
        pause(2.5)
        XCTAssertNil(window.attachedSheet, "the reloaded page's click asks no more")
    }

    private func suppression(_ sheet: NSWindow) -> NSButton? {
        func find(_ view: NSView) -> NSButton? {
            if let button = view as? NSButton, button.title.hasPrefix("Prevent this page"), !button.isHidden { return button }
            for child in view.subviews { if let found = find(child) { return found } }
            return nil
        }
        return sheet.contentView.flatMap(find)
    }

    /// A sheet up when its frame goes, or when its popup closes, ends as
    /// Cancel: the page's script gets its answer, and no sheet is left.
    func testSheetsEndWithTheirFrameOrPopup() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "endings", "newwindow": true], src: "\(baseURL)/opener")
        loaded(seen, web, "/opener")
        let window = try XCTUnwrap(web.window)
        _ = run(web, "window.open('/b', 'p', 'width=300,height=200'); 0")
        spin { !self.popups().isEmpty && self.popups().first?.isLoading == false && self.popups().first?.url?.path == "/b" }
        let popup = try XCTUnwrap(popups().first)
        let popupWindow = try XCTUnwrap(popup.window)
        var popupAnswer: Any?
        popup.evaluateJavaScript("confirm('Stay?')") { value, error in popupAnswer = value ?? error.map { "\($0)" } ?? "none" }
        spin { popupWindow.attachedSheet != nil }
        XCTAssertNotNil(popupWindow.attachedSheet)
        popupWindow.performClose(nil)
        popupWindow.close()
        spin { popupAnswer != nil }
        XCTAssertEqual(popupAnswer as? Bool, false, "the popup's sheet ended as Cancel")
        var answer: Any?
        web.evaluateJavaScript("confirm('Leave?')") { value, error in answer = value ?? error.map { "\($0)" } ?? "none" }
        spin { window.attachedSheet != nil }
        XCTAssertNotNil(window.attachedSheet)
        arm.destroy(handle)
        spin { answer != nil && window.attachedSheet == nil }
        XCTAssertNil(window.attachedSheet, "the frame's sheet ended with it")
        XCTAssertEqual(answer as? Bool, false)
    }

    /// A sandboxed frame opens another app's scheme only with a token that
    /// allows it (HTML's `allow-top-navigation-to-custom-protocols` note).
    func testASandboxWithoutTheTokensCannotAskForAnotherApp() throws {
        guard NSWorkspace.shared.urlForApplication(toOpen: URL(string: "mailto:a@b.test")!) != nil else {
            throw XCTSkip("no app opens mailto: on this Mac")
        }
        let arm = try arm()
        for (sandbox, asks) in [("allow-scripts", false), ("allow-scripts allow-popups", true)] {
            let seen = Seen()
            let (handle, web) = mount(arm, seen, configuration: [:], src: "\(baseURL)/sandboxed-mail", sandbox: sandbox)
            let window = try XCTUnwrap(web.window)
            spin(until: { window.attachedSheet != nil }, 3)
            XCTAssertEqual(window.attachedSheet != nil, asks, sandbox)
            if let sheet = window.attachedSheet { window.endSheet(sheet, returnCode: .alertSecondButtonReturn) }
            arm.destroy(handle)
        }
    }
    #endif

    /// A download that fails keeps the file already at its place: WebKit
    /// writes beside it, and only a finished file is renamed over it.
    func testAFailedDownloadKeepsTheFileItWouldReplace() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "keep", "agent": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("iframe-windows-keep-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let target = folder.appendingPathComponent("report.txt")
        try Data("original".utf8).write(to: target)
        json(["op": "navigate", "url": "\(baseURL)/broken-report"], arm.command, handle)
        spin { !seen.asked.isEmpty }
        json(["token": seen.asked.last?["token"] ?? 0, "path": target.path, "shown": target.path], arm.answer, handle)
        spin { seen.records.count >= 2 }
        XCTAssertEqual(seen.records.map { $0.record["state"] as? String }, ["progressing", "interrupted"])
        XCTAssertEqual(try? String(contentsOf: target, encoding: .utf8), "original")
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: folder.path), ["report.txt"], "no partial file is left")
    }

    #if os(macOS)
    /// A request held for the agent from a popup ends, as Cancel, when the
    /// popup closes.
    func testAClosedPopupEndsItsHeldRequest() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "popup-held", "newwindow": true, "agent": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        _ = run(web, "window.open('/b', 'p', 'width=300,height=200'); 0")
        spin { self.popups().first?.url?.path == "/b" && self.popups().first?.isLoading == false }
        let popup = try XCTUnwrap(popups().first)
        var answered: Any?
        popup.evaluateJavaScript("confirm('Stay?')") { value, _ in answered = value ?? "none" }
        spin { !seen.asked.isEmpty }
        popup.window?.close()
        spin { answered != nil }
        XCTAssertEqual(answered as? Bool, false)
    }
    #endif

    /// A partition change replaces the web view: a request held for the
    /// agent ends with the old one, as Cancel.
    func testAPartitionChangeEndsAHeldRequest() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "first", "agent": true], src: "\(baseURL)/opener")
        defer { arm.destroy(handle) }
        loaded(seen, web, "/opener")
        var answered: Any?
        web.evaluateJavaScript("confirm('Switch?')") { value, _ in answered = value ?? "none" }
        spin { !seen.asked.isEmpty }
        json(["partition": "second", "agent": true], arm.configure, handle)
        spin { answered != nil }
        XCTAssertEqual(answered as? Bool, false)
    }
}

/// Takes the frame a page's message came from.
private final class FrameProbe: NSObject, WKScriptMessageHandler {
    var frame: WKFrameInfo?
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) { frame = message.frameInfo }
}

/// Pages on a loopback port: `/opener` (links, a form, a file input, and a
/// listener that retitles it with what a popup posts), `/popup` (posts to
/// its opener), `/b`, `/posted` (echoes its method), `/report` (an
/// attachment) and `/fullscreen` (posts its own `fullscreenEnabled` to
/// its parent).
final class WindowPages {
    private var listener: NWListener!

    var port: UInt16 { listener.port!.rawValue }

    init(port: UInt16 = 0) throws { listener = try loopbackListener(port: port) { [weak self] in self?.serve($0) } }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: .main)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, _, _ in
            guard let data, let line = String(decoding: data, as: UTF8.self).split(separator: "\r\n").first else { return connection.cancel() }
            let parts = line.split(separator: " ")
            let method = parts.first.map(String.init) ?? "GET", path = parts.dropFirst().first.map(String.init) ?? "/"
            let response: String
            switch path {
            case "/opener":
                response = Self.page("Opener", """
                <a id=blank target=_blank href=/b>new tab</a> <a id=plain href=/b>plain</a> <a id=mail target=_blank href="mailto:a@b.test">mail</a>
                <form id=post method=post action=/posted target=_blank><input name=q value=1></form> <input id=file type=file>
                <script>addEventListener('message', e => { document.title = 'got ' + e.data })</script>
                """)
            case "/popup": response = Self.page("Popup", "<script>opener && opener.postMessage('signed-in', '*')</script>")
            case "/b": response = Self.page("Page B", "B")
            case "/reload-alert": response = Self.page("Reload alert", "<script>alert('again'); location.reload()</script>")
            case "/reload-scheme":
                response = Self.page("Reload scheme", "<a id=mail href=\"mailto:a@b.test\">mail</a><script>setTimeout(() => { document.getElementById('mail').click(); setTimeout(() => location.reload(), 400) }, 200)</script>")
            case "/broken-report":
                response = "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"report.txt\"\r\nContent-Length: 4000\r\nConnection: close\r\n\r\npartial"
            case "/timer-scheme":
                response = Self.page("Timer", "<a id=mail href=\"mailto:a@b.test\">mail</a><script>setInterval(() => { location.href = 'mailto:loop@b.test' }, 200)</script>")
            case "/sandboxed-mail":
                response = Self.page("Sandboxed", "<a id=mail href=\"mailto:a@b.test\">mail</a><script>setTimeout(() => document.getElementById('mail').click(), 300)</script>")
            case "/posted": response = Self.page("Posted", method)
            case "/fullscreen":
                response = Self.page("Fullscreen", "<script>parent.postMessage('fullscreen ' + document.fullscreenEnabled, '*')</script>")
            case "/report":
                let body = "report body"
                response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"report.txt\"\r\nContent-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n\(body)"
            default: response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            }
            connection.send(content: Data(response.utf8), completion: .contentProcessed { _ in connection.cancel() })
        }
    }

    private static func page(_ title: String, _ body: String) -> String {
        let html = "<!doctype html><meta charset=utf-8><title>\(title)</title><body>\(body)</body>"
        return "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: \(html.utf8.count)\r\nConnection: close\r\n\r\n\(html)"
    }
}
#endif
