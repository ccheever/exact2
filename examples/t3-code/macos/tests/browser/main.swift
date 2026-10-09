import AppKit
import Network
import WebKit
import XCTest

// browser-surface part 1: the Browser tab's page (T3BrowserSession.swift, T3BrowserSessions.swift,
// T3BrowserView.swift, T3BrowserFavicon.swift) against a loopback fixture server: page states, history,
// refresh, a load failure, crash recovery, the favicon, pop-ups, refused schemes, the permission set, the
// native user agent, Web Inspector, the profile stores and a tab's lifetime across views. Ported reference
// tests (T3 Code 1e2ecbd975, MIT, see LICENSE-T3), under their own names: webviewCrashRecovery.test.ts (2) and
// Manager.test.ts `previewWindowOpenAction` (3). `T3_BROWSER_TEST_DIR` receives page PNGs.
final class Fixture {
    private let listener: NWListener
    private(set) var paths: [String] = []
    /// path → (status, content type, body, delay in seconds)
    var routes: [String: (Int, String, Data, Double)] = [:]
    /// path → Location (a 302)
    var redirects: [String: String] = [:]
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "browser-fixture")

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }
    var base: String { "http://127.0.0.1:\(port)" }
    var seen: [String] { queue.sync { paths } }
    func page(_ path: String, _ html: String, delay: Double = 0) { routes[path] = (200, "text/html", Data(html.utf8), delay) }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return connection.cancel() }
            let path = String(head.split(separator: " ").dropFirst().first ?? "")
            self.paths.append(path)
            let route = String(path.split(separator: "?").first ?? "")
            if let location = self.redirects[route] {
                let response = Data("HTTP/1.1 302 Found\r\nLocation: \(location)\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".utf8)
                return connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
            }
            let (status, type, body, delay) = self.routes[route] ?? (404, "text/html", Data("<title>Not found</title>missing".utf8), 0)
            var response = Data("HTTP/1.1 \(status) \(status == 200 ? "OK" : "Not Found")\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".utf8)
            response.append(body)
            self.queue.asyncAfter(deadline: .now() + delay) { connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() }) }
        }
    }
    deinit { listener.cancel() }
}

/// A 16×16 red PNG (the fixture's favicon).
let redPNG: Data = {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 16, pixelsHigh: 16, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSColor.red.setFill(); NSRect(x: 0, y: 0, width: 16, height: 16).fill()
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}()

final class BrowserSessionTests: XCTestCase {
    private var window: NSWindow!
    private var fixture: Fixture!
    private var sessions: T3BrowserSessions!
    private var statuses = 0

