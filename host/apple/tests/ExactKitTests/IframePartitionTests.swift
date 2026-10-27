import Network
import WebKit
import XCTest
#if os(macOS)
import AppKit
#else
import UIKit
#endif

#if os(macOS) || os(iOS)
/// A partitioned iframe through the web arm's C ABI (LLP 1020.002 stage 1):
/// its own data store, `FrameNavigation` records in the D2 sequence, the D3
/// commands, the D1 refusals and crash recovery, against pages a loopback
/// server in the test serves.
final class IframePartitionTests: XCTestCase {
    private typealias EventFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ReplyFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias CreateFn = @convention(c) (UInt32, UnsafeMutableRawPointer?, EventFn?, ReplyFn?) -> UnsafeMutableRawPointer?
    private typealias ViewFn = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    private typealias SetFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32, UInt32) -> Void
    private typealias PostFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias DoneFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
    private typealias ClearFn = @convention(c) (UnsafePointer<UInt8>?, UInt32, UnsafeMutableRawPointer?, DoneFn?) -> Void

    /// What one frame reported: `FrameNavigation` records (kind 4) and loads (kind 0).
    private final class Seen {
        var records: [(event: String, record: [String: Any])] = []
        var loads = 0
        var cleared: String?
    }

    private struct Arm {
        let create: CreateFn, view: ViewFn, setSrc: SetFn, setSandbox: SetFn, setDocument: SetFn, configure: PostFn, command: PostFn
        let post: PostFn, destroy: DestroyFn, clear: ClearFn, endSession: @convention(c) (UnsafePointer<UInt8>?, UInt32) -> Void
    }

    private func arm() throws -> Arm {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        let library = try XCTUnwrap(dlopen(path, RTLD_NOW | RTLD_LOCAL), String(cString: dlerror()))
        func symbol<T>(_ name: String, _: T.Type) throws -> T { unsafeBitCast(try XCTUnwrap(dlsym(library, name), "missing \(name)"), to: T.self) }
        return Arm(create: try symbol("exact_web_create", CreateFn.self), view: try symbol("exact_web_platform_view", ViewFn.self),
                   setSrc: try symbol("exact_web_set_src", SetFn.self), setSandbox: try symbol("exact_web_set_sandbox", SetFn.self),
                   setDocument: try symbol("exact_web_set_document", SetFn.self), configure: try symbol("exact_web_configure", PostFn.self),
                   command: try symbol("exact_web_command", PostFn.self), post: try symbol("exact_web_post_message", PostFn.self),
                   destroy: try symbol("exact_web_destroy", DestroyFn.self), clear: try symbol("exact_web_clear_site_data", ClearFn.self),
                   endSession: try symbol("exact_web_end_session", (@convention(c) (UnsafePointer<UInt8>?, UInt32) -> Void).self))
    }

    private static let event: EventFn = { context, _, kind, bytes, length in
        let seen = Unmanaged<Seen>.fromOpaque(context!).takeUnretainedValue()
        let data = Data(bytes: bytes ?? UnsafePointer(bitPattern: 1)!, count: Int(length))
        if kind == 0 { seen.loads += 1 }
        guard kind == 4, let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let name = value["event"] as? String, let record = value["record"] as? [String: Any] else { return }
        seen.records.append((name, record))
    }
    private static let reply: ReplyFn = { _, _, _, _, _, _ in }

    private func send(_ text: String?, _ setter: SetFn, _ handle: UnsafeMutableRawPointer) {
        guard let text else { return setter(handle, nil, 0, 0) }
        Array(text.utf8).withUnsafeBufferPointer { setter(handle, $0.baseAddress, UInt32($0.count), 1) }
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

    /// Until nothing more arrives for 0.4 s: a title WebKit reports after
    /// `didFinish` is a `currententrychange` of its own.
    private func settle(_ seen: Seen) {
        var count = -1
        while count != seen.records.count { count = seen.records.count; pause(0.4) }
    }

    /// The step's events after the frame settled from the last one.
    private func step(_ seen: Seen, _ act: () -> Void) {
        settle(seen)
        seen.records = []
        act()
    }

    /// A new document's navigation, once it settled: `expected`, then only
    /// the `currententrychange` of a title or icon WebKit reports after
    /// `didFinish`, which can arrive in the same run-loop pass as `navigatesuccess`.
    private func assertLoaded(_ seen: Seen, _ expected: [String], file: StaticString = #filePath, line: UInt = #line) {
        spin { self.events(seen).contains("navigatesuccess") }
        settle(seen)
        XCTAssertEqual(Array(events(seen).prefix(expected.count)), expected, "\(seen.records)", file: file, line: line)
        XCTAssert(events(seen).dropFirst(expected.count).allSatisfy { $0 == "currententrychange" }, "only a late title or icon follows: \(events(seen))", file: file, line: line)
    }

    /// A frame of the arm, in a window so WebKit lays it out.
    private func mount(_ arm: Arm, _ seen: Seen, configuration: [String: Any], src: String?, sandbox: String? = nil) -> (UnsafeMutableRawPointer, WKWebView) {
        let handle = arm.create(7, Unmanaged.passUnretained(seen).toOpaque(), Self.event, Self.reply)!
        json(configuration, arm.configure, handle)
        let web = Unmanaged<WKWebView>.fromOpaque(arm.view(handle)!).takeUnretainedValue()
        web.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        #if os(macOS)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView!.addSubview(web)
        window.orderFrontRegardless()
        windows.append(window)
        #else
        let window = hostedWindowScene().map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        let host = UIViewController()
        window.rootViewController = host
        host.view.addSubview(web)
        window.isHidden = false
        windows.append(window)
        #endif
        send(sandbox, arm.setSandbox, handle)
        send(src, arm.setSrc, handle)
        return (handle, web)
    }

    #if os(macOS)
    private var windows: [NSWindow] = []
    override func tearDown() { windows.forEach { $0.close() }; windows = [] }
    #else
    private var windows: [UIWindow] = []
    override func tearDown() { windows.forEach { $0.isHidden = true }; windows = [] }
    #endif

    nonisolated(unsafe) private static var server: FramePages?
    private var baseURL: String { "http://127.0.0.1:\(Self.server!.port)" }
    override func setUpWithError() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        if Self.server == nil { Self.server = try FramePages() }
    }

    private func events(_ seen: Seen) -> [String] { seen.records.map(\.event) }

    func testNavigationEventsCommandsAndTheirRecords() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "memory-a"], src: "\(baseURL)/a")
        defer { arm.destroy(handle) }
        assertLoaded(seen, ["navigate", "currententrychange", "navigatesuccess"])
        let first = try XCTUnwrap(seen.records.last?.record)
        XCTAssertEqual(first["url"] as? String, "\(baseURL)/a")
        XCTAssertEqual(first["title"] as? String, "Page A")
        XCTAssertEqual(first["loading"] as? Bool, false)
        XCTAssertEqual(first["partitioned"] as? Bool, true)
        XCTAssertEqual(first["icons"] as? [String], ["\(baseURL)/a.png", "\(baseURL)/favicon.ico"])
        XCTAssertEqual(seen.records[0].record["destination"] as? String, "\(baseURL)/a")
        XCTAssertEqual(seen.records[0].record["loading"] as? Bool, true)
        XCTAssertEqual(Set(seen.records.map { $0.record["navigationId"] as? Int }).count, 1, "one navigation's events share an id")
        XCTAssertEqual(seen.loads, 1, "`load` still fires for every document")
        // The partition's own storage: the page cannot see the app's web view's, nor the reverse.
        XCTAssertNotEqual(web.configuration.websiteDataStore, WKWebsiteDataStore.default())

        // A server redirect: `navigate` again with the same id and the new destination.
        step(seen) { json(["op": "navigate", "url": "\(baseURL)/redirect"], arm.command, handle) }
        assertLoaded(seen, ["navigate", "navigate", "currententrychange", "navigatesuccess"])
        guard seen.records.count >= 2 else { return }
        XCTAssertEqual(seen.records[1].record["destination"] as? String, "\(baseURL)/b")
        XCTAssertEqual(seen.records[1].record["navigationId"] as? Int, seen.records[0].record["navigationId"] as? Int)
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "\(baseURL)/b")
        XCTAssertEqual(seen.records.last?.record["title"] as? String, "Page B")
        XCTAssertEqual(seen.records.last?.record["canGoBack"] as? Bool, true)

        // A same-document `pushState`: the three events together, a push.
        step(seen) { web.evaluateJavaScript("history.pushState(null, '', '/b#pushed')") { _, _ in } }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(events(seen), ["navigate", "currententrychange", "navigatesuccess"], "\(seen.records)")
        XCTAssertEqual(seen.records.last?.record["navigationType"] as? String, "push")
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "\(baseURL)/b#pushed")

        // A title change with no navigation: `currententrychange` alone.
        step(seen) { web.evaluateJavaScript("document.title = 'Retitled'") { _, _ in } }
        spin { !seen.records.isEmpty }
        XCTAssertEqual(events(seen), ["currententrychange"])
        XCTAssertEqual(seen.records.last?.record["title"] as? String, "Retitled")

        // A same-document traversal starts no provisional navigation; the next
        // navigation is a push again, not the traversal's type.
        step(seen) { json(["op": "traverseHistory", "delta": -1], arm.command, handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.last?.record["navigationType"] as? String, "traverse", "\(seen.records)")
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "\(baseURL)/b")
        step(seen) { web.evaluateJavaScript("location.assign('/a')") { _, _ in } }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.first?.record["navigationType"] as? String, "push", "\(seen.records)")

        // A traversal to an entry of the same URL: WebKit says nothing of a
        // `pushState` that keeps the URL, nor of a traversal back to it, so
        // the command's type must not outlive it; a fragment change after it
        // and a link are pushes.
        step(seen) { web.evaluateJavaScript("history.pushState({again: true}, '', location.href)") { _, _ in } }
        step(seen) { json(["op": "traverseHistory", "delta": -1], arm.command, handle) }
        settle(seen)
        step(seen) { web.evaluateJavaScript("location.hash = 'fragment'") { _, _ in } }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.first?.record["navigationType"] as? String, "push", "\(seen.records)")
        step(seen) { web.evaluateJavaScript("location.assign('/b')") { _, _ in } }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.first?.record["navigationType"] as? String, "push", "\(seen.records)")

        // Back to the first page: a traversal.
        step(seen) { json(["op": "traverseHistory", "delta": -4], arm.command, handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.last?.record["navigationType"] as? String, "traverse")
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "\(baseURL)/a")

        // No such entry: InvalidStateError, nothing else.
        step(seen) { json(["op": "traverseHistory", "delta": -5], arm.command, handle) }
        spin { !seen.records.isEmpty }
        XCTAssertEqual(events(seen), ["navigateerror"])
        XCTAssertEqual(seen.records.last?.record["error"] as? String, "InvalidStateError")

        // `reload(id, ignoreCache=true)`: a reload navigation.
        step(seen) { json(["op": "reload", "ignoreCache": true], arm.command, handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.first?.record["navigationType"] as? String, "reload")

        // `stopLoading` of a page that never answers: AbortError, and nothing follows.
        step(seen) { json(["op": "navigate", "url": "\(baseURL)/slow"], arm.command, handle) }
        spin { self.events(seen).contains("navigate") }
        json(["op": "stopLoading"], arm.command, handle)
        pause(1)
        XCTAssertEqual(events(seen), ["navigate", "navigateerror"], "\(seen.records)")
        XCTAssertEqual(seen.records.last?.record["error"] as? String, "AbortError")
        XCTAssertEqual(seen.records.last?.record["loading"] as? Bool, false)

        // A superseded navigation ends with AbortError before the new one's `navigate`.
        step(seen) { json(["op": "navigate", "url": "\(baseURL)/slow"], arm.command, handle) }
        spin { self.events(seen).contains("navigate") }
        json(["op": "navigate", "url": "\(baseURL)/a"], arm.command, handle)
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(Array(events(seen).prefix(3)), ["navigate", "navigateerror", "navigate"], "\(seen.records)")
        XCTAssertEqual(seen.records.dropFirst().first?.record["error"] as? String, "AbortError")

        // `javascript:` from an address field is refused by name, not run.
        step(seen) { json(["op": "navigate", "url": "javascript:document.title='ran'"], arm.command, handle) }
        spin { !seen.records.isEmpty }
        XCTAssertEqual(events(seen), ["navigateerror"])
        XCTAssertEqual(seen.records.last?.record["error"] as? String, "NotSupportedError")
        XCTAssertEqual(web.title, "Page A")
    }

    func testPartitionsKeepTheirOwnStorageAndClear() throws {
        let arm = try arm(), one = Seen(), two = Seen(), again = Seen()
        let (a, webA) = mount(arm, one, configuration: ["partition": "memory-one"], src: "\(baseURL)/a")
        defer { arm.destroy(a) }
        let (b, webB) = mount(arm, two, configuration: ["partition": "memory-two"], src: "\(baseURL)/a")
        defer { arm.destroy(b) }
        spin { self.events(one).contains("navigatesuccess") && self.events(two).contains("navigatesuccess") }
        var stored: Any?
        webA.evaluateJavaScript("localStorage.setItem('k', 'one'); document.cookie = 'c=one'; 1") { _, _ in }
        pause(0.3)
        webB.evaluateJavaScript("localStorage.getItem('k')") { value, _ in stored = value ?? NSNull() }
        spin { stored != nil }
        XCTAssert(stored is NSNull, "another partition does not see the first's storage: \(String(describing: stored))")
        // The same name shares the store.
        let (c, webC) = mount(arm, again, configuration: ["partition": "memory-one"], src: "\(baseURL)/a")
        defer { arm.destroy(c) }
        spin { self.events(again).contains("navigatesuccess") }
        stored = nil
        webC.evaluateJavaScript("localStorage.getItem('k')") { value, _ in stored = value ?? NSNull() }
        spin { stored != nil }
        XCTAssertEqual(stored as? String, "one")
        // `clearSiteData`: done in WebKit's completion handler; an unnamed partition is NotFoundError.
        let seen = Seen()
        let done: DoneFn = { context, bytes, length in
            Unmanaged<Seen>.fromOpaque(context!).takeUnretainedValue().cleared = String(decoding: UnsafeBufferPointer(start: bytes, count: Int(length)), as: UTF8.self)
        }
        let clear = { (request: [String: Any]) in
            seen.cleared = nil
            let data = try! JSONSerialization.data(withJSONObject: request)
            data.withUnsafeBytes { arm.clear($0.bindMemory(to: UInt8.self).baseAddress, UInt32(data.count), Unmanaged.passUnretained(seen).toOpaque(), done) }
            self.spin { seen.cleared != nil }
        }
        clear(["partition": "memory-never", "types": ["cookies"]])
        XCTAssert(seen.cleared?.hasPrefix("NotFoundError") == true, seen.cleared ?? "")
        clear(["partition": "memory-one", "types": ["bogus"]])
        XCTAssert(seen.cleared?.hasPrefix("SyntaxError") == true, seen.cleared ?? "")
        clear(["partition": "memory-one", "types": ["cookies", "storage"]])
        XCTAssertEqual(seen.cleared, "")
        // A page open in the partition may keep data (D4); a fresh document reads the cleared store.
        let fresh = Seen()
        let (d, webD) = mount(arm, fresh, configuration: ["partition": "memory-one"], src: "\(baseURL)/b")
        defer { arm.destroy(d) }
        spin { self.events(fresh).contains("navigatesuccess") }
        stored = nil
        webD.evaluateJavaScript("[localStorage.getItem('k'), document.cookie].join('|')") { value, _ in stored = value }
        spin { stored != nil }
        XCTAssertEqual(stored as? String, "|")
    }

    func testRefusedSourcesShowBlankAndSayWhy() throws {
        let arm = try arm()
        for (configuration, src, sandbox, error) in [
            (["partition": "memory-r"], "app:/data/page.html", nil, "NotSupportedError"),
            (["partition": "memory-r"], "/deck/index.html", nil, "NotSupportedError"),
            (["partition": "memory-r"], "\(baseURL)/a", "allow-scripts", "NotSupportedError"),
            (["partition": "bad name"], "\(baseURL)/a", nil, "SyntaxError"),
        ] as [([String: Any], String, String?, String)] {
            let seen = Seen()
            let (handle, web) = mount(arm, seen, configuration: configuration, src: src, sandbox: sandbox)
            spin { !seen.records.isEmpty }
            pause(0.5)
            XCTAssertEqual(events(seen), ["navigateerror"], "\(src) \(sandbox ?? "")")
            XCTAssertEqual(seen.records.first?.record["error"] as? String, error)
            XCTAssertEqual(web.url?.absoluteString ?? "about:blank", "about:blank")
            XCTAssertEqual(seen.loads, 0)
            arm.destroy(handle)
        }
    }

    /// A refusal while a navigation is in flight: that navigation ends as
    /// itself, and the `about:blank` shown in its place reports nothing.
    func testARefusalMidNavigationReportsNothingOfTheBlankPage() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "memory-mid"], src: "\(baseURL)/slow")
        defer { arm.destroy(handle) }
        spin { self.events(seen).contains("navigate") }
        send("allow-scripts", arm.setSandbox, handle)
        spin { self.events(seen).filter { $0 == "navigateerror" }.count >= 2 }
        pause(1)
        XCTAssertEqual(events(seen), ["navigate", "navigateerror", "navigateerror"], "\(seen.records)")
        XCTAssertEqual(seen.records.dropFirst().first?.record["error"] as? String, "AbortError")
        XCTAssertEqual(seen.records.last?.record["error"] as? String, "NotSupportedError")
        XCTAssertEqual(seen.loads, 0)
        XCTAssertEqual(web.url?.absoluteString ?? "about:blank", "about:blank")
    }

    func testAnUnpartitionedFrameRefusesTheCommands() throws {
        let arm = try arm(), seen = Seen()
        let (handle, _) = mount(arm, seen, configuration: [:], src: "\(baseURL)/a")
        defer { arm.destroy(handle) }
        json(["op": "reload"], arm.command, handle)
        spin { !seen.records.isEmpty }
        XCTAssertEqual(events(seen), ["navigateerror"])
        XCTAssertEqual(seen.records.first?.record["error"] as? String, "NotSupportedError")
    }

    /// Under the agent a `persist:` partition is in memory, so a drive never
    /// touches a person's saved logins; outside it, it persists, by app (D1).
    func testPersistentPartitionsAreTheAppsAndInMemoryUnderTheAgent() throws {
        let arm = try arm(), seen = Seen()
        func store(_ configuration: [String: Any]) -> WKWebsiteDataStore {
            let (handle, web) = mount(arm, seen, configuration: configuration, src: nil)
            defer { arm.destroy(handle) }
            return web.configuration.websiteDataStore
        }
        XCTAssertFalse(store(["partition": "persist:drive", "agent": true]).isPersistent)
        let one = store(["partition": "persist:kept", "app": "dev.exact.test.one"])
        XCTAssertTrue(one.isPersistent)
        XCTAssertTrue(one === store(["partition": "persist:kept", "app": "dev.exact.test.one"]))
        let two = store(["partition": "persist:kept", "app": "dev.exact.test.two"])
        XCTAssertFalse(one === two, "two apps never share a partition's files")
        if #available(macOS 14.0, iOS 17.0, *) { XCTAssertNotEqual(one.identifier, two.identifier) }
        // An in-memory partition is its session's, and goes when the session ends.
        let first = store(["partition": "tab", "session": "s1"])
        XCTAssertTrue(first === store(["partition": "tab", "session": "s1"]))
        XCTAssertFalse(first === store(["partition": "tab", "session": "s2"]))
        Array("{\"session\":\"s1\"}".utf8).withUnsafeBufferPointer { arm.endSession($0.baseAddress, UInt32($0.count)) }
        XCTAssertFalse(first === store(["partition": "tab", "session": "s1"]), "the ended session's store was let go")
        // The test's own persistent stores go; a store still in use may refuse, which leaves only its folder.
        if #available(macOS 14.0, iOS 17.0, *) {
            for id in [one.identifier, two.identifier].compactMap({ $0 }) {
                var done = false
                WKWebsiteDataStore.remove(forIdentifier: id) { _ in done = true }
                spin { done }
            }
        }
    }

    /// A partitioned frame changing its partition gets a new web view, in the new store.
    func testAChangedPartitionReplacesTheWebView() throws {
        let arm = try arm(), seen = Seen()
        let (handle, first) = mount(arm, seen, configuration: ["partition": "memory-x"], src: "\(baseURL)/a")
        defer { arm.destroy(handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        seen.records = []
        json(["partition": "memory-y"], arm.configure, handle)
        let second = Unmanaged<WKWebView>.fromOpaque(arm.view(handle)!).takeUnretainedValue()
        XCTAssertFalse(first === second)
        second.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        spin { self.events(seen).contains("navigatesuccess") }
        XCTAssertEqual(seen.records.last?.record["url"] as? String, "\(baseURL)/a")
        XCTAssertNotEqual(first.configuration.websiteDataStore, second.configuration.websiteDataStore)
    }

    /// Three recoveries after 250, 500 and 1,000 ms; the fourth death in 30 s ends it (D2, Astra r3).
    func testAFourthCrashIn30SecondsStopsRecovering() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "memory-crash"], src: "\(baseURL)/a")
        defer { arm.destroy(handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        let pid = { (web.value(forKey: "_webProcessIdentifier") as? NSNumber)?.int32Value ?? 0 }
        for death in 1...4 {
            let before = seen.records.count
            spin { pid() > 0 }
            XCTAssertGreaterThan(pid(), 0)
            kill(pid(), SIGKILL)
            spin { seen.records.dropFirst(before).contains { $0.event == "navigateerror" } }
            let error = try XCTUnwrap(seen.records.dropFirst(before).first { $0.event == "navigateerror" }?.record)
            XCTAssertEqual(error["error"] as? String, "crashed")
            XCTAssertEqual(error["recovering"] as? Bool, death < 4, "death \(death)")
            if death < 4 {
                let started = Date()
                spin { seen.records.dropFirst(before).contains { $0.event == "navigatesuccess" } }
                XCTAssert(seen.records.dropFirst(before).contains { $0.event == "navigatesuccess" && $0.record["navigationType"] as? String == "reload" }, "recovery \(death): \(seen.records.dropFirst(before).map { "\($0.event) \($0.record["navigationType"] ?? "") \($0.record["error"] ?? "")" })")
                XCTAssertGreaterThanOrEqual(Date().timeIntervalSince(started), [0.25, 0.5, 1.0][death - 1] * 0.8)
            }
        }
        let after = seen.records.count
        pause(1.5)
        XCTAssertFalse(seen.records.dropFirst(after).contains { $0.event == "navigate" }, "no attempt after the fourth death until reload")
        json(["op": "reload"], arm.command, handle)
        spin { seen.records.dropFirst(after).contains { $0.event == "navigatesuccess" } }
        XCTAssert(seen.records.dropFirst(after).contains { $0.event == "navigatesuccess" })
    }

    /// Stage 1's first measurement (D1, "Hidden tabs"): a partitioned web
    /// view under a hidden node view (`display: none`) still paints on
    /// request, with no private occlusion selector, so D7's capture of a
    /// hidden tab has its pixels. (The web view's own `isHidden` or a zero
    /// `alpha` paints nothing on iOS; the arm sets neither.)
    func testAHiddenFramePaintsWhenAsked() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "memory-hidden"], src: "\(baseURL)/b")
        defer { arm.destroy(handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        #if os(macOS)
        let node = NSView(frame: web.frame)
        #else
        let node = UIView(frame: web.frame)
        #endif
        web.superview?.addSubview(node)
        node.addSubview(web)
        for (hidden, colour) in [(false, "rgb(0, 255, 0)"), (true, "rgb(255, 0, 0)"), (true, "rgb(0, 0, 255)")] {
            node.isHidden = hidden
            var painted: [UInt8]?
            web.evaluateJavaScript("document.body.style.background = '\(colour)'; 1") { _, _ in
                let configuration = WKSnapshotConfiguration()
                configuration.afterScreenUpdates = true
                web.takeSnapshot(with: configuration) { image, _ in painted = image.flatMap(Self.centre) ?? [] }
            }
            spin { painted != nil }
            let pixel = try XCTUnwrap(painted)
            XCTAssertEqual(pixel.count, 3, "a snapshot of the hidden web view")
            #if os(iOS)
            // Outside an app's scene (a bare xctest process) WebKit draws nothing at all.
            if !hidden, pixel == [0, 0, 0] { throw missing("scene", "WebKit paints a snapshot only in an app's window scene") }
            #endif
            let want = colour.dropFirst(4).dropLast().split(separator: ",").map { Int($0.trimmingCharacters(in: .whitespaces))! }
            XCTAssert(zip(want, pixel).allSatisfy { abs($0 - Int($1)) < 60 }, "hidden \(hidden), \(colour): \(pixel)")
        }
    }

    #if os(macOS)
    private static func centre(_ image: NSImage) -> [UInt8]? {
        guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil),
              let colour = NSBitmapImageRep(cgImage: cg).colorAt(x: cg.width / 2, y: cg.height / 2)?.usingColorSpace(.sRGB) else { return nil }
        return [colour.redComponent, colour.greenComponent, colour.blueComponent].map { UInt8(max(0, min(255, $0 * 255))) }
    }
    #else
    private static func centre(_ image: UIImage) -> [UInt8]? {
        guard let cg = image.cgImage, let data = cg.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return nil }
        let offset = (cg.height / 2) * cg.bytesPerRow + (cg.width / 2) * (cg.bitsPerPixel / 8)
        let order = cg.bitmapInfo.contains(.byteOrder32Little)
        return order ? [bytes[offset + 2], bytes[offset + 1], bytes[offset]] : [bytes[offset], bytes[offset + 1], bytes[offset + 2]]
    }
    #endif

    /// The postMessage bridge is absent: nothing is delivered, and the page's
    /// own `window.postMessage` reaches its own listeners (D1).
    func testAPartitionedFrameHasNoBridge() throws {
        let arm = try arm(), seen = Seen()
        let (handle, web) = mount(arm, seen, configuration: ["partition": "memory-p"], src: "\(baseURL)/a")
        defer { arm.destroy(handle) }
        spin { self.events(seen).contains("navigatesuccess") }
        var heard: Any?
        web.callAsyncJavaScript("return await new Promise(r => { addEventListener('message', e => r(e.data)); window.postMessage('self', '*'); })", in: nil, in: .page) { result in
            heard = (try? result.get()) ?? "failed"
        }
        spin { heard != nil }
        XCTAssertEqual(heard as? String, "self")
        let scripts = web.configuration.userContentController.userScripts
        XCTAssertTrue(scripts.allSatisfy { !$0.source.contains("exactFrame") }, "no bridge script in a partitioned frame")
        XCTAssertNil(web.configuration.urlSchemeHandler(forURLScheme: "exact-app"))
    }
}

