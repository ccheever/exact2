import AppKit
import AVFoundation
import WebKit
import XCTest

// What a Browser frame hands the `t3-browser-page` hatch, end to end over a real page in a window (T3BrowserPage.swift):
// a frame's capture and an agent's download adopted into the artifact directory, the artifact actions, the separate
// preview window, the recording ops, the agent's cursor and a profile's old cookies moved into its partition. Ported reference tests (T3 Code 1e2ecbd975, MIT, see LICENSE-T3), under their own
// names: Manager.test.ts `fitPictureInPictureContentSize` (2) and `recordingFileExtension` (1). The Manager tests of
// captureScreenshot's file name, revealArtifact and copyArtifactToClipboard are clone rows here (`testACapture…`,
// `testArtifactActions…`): the reference's mocks of Electron's shell and clipboard become the Finder (recorded in an agent
// run) and a private pasteboard.
final class BrowserCaptureReferenceTests: XCTestCase {
    func testPreservesThePiPContentAreaAcrossAspectRatioChanges() {
        let wide = T3BrowserPictureInPicture.fit(current: NSSize(width: 480, height: 320), aspect: 16 / 9)
        XCTAssertEqual([wide.width, wide.height], [523, 294])
        let tall = T3BrowserPictureInPicture.fit(current: NSSize(width: 480, height: 320), aspect: 9 / 16)
        XCTAssertEqual([tall.width, tall.height], [294, 523])
    }

    func testDoesNotCollapseTowardTheMinimumSizeWhenOrientationChangesRepeatedly() {
        let portrait = T3BrowserPictureInPicture.fit(current: NSSize(width: 523, height: 294), aspect: 9 / 16)
        let landscape = T3BrowserPictureInPicture.fit(current: NSSize(width: portrait.width, height: portrait.height), aspect: 16 / 9)
        XCTAssertEqual([portrait.width, portrait.height], [294, 523])
        XCTAssertEqual([landscape.width, landscape.height], [523, 294])
    }

    func testDerivesTheArtifactExtensionFromTheRecordersActualMimeType() {
        XCTAssertEqual(T3BrowserArtifacts.fileExtension("video/mp4;codecs=avc1.640028"), "mp4")
        XCTAssertEqual(T3BrowserArtifacts.fileExtension("video/webm;codecs=vp9"), "webm")
        XCTAssertEqual(T3BrowserArtifacts.fileExtension("video/x-matroska"), "matroska")
    }
}

final class BrowserCaptureTests: XCTestCase {
    private var window: NSWindow!
    private var web: WKWebView!
    private var fixture: CaptureFixture!
    private var pages: T3BrowserPages!
    private var artifacts: URL!
    private var temporary: URL!