    override func setUpWithError() throws {
        fixture = try Fixture()
        sessions = T3BrowserSessions(agent: true, changed: { [weak self] _ in self?.statuses += 1 })
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 560), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
    }
    override func tearDown() { sessions.sync([]); window.orderOut(nil) }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private func evaluate(_ web: WKWebView, _ script: String) -> String {
        var value: String?
        web.evaluateJavaScript(script) { result, error in value = error.map { "error: \($0.localizedDescription)" } ?? (result as? String) ?? "\(result ?? "nil")" }
        spin(until: { value != nil }, timeout: 5)
        return value ?? "timeout"
    }
    private func mounted(_ id: String, url: String = "") -> (T3BrowserView, T3BrowserSession) {
        let view = T3BrowserView(props: ["tab": id, "url": url, "profile": "default", "environment": "env-1"], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), sessions: sessions)
        view.host.frame = window.contentView!.bounds
        window.contentView!.addSubview(view.host)
        return (view, sessions.sessions[id]!)
    }
    private func kind(_ session: T3BrowserSession) -> String { session.report["kind"] as? String ?? "" }
    private func save(_ web: WKWebView, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"] else { return }
        var done = false
        web.takeSnapshot(with: nil) { image, _ in
            if let image, let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
                try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
            }
            done = true
        }
        spin(until: { done }, timeout: 5)
    }

    func testAnIdleTabLoadsAPageAndReportsItsStates() throws {
        fixture.page("/slow", "<!doctype html><title>Slow page</title><link rel=icon href=/icon.png><h1>slow</h1>", delay: 1.0)
        fixture.routes["/icon.png"] = (200, "image/png", redPNG, 0)
        let (view, session) = mounted("tab-states")
        XCTAssertEqual(kind(session), "Idle", "a new tab is Idle (about:blank)")
        XCTAssertTrue(sessions.navigate(id: "tab-states", url: "\(fixture.base)/slow", profile: "default", environment: "env-1"))
        XCTAssertEqual(kind(session), "Loading", "Loading from the moment the address is asked for")
        XCTAssertEqual(session.report["url"] as? String, "\(fixture.base)/slow")
        spin(until: { self.kind(session) == "Success" })
        XCTAssertEqual(kind(session), "Success")
        XCTAssertEqual(session.report["title"] as? String, "Slow page")
        XCTAssertTrue(statuses > 0, "each change announces t3.status")
        spin(until: { session.report["favicon"] != nil })
        let favicon = try XCTUnwrap(session.report["favicon"] as? [String: Any])
        XCTAssertTrue((favicon["dataUrl"] as? String ?? "").hasPrefix("data:image/png;base64,"))
        XCTAssertEqual(favicon["pageUrl"] as? String, fixture.base, "the icon belongs to the page origin")
        XCTAssertLessThanOrEqual((favicon["dataUrl"] as? String ?? "").count, 8192)
        save(session.web, "browser-loaded.png")
        view.destroy()
    }

    func testBackForwardRefreshAndHardReload() throws {
        fixture.page("/a", "<!doctype html><title>A</title><a id=next href=/b>b</a>")
        fixture.page("/b", "<!doctype html><title>B</title>")
        let (view, session) = mounted("tab-history", url: "\(fixture.base)/a")
        spin(until: { self.kind(session) == "Success" && session.report["title"] as? String == "A" })
        _ = evaluate(session.web, "document.getElementById('next').click(); 'ok'")
        spin(until: { session.report["title"] as? String == "B" && self.kind(session) == "Success" })
        XCTAssertEqual(session.report["canGoBack"] as? Bool, true)
        XCTAssertEqual(session.report["canGoForward"] as? Bool, false)
        XCTAssertTrue(sessions.command(id: "tab-history", name: "back"))
        spin(until: { session.report["title"] as? String == "A" && self.kind(session) == "Success" })
        XCTAssertEqual(session.report["canGoForward"] as? Bool, true)
        XCTAssertTrue(sessions.command(id: "tab-history", name: "forward"))
        spin(until: { session.report["title"] as? String == "B" && self.kind(session) == "Success" })
        let before = fixture.seen.filter { $0 == "/b" }.count
        XCTAssertTrue(sessions.command(id: "tab-history", name: "refresh"))
        spin(until: { self.fixture.seen.filter { $0 == "/b" }.count > before && self.kind(session) == "Success" })
        XCTAssertTrue(sessions.command(id: "tab-history", name: "hard-reload"))
        spin(until: { self.fixture.seen.filter { $0 == "/b" }.count > before + 1 && self.kind(session) == "Success" })
        XCTAssertGreaterThan(fixture.seen.filter { $0 == "/b" }.count, before + 1, "refresh and hard reload both load the page again")
        XCTAssertFalse(sessions.command(id: "missing", name: "back"), "a tab without a page answers nothing")
        view.destroy()
    }

    func testRefreshWhileLoadingAsksForThePendingPageAgain() throws {
        fixture.page("/very-slow", "<!doctype html><title>Finally</title>", delay: 1.5)
        let (view, session) = mounted("tab-stop")
        _ = sessions.navigate(id: "tab-stop", url: "\(fixture.base)/very-slow", profile: "default", environment: "env-1")
        spin(until: { self.fixture.seen.contains("/very-slow") }, timeout: 3)
        XCTAssertEqual(kind(session), "Loading")
        XCTAssertTrue(sessions.command(id: "tab-stop", name: "refresh"), "the reference's Stop button reloads (PreviewManager.refresh)")
        spin(until: { self.kind(session) == "Success" })
        XCTAssertEqual(session.report["title"] as? String, "Finally")
        XCTAssertEqual(session.report["url"] as? String, "\(fixture.base)/very-slow", "the pending page, not about:blank")
        view.destroy()
    }

    func testALoadFailureIsReportedUntilTheNextLoad() throws {
        // A loopback port nothing listens on any more: the connection is refused.
        var refusedPort: UInt16 = 0
        do { let gone = try Fixture(); refusedPort = gone.port }
        let (view, session) = mounted("tab-fail")
        _ = sessions.navigate(id: "tab-fail", url: "http://127.0.0.1:\(refusedPort)/", profile: "default", environment: "env-1")
        spin(until: { self.kind(session) == "LoadFailed" })
        XCTAssertEqual(kind(session), "LoadFailed")
        XCTAssertEqual(session.report["description"] as? String, "ERR_CONNECTION_REFUSED")
        XCTAssertEqual(session.report["code"] as? Int, NSURLErrorCannotConnectToHost)
        XCTAssertEqual(session.report["failures"] as? Int, 1, "each failure counted once (the data module reports it once)")
        fixture.page("/ok", "<!doctype html><title>Recovered</title>")
        _ = sessions.navigate(id: "tab-fail", url: "\(fixture.base)/ok", profile: "default", environment: "env-1")
        spin(until: { self.kind(session) == "Success" })
        XCTAssertNil(session.report["description"], "a new load clears the failure")
        view.destroy()
    }

    // Review round 1: a pending address must end with its load, whatever way the load ends.
    func testAFragmentJumpAndARedirectedRefusedDownloadDoNotStayLoading() throws {
        fixture.page("/doc", "<!doctype html><title>Doc</title><p id=top>top</p><div style='height:3000px'></div><p id=end>end</p>")
        fixture.redirects["/redirect-to-file"] = "/file.bin"
        fixture.routes["/file.bin"] = (200, "application/octet-stream", Data(repeating: 2, count: 32), 0)
        let (view, session) = mounted("tab-pending", url: "\(fixture.base)/doc")
        spin(until: { self.kind(session) == "Success" })
        _ = sessions.navigate(id: "tab-pending", url: "\(fixture.base)/doc#end", profile: "default", environment: "env-1")
        spin(until: { session.web.url?.fragment == "end" }, timeout: 3)
        spin(until: { self.kind(session) == "Success" }, timeout: 3)
        XCTAssertEqual(kind(session), "Success", "a fragment jump has no load to wait for")
        XCTAssertEqual(session.report["url"] as? String, "\(fixture.base)/doc#end")
        _ = sessions.navigate(id: "tab-pending", url: "\(fixture.base)/redirect-to-file", profile: "default", environment: "env-1")
        spin(until: { self.fixture.seen.contains("/file.bin") }, timeout: 5)
        spin(until: { self.kind(session) != "Loading" }, timeout: 5)
        XCTAssertEqual(kind(session), "Success", "a refused download behind a redirect ends its load; the page stays")
        XCTAssertTrue((session.report["refused"] as? [String] ?? []).contains { $0.hasPrefix("download:") })
        view.destroy()
    }

    func testTwoSessionsKeepTheirOwnPages() {
        let other = T3BrowserSessions(agent: true, changed: { _ in })
        sessions.sync([["id": "mine", "url": "", "profile": "default", "environment": "env-1"]])
        other.sync([["id": "theirs", "url": "", "profile": "default", "environment": "env-1"]])
        sessions.sync([])
        XCTAssertNil(sessions.sessions["mine"])
        XCTAssertNotNil(other.sessions["theirs"], "one session's sync closes only its own pages")
        other.sync([])
    }

    func testAFaviconFetchCancelledWithItsTabDoesNotRunOn() throws {
        // Several icons, the first slow: the tab closes while it downloads; the next candidate must not be asked
        // for on the invalidated session (NSGenericException).
        fixture.page("/icons", "<!doctype html><title>Icons</title><link rel=icon href=/slow-icon.png><link rel=icon href=/icon-2.png><link rel=icon href=/icon-3.png>")
        fixture.routes["/slow-icon.png"] = (404, "image/png", Data(), 0.6)
        fixture.routes["/icon-2.png"] = (200, "image/png", redPNG, 0)
        let (view, session) = mounted("tab-icons", url: "\(fixture.base)/icons")
        spin(until: { self.fixture.seen.contains("/slow-icon.png") })
        view.destroy()
        sessions.close("tab-icons")
        XCTAssertTrue(session.closed)
        RunLoop.main.run(until: Date().addingTimeInterval(1.2))
        XCTAssertFalse(fixture.seen.contains("/icon-2.png"), "no candidate is fetched after the tab closed")
    }

    func testAMissingPageIsAPageNotAFailure() throws {
        let (view, session) = mounted("tab-404", url: "\(fixture.base)/missing")
        spin(until: { self.kind(session) == "Success" })
        XCTAssertEqual(session.report["title"] as? String, "Not found", "a 404 renders, as in Chromium")
        view.destroy()
    }

    func testCancelledAndPolicyStoppedLoadsAreNotFailures() {
        XCTAssertNil(T3BrowserSession.loadFailure(NSError(domain: NSURLErrorDomain, code: NSURLErrorCancelled), url: "http://x/"))
        XCTAssertNil(T3BrowserSession.loadFailure(NSError(domain: "WebKitErrorDomain", code: 102), url: "http://x/"))
        let failure = T3BrowserSession.loadFailure(NSError(domain: NSURLErrorDomain, code: NSURLErrorCannotFindHost), url: "http://nowhere.invalid/")
        XCTAssertEqual(failure?.description, "ERR_NAME_NOT_RESOLVED")
        XCTAssertEqual(T3BrowserSession.loadFailure(NSError(domain: NSURLErrorDomain, code: NSURLErrorServerCertificateUntrusted), url: "https://x/")?.description, "ERR_CERT_AUTHORITY_INVALID")
        XCTAssertEqual(T3BrowserSession.loadFailure(NSError(domain: "Elsewhere", code: 7, userInfo: [NSLocalizedDescriptionKey: "Odd failure"]), url: "http://x/")?.description, "Odd failure")
    }

    func testACrashedPageRecoversAtItsURL() throws {
        fixture.page("/crash", "<!doctype html><title>Crash me</title>")
        let (view, session) = mounted("tab-crash", url: "\(fixture.base)/crash")
        spin(until: { self.kind(session) == "Success" })
        // The page's content process dies unexpectedly (SIGKILL on WebKit's `_webProcessIdentifier`), as a crash does.
        let pid = (session.web.value(forKey: "_webProcessIdentifier") as? NSNumber)?.int32Value ?? 0
        guard pid > 0 else { throw XCTSkip("this WebKit does not name its content process") }
        let before = fixture.seen.filter { $0 == "/crash" }.count
        Darwin.kill(pid, SIGKILL)
        spin(until: { (session.report["crashes"] as? Int ?? 0) == 1 })
        XCTAssertEqual(session.report["crashes"] as? Int, 1, "webViewWebContentProcessDidTerminate")
        spin(until: { self.fixture.seen.filter { $0 == "/crash" }.count > before && self.kind(session) == "Success" })
        XCTAssertGreaterThan(fixture.seen.filter { $0 == "/crash" }.count, before, "recovered by loading the last URL again (250 ms later)")
        XCTAssertEqual(session.report["crashed"] as? Bool, false)
        XCTAssertEqual(session.report["title"] as? String, "Crash me")
        view.destroy()
    }

    func testAPageCannotLeaveTheWebOrDownload() throws {
        fixture.page("/links", "<!doctype html><title>Links</title><a id=m href='mailto:someone@example.com'>mail</a><a id=d href=/file.bin>file</a>")
        fixture.routes["/file.bin"] = (200, "application/octet-stream", Data(repeating: 1, count: 64), 0)
        let (view, session) = mounted("tab-schemes", url: "\(fixture.base)/links")
        spin(until: { self.kind(session) == "Success" })
        _ = evaluate(session.web, "document.getElementById('m').click(); 'ok'")
        spin(until: { (session.report["refused"] as? [String] ?? []).contains { $0.hasPrefix("mailto:") } }, timeout: 3)
        XCTAssertTrue((session.report["refused"] as? [String] ?? []).contains { $0.hasPrefix("mailto:") }, "the main frame refuses mailto:")
        _ = evaluate(session.web, "document.getElementById('d').click(); 'ok'")
        spin(until: { (session.report["refused"] as? [String] ?? []).contains { $0.hasPrefix("download:") } }, timeout: 5)
        XCTAssertTrue((session.report["refused"] as? [String] ?? []).contains { $0.hasPrefix("download:") }, "a download waits for part 3")
        spin(until: { self.kind(session) == "Success" }, timeout: 3)
        XCTAssertEqual(session.report["title"] as? String, "Links", "the page stays where it was")
        view.destroy()
    }

    func testScriptedPopupsOpenAWindowAndBlankLinksStayInTheTab() throws {
        fixture.page("/opener", "<!doctype html><title>Opener</title><a id=blank href=/target target=_blank>blank</a>")
        fixture.page("/target", "<!doctype html><title>Target</title>")
        fixture.page("/auth", "<!doctype html><title>Sign in</title><script>window.opener && window.opener.postMessage('hello', '*')</script>")
        let (view, session) = mounted("tab-popups", url: "\(fixture.base)/opener")
        spin(until: { self.kind(session) == "Success" })
        _ = evaluate(session.web, "window.got = ''; window.addEventListener('message', e => window.got = e.data); 'ok'")
        // evaluateJavaScript runs as a user gesture, so the popup blocker lets the scripted window open.
        _ = evaluate(session.web, "String(!!window.open('/auth', 'signin', 'width=420,height=360'))")
        spin(until: { (session.report["popups"] as? Int ?? 0) == 1 }, timeout: 5)
        XCTAssertEqual(session.report["popups"] as? Int, 1, "a scripted pop-up opens a real window")
        let popup = try XCTUnwrap(session.popups.first)
        spin(until: { popup.web.title == "Sign in" })
        spin(until: { self.evaluate(session.web, "String(window.got)") == "hello" }, timeout: 5)
        XCTAssertEqual(evaluate(session.web, "String(window.got)"), "hello", "the pop-up keeps its opener")
        XCTAssertEqual(popup.window.frame.size.width, 420, accuracy: 1)
        XCTAssertEqual(evaluate(popup.web, "String(!!window.open('/target', 'again', 'width=200,height=200'))"), "false", "the chain stops at the first pop-up")
        popup.close()
        spin(until: { (session.report["popups"] as? Int ?? 1) == 0 }, timeout: 3)
        XCTAssertEqual(session.report["popups"] as? Int, 0)
        _ = evaluate(session.web, "document.getElementById('blank').click(); 'ok'")
        spin(until: { session.report["title"] as? String == "Target" })
        XCTAssertEqual(session.report["title"] as? String, "Target", "a target=_blank link loads in the tab")
        view.destroy()
    }

    func testTheCameraAndMicrophoneAreDenied() throws {
        fixture.page("/media", "<!doctype html><title>Media</title>")
        let (view, session) = mounted("tab-media", url: "\(fixture.base)/media")
        spin(until: { self.kind(session) == "Success" })
        // The delegate is asked only where the page may ask at all (a secure context; loopback is one).
        _ = evaluate(session.web, "navigator.mediaDevices.getUserMedia({ video: true }).then(() => document.title = 'granted', e => document.title = 'denied:' + e.name); 'ok'")
        spin(until: { (session.report["title"] as? String ?? "").hasPrefix("denied") || (session.report["title"] as? String) == "granted" }, timeout: 5)
        XCTAssertNotEqual(session.report["title"] as? String, "granted", "camera access is not granted")
        if (session.report["refused"] as? [String] ?? []).isEmpty { print("note: WebKit refused before asking the delegate (no capture device in this run)") }
        else { XCTAssertTrue((session.report["refused"] as? [String] ?? []).contains("permission:camera")) }
        view.destroy()
    }

    func testThePageKeepsWebKitsNativeUserAgentAndIsInspectableInDevelopment() throws {
        fixture.page("/ua", "<!doctype html><title>UA</title>")
        let (view, session) = mounted("tab-ua", url: "\(fixture.base)/ua")
        spin(until: { self.kind(session) == "Success" })
        XCTAssertTrue(session.web.customUserAgent?.isEmpty ?? true, "nothing rewrites the user agent")
        let agent = evaluate(session.web, "navigator.userAgent")
        XCTAssertTrue(agent.contains("AppleWebKit"), agent)
        XCTAssertFalse(agent.contains("T3") || agent.contains("Exact"), agent)
        XCTAssertEqual(session.web.isInspectable, T3WebInspection.enabled)
        XCTAssertTrue(session.web.isInspectable, "this binary is a development build (no receipt, no distribution.json)")
        view.destroy()
    }

    func testATabOutlivesItsViewAndClosesWithItsSession() throws {
        fixture.page("/kept", "<!doctype html><title>Kept</title><input id=f>")
        let (first, session) = mounted("tab-kept", url: "\(fixture.base)/kept")
        spin(until: { self.kind(session) == "Success" })
        _ = evaluate(session.web, "document.getElementById('f').value = 'typed'; 'ok'")
        XCTAssertTrue(session.web.superview === first.host)
        first.destroy() // the panel hides or the thread changes
        XCTAssertNil(session.web.superview, "the view gives the page back")
        XCTAssertNotNil(sessions.sessions["tab-kept"], "the session keeps its page")
        let (second, again) = mounted("tab-kept")
        XCTAssertTrue(again === session && session.web.superview === second.host, "a new view shows the same page")
        XCTAssertEqual(evaluate(session.web, "document.getElementById('f').value"), "typed", "with its state")
        // A view handed a newer page for the same tab keeps working; an old view's destroy cannot take it back.
        let (third, _) = mounted("tab-kept")
        second.destroy()
        XCTAssertTrue(session.web.superview === third.host)
        third.destroy()
        sessions.sync([["id": "other", "url": "", "profile": "default", "environment": "env-1"]])
        XCTAssertNil(sessions.sessions["tab-kept"], "a session the data module no longer lists closes")
        XCTAssertTrue(session.closed)
        XCTAssertNotNil(sessions.sessions["other"])
    }

    func testTheModuleOps() {
        fixture.page("/op", "<!doctype html><title>Op</title>")
        let synced = sessions.perform(["op": "browserSync", "tabs": [["id": "tab-op", "url": "", "profile": "default", "environment": "env-1"]], "generation": 3])
        XCTAssertEqual(synced?["ok"] as? Bool, true)
        XCTAssertEqual(synced?["generation"] as? Int, 3)
        XCTAssertEqual((sessions.status["browserTabs"] as? [String: Any])?.keys.sorted(), ["tab-op"])
        let started = sessions.perform(["op": "browserNavigate", "tab": "tab-op", "url": "\(fixture.base)/op", "profile": "default", "environment": "env-1"])
        XCTAssertEqual((started?["value"] as? [String: Any])?["started"] as? Bool, true)
        let refused = sessions.perform(["op": "browserNavigate", "tab": "tab-op", "url": "file:///etc/hosts"])
        XCTAssertEqual((refused?["value"] as? [String: Any])?["started"] as? Bool, false, "only http(s) loads")
        XCTAssertNil(sessions.perform(["op": "somethingElse"]), "other ops go to the next area")
        let session = sessions.sessions["tab-op"]!
        spin(until: { self.kind(session) == "Success" })
        XCTAssertEqual(((sessions.status["browserTabs"] as? [String: Any])?["tab-op"] as? [String: Any])?["title"] as? String, "Op")
    }

    func testEachEnvironmentProfileHasItsOwnStore() {
        let a = T3BrowserSessions.storeIdentifier(environment: "env-1", profile: "default")
        XCTAssertEqual(a, T3BrowserSessions.storeIdentifier(environment: "env-1", profile: "default"), "stable across launches")
        XCTAssertNotEqual(a, T3BrowserSessions.storeIdentifier(environment: "env-2", profile: "default"))
        XCTAssertNotEqual(a, T3BrowserSessions.storeIdentifier(environment: "env-1", profile: "work"))
        XCTAssertFalse(sessions.store(environment: "env-1", profile: "default").isPersistent, "an agent run keeps the pages' data in memory")
    }
}

