import AppKit
import CryptoKit
import ImageIO
import Network
import UniformTypeIdentifiers
import WebKit
import XCTest

// Task media-actions: T3MediaActions.swift (the media menu, Save, Copy image, Copy path) against a
// loopback server, and the rendered HTML preview's external hosts (R6MediaPreview.swift). Every
// pasteboard write goes to the agent's private pasteboard; saves go to a temporary exports/.
// `T3_MEDIA_ACTIONS_NETWORK=1` adds the public https host (needs the network).

final class Fixture {
    private let listener: NWListener
    private(set) var paths: [String] = []
    var routes: [String: (Int, String, Data)] = [:]
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "media-actions-fixture")

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
    var base: String { "http://127.0.0.1:\(port)" }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return connection.cancel() }
            let path = head.split(separator: " ").dropFirst().first.map(String.init) ?? ""
            self.paths.append(path)
            let route = String(path.split(separator: "?").first ?? "")
            let (status, type, body) = self.routes[route] ?? (404, "text/plain", Data("missing".utf8))
            var response = Data("HTTP/1.1 \(status) \(status == 200 ? "OK" : "Error")\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n".utf8)
            response.append(body)
            connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
        }
    }
    deinit { listener.cancel() }
}

/// A raster of the given size and type, drawn here.
func raster(width: Int, height: Int, type: UTType) -> Data {
    let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(CGColor(red: 0.2, green: 0.4, blue: 0.9, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let data = NSMutableData()
    let destination = CGImageDestinationCreateWithData(data, type.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, context.makeImage()!, nil)
    CGImageDestinationFinalize(destination)
    return data as Data
}

/// A real grayscale PNG of width × height (70,000,000 pixels compress to a few hundred KB).
func grayPng(width: Int, height: Int) -> Data {
    let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
    context.setFillColor(gray: 0.5, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let data = NSMutableData()
    let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, context.makeImage()!, nil)
    CGImageDestinationFinalize(destination)
    return data as Data
}

final class MediaActionsTests: XCTestCase {
    private var fixture: Fixture!
    private var exports: URL!

    override func setUpWithError() throws {
        fixture = try Fixture()
        exports = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-media-actions-\(UUID().uuidString)/exports", isDirectory: true)
    }

    private func run(_ request: [String: Any]) -> [String: Any] {
        let done = expectation(description: request["op"] as? String ?? "op")
        var value: [String: Any] = [:]
        T3MediaActions.perform(request.merging(["generation": 7]) { first, _ in first }, exportsRoot: exports) { reply in
            XCTAssertEqual(reply["generation"] as? Int, 7)
            value = reply["value"] as? [String: Any] ?? ["reply": reply]
            done.fulfill()
        }
        wait(for: [done], timeout: 15)
        return value
    }
    private var board: NSPasteboard { NSPasteboard(name: T3MediaActions.agentPasteboard) }
    private func pastedSize() -> (Int, Int)? {
        guard let png = board.data(forType: .png), let source = CGImageSourceCreateWithData(png as CFData, nil),
              CGImageSourceGetType(source) as String? == UTType.png.identifier,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any] else { return nil }
        return (properties[kCGImagePropertyPixelWidth] as? Int ?? 0, properties[kCGImagePropertyPixelHeight] as? Int ?? 0)
    }

    func testMenuKeepsTheReferenceOrderAndDisabledItems() {
        _ = NSApplication.shared
        // mediaMenuItems for a workspace image with no URL yet and no asset (media-actions.ts).
        let items: [[String: Any]] = [["id": "copy-full-path", "label": "Copy full path"], ["id": "copy-relative-path", "label": "Copy relative path"],
                                      ["id": "open-file", "label": "Open in file viewer"], ["id": "save", "label": "Save image", "disabled": true],
                                      ["id": "copy-image", "label": "Copy image", "disabled": true]]
        let owner = T3ContextMenu()
        let menu = owner.menu(for: items)
        XCTAssertEqual(menu.items.map(\.title), ["Copy full path", "Copy relative path", "Open in file viewer", "Save image", "Copy image"])
        XCTAssertEqual(menu.items.map(\.isEnabled), [true, true, true, false, false])
        XCTAssertFalse(menu.items.contains(where: \.isSeparatorItem))
        menu.performActionForItem(at: 1)
        XCTAssertEqual(owner.picked, "copy-relative-path")
    }

    func testAgentMenuAnswersTheDrivePicksAndTheKeyboardCorner() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 40, y: 40, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        let media = NSButton(frame: NSRect(x: 30, y: 100, width: 120, height: 80))
        window.contentView!.addSubview(media)
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(media)
        let items: [[String: Any]] = [["id": "copy-url", "label": "Copy URL"], ["id": "save", "label": "Save video"]]
        // T3_AGENT_MEDIA_PICKS is "save,copy-image,save" (set before the suite runs).
        let first = T3MediaActions.menu(items, anchor: "bottom-left", agent: true)
        XCTAssertEqual(first["id"] as? String, "save")
        XCTAssertEqual(first["shown"] as? Bool, false)
        XCTAssertEqual(first["items"] as? [String], ["Copy URL", "Save video"])
        // The bottom-left corner in the content's top-left space: x 30, y 300 - 100 = 200.
        XCTAssertEqual((first["anchor"] as? [CGFloat]) ?? [], [30, 200])
        let second = T3MediaActions.menu(items, anchor: nil, agent: true)
        XCTAssertTrue(second["id"] is NSNull, "a pick the menu does not offer (Copy image on a video) answers dismissed")
        XCTAssertTrue(second["anchor"] is NSNull)
        let third = T3MediaActions.menu([["id": "save", "label": "Save image", "disabled": true]], anchor: nil, agent: true)
        XCTAssertTrue(third["id"] is NSNull, "a disabled item cannot be picked")
        window.orderOut(nil)
    }

    func testCopyTextWritesTheAgentPasteboard() {
        XCTAssertEqual(run(["op": "mediaCopyText", "text": "/repo/screens/logo.png"])["ok"] as? Bool, true)
        XCTAssertEqual(board.string(forType: .string), "/repo/screens/logo.png")
    }

    func testCopyImageWritesPngOfTheSamePixelSize() throws {
        let png = raster(width: 31, height: 17, type: .png), jpeg = raster(width: 40, height: 24, type: .jpeg)
        fixture.routes["/api/assets/a/shot.png"] = (200, "image/png", png)
        fixture.routes["/api/assets/a/photo.jpg"] = (200, "image/jpeg", jpeg)
        fixture.routes["/api/assets/a/mark.svg"] = (200, "image/svg+xml", Data("<svg xmlns='http://www.w3.org/2000/svg' width='48' height='20'><rect width='48' height='20' fill='red'/></svg>".utf8))
        XCTAssertEqual(run(["op": "mediaCopyImage", "url": "\(fixture.base)/api/assets/a/shot.png"])["ok"] as? Bool, true)
        XCTAssertEqual(board.data(forType: .png), png, "a PNG goes to the pasteboard as it is")
        let jpegReply = run(["op": "mediaCopyImage", "url": "\(fixture.base)/api/assets/a/photo.jpg"])
        XCTAssertEqual(jpegReply["ok"] as? Bool, true)
        XCTAssertEqual(pastedSize().map { [$0.0, $0.1] }, [40, 24])
        let svgReply = run(["op": "mediaCopyImage", "url": "\(fixture.base)/api/assets/a/mark.svg"])
        XCTAssertEqual(svgReply["ok"] as? Bool, true, "\(svgReply)")
        XCTAssertEqual(pastedSize().map { [$0.0, $0.1] }, [48, 20])
        print("copy image: png 31x17 bytes equal; jpeg 40x24 -> png \(jpegReply["width"] ?? "?")x\(jpegReply["height"] ?? "?"); svg -> png \(svgReply["width"] ?? "?")x\(svgReply["height"] ?? "?")")
    }

    func testCopyImageRefusalsUseTheReferenceSentences() {
        // Served as WebP so it is decoded, not passed through as PNG: 10,000 × 7,000 = 70,000,000 pixels.
        fixture.routes["/api/assets/a/huge.webp"] = (200, "image/webp", grayPng(width: 10_000, height: 7_000))
        fixture.routes["/api/assets/a/page"] = (200, "text/html; charset=utf-8", Data("<!doctype html><p>sign in".utf8))
        fixture.routes["/api/assets/a/garbage.jpg"] = (200, "image/jpeg", Data("not an image".utf8))
        let cases: [(String, String)] = [
            ("\(fixture.base)/api/assets/a/huge.webp", "This image is too large or has no usable dimensions. Try saving it instead."),
            ("\(fixture.base)/api/assets/a/page", "This link returned a web page instead of media. Open the original URL."),
            ("\(fixture.base)/api/assets/a/gone.png", "The file could not be fetched (HTTP 404)."),
            ("\(fixture.base)/api/assets/a/garbage.jpg", "The browser could not decode this image for copying. Try saving it instead."),
            ("http://127.0.0.1:9/unreachable.png", "The file could not be fetched. The host may block browser access (CORS), or the connection may be unavailable."),
        ]
        for (url, message) in cases {
            let reply = run(["op": "mediaCopyImage", "url": url])
            XCTAssertEqual(reply["ok"] as? Bool, false, url)
            XCTAssertEqual(reply["message"] as? String, message, url)
        }
        XCTAssertEqual(run(["op": "mediaCopyImage", "url": "file:///etc/hosts"])["message"] as? String, "This media is unavailable. Try reopening the preview.")
        print("copy image refusals: \(cases.count + 1) sentences as the reference")
    }

    func testSaveWritesTheSameBytesUnderTheMediaName() throws {
        let video = Data((0..<200_000).map { UInt8($0 % 251) })
        fixture.routes["/api/assets/v/clip.mp4"] = (200, "video/mp4", video)
        let reply = run(["op": "mediaSave", "url": "\(fixture.base)/api/assets/v/clip.mp4?sig=1", "name": "clip one.mp4"])
        XCTAssertEqual(reply["ok"] as? Bool, true)
        XCTAssertEqual(reply["name"] as? String, "clip one.mp4")
        let saved = try Data(contentsOf: exports.appendingPathComponent("clip one.mp4"))
        XCTAssertEqual(SHA256.hash(data: saved).description, SHA256.hash(data: video).description)
        XCTAssertEqual(run(["op": "mediaSave", "url": "\(fixture.base)/api/assets/v/clip.mp4", "name": "clip one.mp4"])["name"] as? String, "clip one (1).mp4")
        XCTAssertEqual(run(["op": "mediaSave", "url": "\(fixture.base)/api/assets/v/expired.mp4", "name": "x.mp4"])["message"] as? String, "The file could not be fetched (HTTP 404).")
        let panel = T3MediaActions.savePanel(name: "clip one.mp4")
        XCTAssertEqual(panel.nameFieldStringValue, "clip one.mp4")
        XCTAssertEqual(panel.directoryURL?.lastPathComponent, "Downloads", "the first Save opens in Downloads, as Electron's")
        XCTAssertEqual(panel.allowedContentTypes, [UTType(filenameExtension: "mp4")!])
        XCTAssertTrue(panel.allowsOtherFileTypes)
        print("save: sha256 \(SHA256.hash(data: saved).description.suffix(16)) equal to the source; second save named \"clip one (1).mp4\"")
    }

    /// The rendered HTML preview loads assets from other hosts as the reference's sandboxed frame does:
    /// an http IP host (stylesheet, script, fetch), a named http host resolving to loopback
    /// (localtest.me: this unbundled binary has no ATS; the bundle allows it through app.json's
    /// `appTransportSecurity`, exact2 #106), and, with the
    /// network, a public https image.
    func testRenderedHtmlLoadsExternalAssets() throws {
        _ = NSApplication.shared
        let other = try Fixture()
        let media = R6MediaPreview(agent: true)
        let window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 520, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        let host = NSView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(host)
        window.orderFrontRegardless()
        defer { media.destroy(); window.orderOut(nil) }
        let ip = "http://127.0.0.1:\(other.port)", named = "http://localtest.me:\(other.port)"
        let network = ProcessInfo.processInfo.environment["T3_MEDIA_ACTIONS_NETWORK"] == "1"
        other.routes["/style.css"] = (200, "text/css", Data("h1 { color: rgb(9, 8, 7); }".utf8))
        other.routes["/app.js"] = (200, "text/javascript", Data("window.external = 'ran';".utf8))
        other.routes["/data.json"] = (200, "application/json", Data("{\"ok\":1}".utf8))
        other.routes["/named.svg"] = (200, "image/svg+xml", Data("<svg xmlns='http://www.w3.org/2000/svg' width='14' height='14'><rect width='14' height='14' fill='green'/></svg>".utf8))
        fixture.routes["/api/assets/tok/page.html"] = (200, "text/html", Data("""
        <!doctype html><html><head><link rel="stylesheet" href="\(ip)/style.css"><script src="\(ip)/app.js"></script></head>
        <body><h1 id="h">External assets</h1><img id="n" src="\(named)/named.svg" alt="">\(network ? "<img id=\"s\" src=\"https://www.google.com/favicon.ico\" alt=\"\">" : "")
        <script>fetch('\(ip)/data.json').then(function (r) { return r.text(); }).then(function (t) { document.body.dataset.fetched = t; }, function () { document.body.dataset.fetched = 'refused'; });</script>
        </body></html>
        """.utf8))
        media.mount(host: host, kind: "html", raw: "\(fixture.base)/api/assets/tok/page.html", name: "page.html")
        let web = try XCTUnwrap(host.subviews.first as? WKWebView)
        func evaluate(_ script: String) -> String {
            var value: String?
            let end = Date().addingTimeInterval(5)
            while media.documentFrame(of: web) == nil && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
            guard let frame = media.documentFrame(of: web) else { return "no frame" }
            web.evaluateJavaScript(script, in: frame, in: .page) { result in
                switch result { case .success(let any): value = (any as? String) ?? "\(any)"; case .failure(let error): value = "error: \(error.localizedDescription)" }
            }
            while value == nil && Date() < end.addingTimeInterval(5) { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
            return value ?? "timeout"
        }
        func settle(_ script: String, _ expected: String) -> String {
            let end = Date().addingTimeInterval(network ? 10 : 6)
            var value = evaluate(script)
            while value != expected && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.1)); value = evaluate(script) }
            return value
        }
        XCTAssertEqual(settle("getComputedStyle(document.getElementById('h')).color", "rgb(9, 8, 7)"), "rgb(9, 8, 7)", "http IP stylesheet")
        XCTAssertEqual(settle("String(window.external)", "ran"), "ran", "http IP script")
        XCTAssertEqual(settle("String(document.body.dataset.fetched)", "{\"ok\":1}"), "{\"ok\":1}", "http IP fetch")
        XCTAssertEqual(settle("String(document.getElementById('n').naturalWidth)", "14"), "14", "named http host (loopback)")
        var https = "not run"
        if network { https = settle("String(document.getElementById('s').naturalWidth > 0)", "true"); XCTAssertEqual(https, "true", "public https image") }
        let reached = Set(other.seen.map { String($0.split(separator: "?").first ?? "") })
        XCTAssertEqual(reached, ["/style.css", "/app.js", "/data.json", "/named.svg"])
        print("rendered html: http IP css/js/fetch loaded; named http host (localtest.me) loaded in this unbundled binary; public https image: \(https)")
    }
}

setenv("T3_AGENT_MEDIA_PICKS", "save,copy-image,save", 1)
_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(forTestCaseClass: MediaActionsTests.self)
suite.run()
let outcome = suite.testRun!
print("Executed \(outcome.executionCount) tests, with \(outcome.totalFailureCount) failures")
exit(outcome.totalFailureCount == 0 ? 0 : 1)