    override func setUpWithError() throws {
        fixture = try CaptureFixture()
        pages = T3BrowserPages(agent: true)
        artifacts = CaptureSpin.outputDirectory("artifacts")
        temporary = CaptureSpin.outputDirectory("app-tmp")
        pages.artifactDirectoryOverride = artifacts
        pages.temporaryRoot = temporary
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 640, height: 360), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        web = WKWebView(frame: NSRect(x: 0, y: 0, width: 640, height: 360), configuration: configuration)
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        window.contentView = web
        window.orderFrontRegardless()
    }
    override func tearDown() { pages.destroy(); window.orderOut(nil) }

    /// The frame's page loaded and handed to the hatch as frame `id`, as the arm hands an `iframe hatch=`'s web view.
    private func page(_ id: String, _ path: String, _ html: String) {
        fixture.page(path, html)
        web.load(URLRequest(url: URL(string: fixture.base + path)!))
        CaptureSpin.until({ !web.isLoading && web.url != nil }, timeout: 10)
        CaptureSpin.wait(0.2)
        pages.attach(id, web)
    }
    /// One op, answered (the reply may come after a snapshot or the encoder).
    private func op(_ request: [String: Any]) -> [String: Any] {
        var answer: [String: Any]?
        XCTAssertTrue(pages.perform(request) { answer = $0 })
        CaptureSpin.until({ answer != nil }, timeout: 15)
        return answer ?? ["ok": false, "error": ["message": "timeout"]]
    }
    private func value(_ reply: [String: Any]) -> [String: Any] { reply["value"] as? [String: Any] ?? [:] }
    private func report(_ id: String) -> [String: Any] { ((pages.status["browserPages"] as? [String: Any])?[id] as? [String: Any]) ?? [:] }

    func testACaptureIsAdoptedAsAnArtifactNamedForTheSite() throws {
        let shots = temporary.appendingPathComponent("browser-shots", isDirectory: true)
        try FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        try Data([0x89, 0x50, 0x4e, 0x47]).write(to: shots.appendingPathComponent("frame-1.png"))
        let reply = op(["op": "browserArtifact", "action": "adopt", "path": "app:/tmp/browser-shots/frame-1.png", "url": "http://127.0.0.1:9/x", "generation": 4])
        XCTAssertEqual(reply["ok"] as? Bool, true, "\(reply)")
        XCTAssertEqual(reply["generation"] as? Int, 4)
        let artifact = value(reply), path = try XCTUnwrap(artifact["path"] as? String)
        // Manager.ts captureScreenshot: `<millis36>-<first eight characters of a v4 UUID>`, so a second capture in the
        // same millisecond never replaces the first (Manager.test.ts "screenshot filename collisions").
        XCTAssertNotNil((path as NSString).lastPathComponent.range(of: #"^browser-screenshot-127-0-0-1-[0-9a-z]+-[0-9a-f]{8}\.png$"#, options: .regularExpression), path)
        XCTAssertTrue(path.hasPrefix(artifacts.path + "/"))
        XCTAssertTrue(FileManager.default.fileExists(atPath: path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: shots.appendingPathComponent("frame-1.png").path), "moved, not copied")
        XCTAssertEqual(artifact["sizeBytes"] as? Int, 4)
        XCTAssertEqual(artifact["mimeType"] as? String, "image/png")
        XCTAssertNotNil(ISO8601DateFormatter().date(from: (artifact["createdAt"] as? String ?? "").replacingOccurrences(of: "\\.\\d+", with: "", options: .regularExpression)))
        for outside in ["app:/tmp/saves/x.png", "app:/tmp/browser-shots/../saves/x.png", "/etc/hosts"] {
            XCTAssertEqual((op(["op": "browserArtifact", "action": "adopt", "path": outside])["error"] as? [String: Any])?["kind"] as? String, "ArtifactPath", outside)
        }
        XCTAssertEqual(T3BrowserArtifacts.slug("https://Docs.Example.com:8443/x"), "docs-example-com")
        XCTAssertEqual(T3BrowserArtifacts.slug("about:blank"), "site")
        XCTAssertEqual(T3BrowserArtifacts.slug("https://" + String(repeating: "a.", count: 60) + "com").count, 79, "at most 80, a cut-off dash dropped")
    }

    func testAnAgentsFinishedDownloadIsAdoptedUnderItsOwnName() throws {
        let downloads = temporary.appendingPathComponent("browser-downloads", isDirectory: true)
        try FileManager.default.createDirectory(at: downloads, withIntermediateDirectories: true)
        try Data("a,b\n".utf8).write(to: downloads.appendingPathComponent("report.csv"))
        try Data("x".utf8).write(to: downloads.appendingPathComponent("notes.txt"))
        let reply = op(["op": "browserArtifact", "action": "adopt", "path": "app:/tmp/browser-downloads/report.csv"])
        let path = try XCTUnwrap(value(reply)["path"] as? String, "\(reply)")
        XCTAssertNotNil((path as NSString).lastPathComponent.range(of: #"^browser-download-[0-9a-z]+-[0-9a-f]{8}-report\.csv$"#, options: .regularExpression), path)
        XCTAssertEqual(try String(contentsOfFile: path, encoding: .utf8), "a,b\n")
        XCTAssertFalse(FileManager.default.fileExists(atPath: downloads.appendingPathComponent("report.csv").path), "moved, not copied")
        // The frame's record may name the file by its path in the app's temporary directory.
        let absolute = op(["op": "browserArtifact", "action": "adopt", "path": downloads.appendingPathComponent("notes.txt").path])
        XCTAssertTrue((value(absolute)["path"] as? String ?? "").hasSuffix("-notes.txt"), "\(absolute)")
        XCTAssertEqual((op(["op": "browserArtifact", "action": "adopt", "path": "app:/tmp/browser-downloads/../saves/x"])["error"] as? [String: Any])?["kind"] as? String, "ArtifactPath")
    }

    func testAProfilesOldCookiesMoveOnceIntoTheFramesPartitionAndTheOldStoreGoes() throws {
        guard #available(macOS 14.0, *) else { throw XCTSkip("per-identifier stores are macOS 14") }
        let environment = "test-\(UUID().uuidString)", identifier = T3BrowserPages.oldStoreIdentifier(environment: environment, profile: "default")
        let cookie = try XCTUnwrap(HTTPCookie(properties: [.name: "session", .value: "signed-in", .domain: "example.test", .path: "/", .expires: Date().addingTimeInterval(3600)]))
        var set = false
        autoreleasepool {
            let old = WKWebsiteDataStore(forIdentifier: identifier)
            old.httpCookieStore.setCookie(cookie) { set = true }
            CaptureSpin.until({ set })
        }
        let target = WKWebsiteDataStore.nonPersistent()
        var moved: Int?
        pages.migrateCookies(environment: environment, profile: "default", into: target) { moved = $0 }
        CaptureSpin.until({ moved != nil }, timeout: 15)
        XCTAssertEqual(moved, 1)
        var cookies: [HTTPCookie]?
        target.httpCookieStore.getAllCookies { cookies = $0 }
        CaptureSpin.until({ cookies != nil })
        XCTAssertEqual(cookies?.map { "\($0.name)=\($0.value)" }, ["session=signed-in"])
        var identifiers: [UUID]?
        WKWebsiteDataStore.fetchAllDataStoreIdentifiers { identifiers = $0 }
        CaptureSpin.until({ identifiers != nil })
        XCTAssertEqual(identifiers?.contains(identifier), false, "the old store is deleted")
        // Once per scope, and nothing for a profile that had no store.
        var again: Int?, none: Int?
        pages.migrateCookies(environment: environment, profile: "default", into: target) { again = $0 }
        pages.migrateCookies(environment: environment, profile: "work", into: target) { none = $0 }
        CaptureSpin.until({ again != nil && none != nil })
        XCTAssertEqual([again, none], [0, 0])
        XCTAssertEqual(T3BrowserPages.oldStoreIdentifier(environment: "env", profile: "default"), T3BrowserPages.oldStoreIdentifier(environment: "env", profile: "default"))
        XCTAssertNotEqual(T3BrowserPages.oldStoreIdentifier(environment: "env", profile: "default"), T3BrowserPages.oldStoreIdentifier(environment: "env", profile: "work"))
    }

    /// Reveal, Copy path and the copy Copy image reads (`stage`; the clipboard write is the data module's, through
    /// @exact/clipboard: browser-capture.ts).
    func testArtifactActionsOnlyTouchFilesInsideTheArtifactDirectory() throws {
        let inside = artifacts.appendingPathComponent("browser-screenshot-test.png")
        try Data("png".utf8).write(to: inside)
        let reveal = op(["op": "browserArtifact", "action": "reveal", "path": inside.path])
        XCTAssertEqual(value(reveal)["revealed"] as? String, inside.resolvingSymlinksInPath().path, "an agent run records the reveal instead of opening Finder")
        let pasteboard = T3BrowserArtifacts.pasteboard(agent: true)
        XCTAssertNotEqual(pasteboard.name, NSPasteboard.general.name, "an agent run never writes the person's clipboard")
        XCTAssertEqual(value(op(["op": "browserArtifact", "action": "copy-path", "path": inside.path]))["copied"] as? Bool, true)
        XCTAssertEqual(pasteboard.string(forType: .string), inside.resolvingSymlinksInPath().path)
        let staged = value(op(["op": "browserArtifact", "action": "stage", "path": inside.path]))["path"] as? String ?? ""
        XCTAssertTrue(staged.hasPrefix("app:/tmp/copy-image/artifact-") && staged.hasSuffix(".png"), staged)
        let copy = temporary.appendingPathComponent(String(staged.dropFirst("app:/tmp/".count)))
        XCTAssertEqual(try Data(contentsOf: copy), Data("png".utf8), "the artifact's bytes, under the app's files")
        for outside in ["/tmp/t3/dev/settings.json", artifacts.appendingPathComponent("../escape.png").path, "relative.png"] {
            for action in ["reveal", "stage"] {
                let refused = op(["op": "browserArtifact", "action": action, "path": outside])
                XCTAssertEqual(refused["ok"] as? Bool, false, "\(action) \(outside)")
                XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "ArtifactPath")
            }
        }
    }

    func testTheSeparateWindowFloatsTheLivePageAndFollowsItsAspect() throws {
        page("frame-pip", "/pip", "<!doctype html><title>Floating page</title><body style='margin:0;background:#0a0'><h1>pip</h1>")
        XCTAssertEqual(value(op(["op": "browserPip", "frame": "frame-pip", "action": "open"]))["open"] as? Bool, true)
        let pip = try XCTUnwrap(pages.pages["frame-pip"]?.pip)
        XCTAssertEqual(report("frame-pip")["pip"] as? Bool, true)
        XCTAssertEqual(pip.window.title, "Preview · Floating page")
        XCTAssertEqual(pip.window.level, .floating)
        XCTAssertTrue(pip.window.collectionBehavior.contains(.canJoinAllSpaces))
        XCTAssertEqual(pip.window.contentMinSize, NSSize(width: 240, height: 160))
        CaptureSpin.until({ pip.frames >= 3 }, timeout: 5)
        XCTAssertGreaterThanOrEqual(pip.frames, 3, "about 12 frames a second")
        let content = pip.window.contentRect(forFrameRect: pip.window.frame).size
        XCTAssertEqual(Double(content.width / content.height), 640.0 / 360.0, accuracy: 0.03, "the window follows the page's aspect ratio")
        XCTAssertEqual(value(op(["op": "browserPip", "frame": "frame-pip", "action": "open"]))["open"] as? Bool, true, "opening again shows the same window")
        XCTAssertTrue(pages.pages["frame-pip"]?.pip === pip)
        pip.window.performClose(nil)
        CaptureSpin.until({ pages.pages["frame-pip"]?.pip == nil }, timeout: 3)
        XCTAssertEqual(report("frame-pip")["pip"] as? Bool, false, "closing the window ends it")
        XCTAssertEqual(value(op(["op": "browserPip", "frame": "frame-pip", "action": "close"]))["open"] as? Bool, false)
    }

    func testTheRecordingOpsMakeAnArtifact() throws {
        page("frame-rec", "/rec", "<!doctype html><title>Rec</title><body style='margin:0'><h1 id=t>0</h1><script>let n=0;setInterval(()=>{document.getElementById('t').textContent=String(++n);document.body.style.background=n%2?'#f00':'#00f'},30)</script>")
        XCTAssertEqual(value(op(["op": "browserRecord", "frame": "frame-rec", "action": "arm", "showKeyPresses": true, "showMousePresses": true, "theme": ["primary": "#2563eb"]]))["armed"] as? Bool, true)
        XCTAssertEqual((report("frame-rec")["recording"] as? [String: Any])?["armed"] as? Bool, true)
        let stream = value(op(["op": "browserRecord", "frame": "frame-rec", "action": "capture", "frameRate": 30]))
        XCTAssertEqual(stream["frameRate"] as? Int, 30)
        XCTAssertGreaterThan(stream["width"] as? Int ?? 0, 0)
        XCTAssertEqual(value(op(["op": "browserRecord", "frame": "frame-rec", "action": "decorate", "showKeyPresses": true, "showMousePresses": true, "primaryColor": "#2563eb"]))["active"] as? Bool, true)
        XCTAssertEqual(value(op(["op": "browserRecord", "frame": "frame-rec", "action": "begin", "mimeType": "video/mp4;codecs=avc1", "bitsPerSecond": 2_500_000]))["mimeType"] as? String, "video/mp4;codecs=avc1")
        CaptureSpin.wait(1)
        XCTAssertEqual(value(op(["op": "browserRecord", "frame": "frame-rec", "action": "disarm"]))["armed"] as? Bool, false)
        let encoded = value(op(["op": "browserRecord", "frame": "frame-rec", "action": "finish"]))
        XCTAssertGreaterThan(encoded["frames"] as? Int ?? 0, 15, "\(encoded)")
        _ = op(["op": "browserRecord", "frame": "frame-rec", "action": "release"])
        let refused = op(["op": "browserRecord", "frame": "frame-rec", "action": "save", "mimeType": "video/mp4", "path": "/etc/hosts"])
        XCTAssertEqual(refused["ok"] as? Bool, false, "only the encoder's own file is saved")
        let saved = value(op(["op": "browserRecord", "frame": "frame-rec", "action": "save", "mimeType": encoded["mimeType"] as? String ?? "", "path": encoded["path"] as? String ?? ""]))
        let path = try XCTUnwrap(saved["path"] as? String)
        XCTAssertTrue(path.hasPrefix(artifacts.path + "/browser-recording-") && path.hasSuffix(".mp4"), path)
        XCTAssertFalse(FileManager.default.fileExists(atPath: encoded["path"] as? String ?? ""), "moved, not copied")
        let asset = AVURLAsset(url: URL(fileURLWithPath: path))
        var tracks: [AVAssetTrack] = [], done = false
        asset.loadTracks(withMediaType: .video) { loaded, _ in tracks = loaded ?? []; done = true }
        CaptureSpin.until({ done }, timeout: 5)
        XCTAssertEqual(tracks.count, 1)
        if let output = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"] { try? FileManager.default.copyItem(atPath: path, toPath: "\(output)/ops-recording.mp4") }
    }

    func testAnArmedRecordingIsCancelledAndAMissingFrameRefused() {
        page("frame-arm", "/idle", "<!doctype html><title>Idle</title>")
        _ = op(["op": "browserRecord", "frame": "frame-arm", "action": "arm"])
        _ = op(["op": "browserRecord", "frame": "frame-arm", "action": "cancel"])
        XCTAssertEqual((report("frame-arm")["recording"] as? [String: Any])?["armed"] as? Bool, false)
        let refused = op(["op": "browserRecord", "frame": "frame-missing", "action": "arm"])
        XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "NotFound")
        XCTAssertFalse(pages.perform(["op": "browserSync"]) { _ in }, "other ops pass through")
    }

    func testTheAgentsCursorIsALayerOfThePagesView() {
        page("frame-cursor", "/cursor", "<!doctype html><title>Cursor</title>")
        XCTAssertEqual(op(["op": "browserPage", "frame": "frame-cursor", "phase": "click", "x": 40, "y": 30, "sequence": 1, "controller": "agent"])["ok"] as? Bool, true)
        let cursor = web.layer?.sublayers?.first { $0.name == "t3-agent-cursor" }
        XCTAssertNotNil(cursor, "the arrow is a sublayer of the web view's own layer")
        XCTAssertEqual(cursor?.opacity ?? 0, 1, accuracy: 0.01, "active right after a move")
        XCTAssertEqual(report("frame-cursor")["size"] as? [Double], [640, 360], "the page's laid-out size, which the floating player reads")
    }
}