final class BrowserReferenceTests: XCTestCase {
    // webviewCrashRecovery.test.ts
    func testBacksOffAndStopsAfterABoundedNumberOfRapidCrashes() throws {
        let first = try XCTUnwrap(T3BrowserCrashRecovery().plan(now: 1_000))
        XCTAssertEqual(first.delayMs, 250)
        let second = try XCTUnwrap(first.state.plan(now: 1_100))
        XCTAssertEqual(second.delayMs, 500)
        let third = try XCTUnwrap(second.state.plan(now: 1_200))
        XCTAssertEqual(third.delayMs, 1_000)
        XCTAssertNil(third.state.plan(now: 1_300))
    }

    func testAllowsRecoveryAgainAfterTheCrashWindowExpires() throws {
        let first = try XCTUnwrap(T3BrowserCrashRecovery().plan(now: 1_000))
        let second = try XCTUnwrap(first.state.plan(now: 1_100))
        let third = try XCTUnwrap(second.state.plan(now: 1_200))
        let again = try XCTUnwrap(third.state.plan(now: 1_000 + T3BrowserCrashRecovery.windowMs))
        XCTAssertEqual(again.delayMs, 250)
        XCTAssertEqual(again.state.attempts, 1)
        XCTAssertEqual(again.state.windowStartedAt, 1_000 + T3BrowserCrashRecovery.windowMs)
    }

