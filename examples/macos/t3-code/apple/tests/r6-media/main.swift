import AppKit
import Network
import PDFKit
import WebKit
import XCTest

// Lane r6-media: the attachment preview's PDF and HTML bodies (R6MediaPreview.swift) against a
// loopback asset server. Lane r12-render: the HTML page runs in the reference's sandboxed iframe
// (an opaque origin with no storage, no top navigation) and, as there, may load from any host and
// navigate its frame in place; a second loopback server stands in for another host.
// `T3_MEDIA_TEST_DIR` receives the rendered PNGs.
final class LoopbackAssets {
    private let listener: NWListener
    private(set) var paths: [String] = []
    var bodies: [String: (String, Data)] = [:]
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "r6-media-assets")

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }

    var seen: [String] { queue.sync { paths } }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return connection.cancel() }
            let path = head.split(separator: " ").dropFirst().first.map(String.init) ?? ""
            self.paths.append(path)
            let route = String(path.split(separator: "?").first ?? "")
            let (type, body) = self.bodies[route] ?? ("text/plain", Data("missing".utf8))
            let status = self.bodies[route] == nil ? "404 Not Found" : "200 OK"
            var response = Data("HTTP/1.1 \(status)\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n".utf8)
            response.append(body)
            connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
        }
    }
    deinit { listener.cancel() }
}

final class MediaPreviewTests: XCTestCase {
    private var window: NSWindow!
    private var host: NSView!
    private var assets: LoopbackAssets!
    private let media = R6MediaPreview(agent: true)