/// A ready listener on 127.0.0.1 that hands each connection to `serve`.
/// Port 0 lets the system choose.
func loopbackListener(port: UInt16 = 0, _ serve: @escaping (NWConnection) -> Void) throws -> NWListener {
    let parameters = NWParameters.tcp
    parameters.allowLocalEndpointReuse = true
    parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: port)!)
    let listener = try NWListener(using: parameters)
    listener.newConnectionHandler = serve
    var state: NWListener.State = .setup
    listener.stateUpdateHandler = { state = $0 }
    listener.start(queue: .main)
    let deadline = Date(timeIntervalSinceNow: 5)
    while Date() < deadline { if case .ready = state { return listener }; RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
    throw NSError(domain: "loopbackListener", code: 1, userInfo: [NSLocalizedDescriptionKey: "no listener on 127.0.0.1:\(port == 0 ? "any port" : "\(port)"): \(state)"])
}

/// Fixed pages on a loopback port: `/a` (a title and an icon), `/b`,
/// `/redirect` (302 to `/b`) and `/slow`, which never answers.
final class FramePages {
    private var listener: NWListener!
    private var held: [NWConnection] = []

    var port: UInt16 { listener.port!.rawValue }

    init(port: UInt16 = 0) throws { listener = try loopbackListener(port: port) { [weak self] in self?.serve($0) } }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: .main)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, _, _ in
            guard let self, let data, let line = String(decoding: data, as: UTF8.self).split(separator: "\r\n").first else { return connection.cancel() }
            let path = line.split(separator: " ").dropFirst().first.map(String.init) ?? "/"
            let response: String
            switch path {
            case "/a": response = Self.page("Page A", head: "<link rel=icon href=/a.png>")
            case "/b": response = Self.page("Page B")
            case "/redirect": response = "HTTP/1.1 302 Found\r\nLocation: /b\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            case "/slow": self.held.append(connection); return
            default: response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            }
            connection.send(content: Data(response.utf8), completion: .contentProcessed { _ in connection.cancel() })
        }
    }

    private static func page(_ title: String, head: String = "") -> String {
        let body = "<!doctype html><meta charset=utf-8><title>\(title)</title>\(head)<body>\(title)</body>"
        return "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n\(body)"
    }
}
#endif