    // Manager.test.ts previewWindowOpenAction
    func testOpensARealWindowForScriptedPopupsSoTheOpenerSurvives() {
        XCTAssertEqual(T3BrowserSession.windowOpenAction(url: "https://accounts.google.com/o/oauth2/auth", newWindow: true), "popup")
        XCTAssertEqual(T3BrowserSession.windowOpenAction(url: "http://localhost:5173/auth", newWindow: true), "popup")
    }

    func testKeepsTargetBlankLinksInThePreviewTab() {
        XCTAssertEqual(T3BrowserSession.windowOpenAction(url: "https://accounts.google.com/o/oauth2/auth", newWindow: false), "navigate")
    }

    func testDoesNotHandAWindowToSchemesThatCannotBeHardened() {
        for url in ["about:blank", "javascript:alert(1)", "file:///etc/passwd", "vscode://vscode-remote/ssh-remote+box/tmp", "not a url"] {
            XCTAssertEqual(T3BrowserSession.windowOpenAction(url: url, newWindow: true), "navigate", url)
        }
    }

    func testFaviconCandidatesAreBoundedAsTheReferenceBoundsThem() throws {
        let many = (0..<12).map { "https://example.com/icon-\($0).png" }
        XCTAssertEqual(T3BrowserFavicon.selectCandidates(many + many).count, 8, "eight at most, deduplicated")
        XCTAssertEqual(T3BrowserFavicon.selectCandidates(["javascript:1", "file:///x.png", "https://example.com/" + String(repeating: "a", count: 2_100), "data:text/plain,hi", "data:image/png;base64,AA=="]), ["data:image/png;base64,AA=="])
        XCTAssertEqual(T3BrowserFavicon.origin(URL(string: "HTTP://Example.com:8080/a")), "http://example.com:8080")
        XCTAssertNil(T3BrowserFavicon.origin(URL(string: "about:blank")))
        let image = NSImage(data: redPNG)!
        let url = try XCTUnwrap(T3BrowserFavicon.dataURL(image))
        XCTAssertTrue(url.hasPrefix("data:image/png;base64,"))
    }
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(name: "browser")
suite.addTest(XCTestSuite(forTestCaseClass: BrowserReferenceTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: BrowserSessionTests.self))
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