    override func setUpWithError() throws {
        assets = try LoopbackAssets()
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 560, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
        host = NSView(frame: window.contentView!.bounds)
        host.autoresizingMask = [.width, .height]
        window.contentView!.addSubview(host)
        window.orderFrontRegardless()
    }
    override func tearDown() { media.destroy(); window.orderOut(nil) }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
    }
    private func evaluate(_ web: WKWebView, _ script: String, frame: Bool = true) -> String {
        var value: String?
        let done: (Result<Any, Error>) -> Void = { result in
            switch result {
            case .success(let any): value = (any as? String) ?? "\(any)"
            case .failure(let error): value = "error: \(error.localizedDescription)"
            }
        }
        if frame {
            spin(until: { self.media.documentFrame(of: web) != nil }, timeout: 5)
            guard let info = media.documentFrame(of: web) else { return "no frame" }
            web.evaluateJavaScript(script, in: info, in: .page, completionHandler: done)
        } else {
            web.evaluateJavaScript(script, in: nil, in: .page, completionHandler: done)
        }
        spin(until: { value != nil }, timeout: 5)
        return value ?? "timeout"
    }
    private func save(_ view: NSView, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["T3_MEDIA_TEST_DIR"] else { return }
        if let web = view as? WKWebView {
            var done = false
            web.takeSnapshot(with: nil) { image, _ in
                if let image, let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
                    try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
                }
                done = true
            }
            spin(until: { done }, timeout: 5)
            return
        }
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
        view.cacheDisplay(in: view.bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
    }

    func testHtmlRunsInTheReferenceSandbox() throws {
        let other = try LoopbackAssets()
        let base = "http://127.0.0.1:\(assets.port)", away = "http://127.0.0.1:\(other.port)"
        other.bodies["/img.svg"] = ("image/svg+xml", Data("<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12'><rect width='12' height='12' fill='blue'/></svg>".utf8))
        other.bodies["/script.js"] = ("text/javascript", Data("window.remote = 'remote:' + self.origin;".utf8))
        other.bodies["/next.html"] = ("text/html", Data("<!doctype html><title>next</title><p id=n>navigated</p>".utf8))
        assets.bodies["/api/assets/page"] = ("text/html", Data("""
        <!doctype html><html><head><script src="\(away)/script.js"></script></head><body><h1>Fixture page</h1><p id="s">no script</p>
        <img id="i" src="\(away)/img.svg" alt="">
        <a id="l" href="\(away)/next.html">link</a> <a id="b" href="\(away)/beacon-blank" target="_blank">popup</a>
        <a id="t" href="\(away)/beacon-top" target="_top">top</a>
        <script>
        document.getElementById('s').textContent = 'ran:' + self.origin;
        fetch('\(away)/beacon-fetch').catch(function () {});
        try { localStorage.setItem('a', 'b'); document.title = 'storage'; } catch (e) { document.title = 'no storage'; }
        </script></body></html>
        """.utf8))
        media.mount(host: host, kind: "html", raw: "\(base)/api/assets/page?sig=1", name: "page.html")
        let web = try XCTUnwrap(host.subviews.first as? WKWebView)
        spin(until: { !web.isLoading && web.url != nil && (self.media.loaded["page.html"] != nil) })
        spin(until: { self.evaluate(web, "document.getElementById('s').textContent") != "no script" }, timeout: 5)
        XCTAssertEqual(evaluate(web, "document.getElementById('s').textContent"), "ran:null", "an opaque origin, as the sandboxed frame")
        XCTAssertEqual(evaluate(web, "document.title"), "no storage")
        XCTAssertEqual(evaluate(web, "String(window.remote)"), "remote:null", "a script from another host ran, as the reference's CSP allows")
        spin(until: { self.evaluate(web, "String(document.getElementById('i').naturalWidth)") == "12" }, timeout: 5)
        XCTAssertEqual(evaluate(web, "String(document.getElementById('i').naturalWidth)"), "12", "an image from another host painted")
        XCTAssertEqual(evaluate(web, "document.querySelector('iframe').getAttribute('sandbox')", frame: false), "allow-scripts allow-forms allow-popups allow-modals")
        save(web, "r6-media-html.png")
        _ = evaluate(web, "document.getElementById('b').click(); document.getElementById('t').click(); 'clicked'")
        RunLoop.main.run(until: Date().addingTimeInterval(0.8))
        XCTAssertEqual(evaluate(web, "document.getElementById('s').textContent"), "ran:null", "neither the popup nor the top link moved the frame")
        XCTAssertEqual(web.url?.absoluteString, "\(base)/api/assets/", "the window still shows the wrapper")
        _ = evaluate(web, "document.getElementById('l').click(); 'clicked'")
        spin(until: { self.evaluate(web, "String(document.getElementById('n') && document.getElementById('n').textContent)") == "navigated" }, timeout: 5)
        XCTAssertEqual(evaluate(web, "document.getElementById('n').textContent"), "navigated", "the link navigated the frame in place")
        XCTAssertEqual(evaluate(web, "self.origin"), "null", "the navigated frame keeps the sandbox")
        XCTAssertEqual(web.url?.absoluteString, "\(base)/api/assets/")
        let reached = Set(other.seen.map { String($0.split(separator: "?").first ?? "") })
        XCTAssertEqual(reached, ["/script.js", "/img.svg", "/beacon-fetch", "/next.html"], "other hosts are reachable; the popup and the top navigation are not")
        XCTAssertTrue(media.refused.contains { $0.hasSuffix("/beacon-blank") }, "the popup was refused (opened externally outside the agent): \(media.refused)")
        XCTAssertEqual(Set(assets.seen.map { String($0.split(separator: "?").first ?? "") }), ["/api/assets/page"])
        XCTAssertEqual(web.appearance?.name, .aqua)
    }

    /// Lane r11-misc: the reference's frame loads its siblings through the signed URL's token
    /// directory; lane r12-render: and, as there, anything else the page names.
    func testSiblingAssetsLoadThroughTheTokenDirectory() throws {
        let base = "http://127.0.0.1:\(assets.port)", dir = "/api/assets/tok.sig"
        assets.bodies["\(dir)/page.html"] = ("text/html", Data("""
        <!doctype html><html><head><link rel="stylesheet" href="style.css"><script src="js/app.js"></script></head>
        <body><h1 id="h">Sibling page</h1><img id="i" src="img/dot.svg" width="20" height="20" alt="">
        <img src="../other/leak.png" alt=""><img src="http://localhost:\(assets.port)\(dir)/alias.png" alt="">
        <script>fetch('data.json').then(function (r) { return r.text(); }).then(function (t) { document.body.dataset.fetched = t; }, function () { document.body.dataset.fetched = 'refused'; });
        fetch('\(base)/beacon-fetch').catch(function () {});</script></body></html>
        """.utf8))
        assets.bodies["\(dir)/style.css"] = ("text/css", Data("h1 { color: rgb(1, 2, 3); }".utf8))
        assets.bodies["\(dir)/js/app.js"] = ("text/javascript", Data("window.sibling = 'ran:' + self.origin;".utf8))
        assets.bodies["\(dir)/img/dot.svg"] = ("image/svg+xml", Data("<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'><rect width='20' height='20' fill='red'/></svg>".utf8))
        assets.bodies["\(dir)/data.json"] = ("application/json", Data("{}".utf8))
        media.mount(host: host, kind: "html", raw: "\(base)\(dir)/page.html?workspace-revision=1", name: "page.html")
        let web = try XCTUnwrap(host.subviews.first as? WKWebView)
        spin(until: { !web.isLoading && self.media.loaded["page.html"] != nil })
        spin(until: { self.evaluate(web, "getComputedStyle(document.getElementById('h')).color") == "rgb(1, 2, 3)" }, timeout: 5)
        XCTAssertEqual(evaluate(web, "getComputedStyle(document.getElementById('h')).color"), "rgb(1, 2, 3)", "the sibling stylesheet applied")
        XCTAssertEqual(evaluate(web, "String(window.sibling)"), "ran:null", "the sibling script ran in the opaque origin")
        XCTAssertEqual(evaluate(web, "String(document.getElementById('i').naturalWidth)"), "20", "the sibling image painted")
        XCTAssertEqual(evaluate(web, "location.href"), "\(base)\(dir)/page.html?workspace-revision=1")
        RunLoop.main.run(until: Date().addingTimeInterval(0.8))
        let seen = Set(assets.seen.map { String($0.split(separator: "?").first ?? "") })
        XCTAssertTrue(seen.isSuperset(of: ["\(dir)/page.html", "\(dir)/style.css", "\(dir)/js/app.js", "\(dir)/img/dot.svg", "\(dir)/data.json", "/api/assets/other/leak.png", "/beacon-fetch"]),
                      "the token directory and, as in the reference, every other path the page names: \(seen)")
        XCTAssertEqual(R6MediaPreview.wrapperURL(for: URL(string: "\(base)\(dir)/page.html?x=1")!)?.absoluteString, "\(base)/api/assets/")
        save(web, "r11-misc-sibling.png")
    }

    func testPdfFitsTheWidthOnTheViewerSurface() throws {
        // Two US-letter pages drawn here.
        let document = PDFDocument()
        for index in 0..<2 {
            let text = NSTextField(labelWithString: "Parity report, page \(index + 1)")
            text.font = .systemFont(ofSize: 24)
            let sheet = NSView(frame: NSRect(x: 0, y: 0, width: 612, height: 792))
            text.frame = NSRect(x: 72, y: 680, width: 468, height: 40)
            sheet.addSubview(text)
            let page = try XCTUnwrap(PDFDocument(data: sheet.dataWithPDF(inside: sheet.bounds))?.page(at: 0))
            document.insert(page, at: index)
        }
        let pdf = try XCTUnwrap(document.dataRepresentation())
        assets.bodies["/api/assets/report"] = ("application/pdf", pdf)
        media.mount(host: host, kind: "pdf", raw: "http://127.0.0.1:\(assets.port)/api/assets/report", name: "report.pdf")
        let view = try XCTUnwrap(host.subviews.first as? PDFView)
        spin(until: { view.document != nil })
        XCTAssertEqual(view.document?.pageCount, 2)
        XCTAssertTrue(view.autoScales)
        XCTAssertEqual(view.displayMode, .singlePageContinuous)
        // FitH: the page spans the view's width, less PDFView's page margins.
        let width = (view.document?.page(at: 0)?.bounds(for: .mediaBox).width ?? 0) * view.scaleFactor
        XCTAssertGreaterThan(width, host.bounds.width * 0.85)
        XCTAssertEqual(view.accessibilityLabel(), "report.pdf")
        save(view, "r6-media-pdf.png")
        media.unmount(host: host)
        XCTAssertTrue(host.subviews.isEmpty)
    }

    func testRefusesAnythingButASignedAssetRoute() {
        media.mount(host: host, kind: "html", raw: "file:///etc/hosts", name: "x.html")
        media.mount(host: host, kind: "html", raw: "http://127.0.0.1:\(assets.port)/elsewhere", name: "x.html")
        media.mount(host: host, kind: "audio", raw: "http://127.0.0.1:\(assets.port)/api/assets/a", name: "a.wav")
        XCTAssertTrue(host.subviews.isEmpty)
        XCTAssertTrue(assets.seen.isEmpty)
    }

    func testFailedLoadSaysSo() throws {
        media.mount(host: host, kind: "pdf", raw: "http://127.0.0.1:\(assets.port)/api/assets/missing", name: "gone.pdf")
        let view = try XCTUnwrap(host.subviews.first)
        spin(until: { view.subviews.contains { ($0 as? NSTextField)?.stringValue == "Could not load this file." } })
        XCTAssertTrue(view.subviews.contains { ($0 as? NSTextField)?.stringValue == "Could not load this file." })
    }
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(forTestCaseClass: MediaPreviewTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
