import AppKit
import AVFoundation
import WebKit
import XCTest

// browser-surface part 3 (capture): screenshots, artifact actions, held pages, the separate preview window and the
// module's capture ops end to end (T3BrowserCapture.swift, T3BrowserSessions.performCapture). Ported reference tests
// (T3 Code 1e2ecbd975, MIT, see LICENSE-T3), under their own names: Manager.test.ts `fitPictureInPictureContentSize` (2)
// and `recordingFileExtension` (1). The Manager tests of captureScreenshot, revealArtifact, copyArtifactToClipboard and
// the download handler are clone rows here (`testTheScreenshot…`, `testArtifactActions…`): the reference's mocks of
// Electron's capturePage, shell and clipboard become a real page, the Finder (recorded in an agent run) and a private
// pasteboard.
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
    private var fixture: CaptureFixture!
    private var sessions: T3BrowserSessions!
    private var artifacts: URL!

    override func setUpWithError() throws {
        fixture = try CaptureFixture()
        sessions = T3BrowserSessions(agent: true, changed: { _ in })
        artifacts = CaptureSpin.outputDirectory("artifacts")
        sessions.artifactDirectoryOverride = artifacts
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 720, height: 560), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
    }
    override func tearDown() { sessions.sync([]); sessions.parking.close(); window.orderOut(nil) }

    private func mounted(_ id: String, url: String) -> (T3BrowserView, T3BrowserSession) {
        let view = T3BrowserView(props: ["tab": id, "url": url, "profile": "default", "environment": "env-1"], events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1), sessions: sessions)
        view.host.frame = window.contentView!.bounds
        window.contentView!.addSubview(view.host)
        return (view, sessions.sessions[id]!)
    }
    private func loaded(_ session: T3BrowserSession) { CaptureSpin.until({ session.report["kind"] as? String == "Success" }) }
    /// One capture op, answered (the reply may come after a snapshot or the encoder).
    private func op(_ request: [String: Any]) -> [String: Any] {
        var answer: [String: Any]?
        XCTAssertTrue(sessions.performCapture(request) { answer = $0 })
        CaptureSpin.until({ answer != nil }, timeout: 15)
        return answer ?? ["ok": false, "error": ["message": "timeout"]]
    }
    private func value(_ reply: [String: Any]) -> [String: Any] { reply["value"] as? [String: Any] ?? [:] }

    func testTheScreenshotIsAnArtifactNamedForTheSiteAtMost1280PixelsWide() throws {
        fixture.page("/wide", "<!doctype html><title>Wide</title><body style='margin:0;background:linear-gradient(90deg,#f00,#00f)'><h1>wide</h1>")
        let (view, session) = mounted("tab-shot", url: "\(fixture.base)/wide")
        view.host.frame = NSRect(x: 0, y: 0, width: 900, height: 500) // 1,800 pixels wide on a 2× display
        loaded(session)
        let reply = op(["op": "browserScreenshot", "tab": "tab-shot", "generation": 4])
        XCTAssertEqual(reply["ok"] as? Bool, true, "\(reply)")
        XCTAssertEqual(reply["generation"] as? Int, 4)
        let artifact = value(reply), path = try XCTUnwrap(artifact["path"] as? String)
        XCTAssertTrue(path.hasPrefix(artifacts.path + "/browser-screenshot-127-0-0-1-") && path.hasSuffix(".png"), path)
        XCTAssertEqual(artifact["mimeType"] as? String, "image/png")
        XCTAssertEqual(artifact["tabId"] as? String, "tab-shot")
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        XCTAssertEqual(artifact["sizeBytes"] as? Int, data.count)
        let rep = try XCTUnwrap(NSBitmapImageRep(data: data))
        XCTAssertLessThanOrEqual(rep.pixelsWide, 1280, "MAX_SCREENSHOT_WIDTH")
        XCTAssertEqual(Double(rep.pixelsWide) / Double(rep.pixelsHigh), 900.0 / 500.0, accuracy: 0.02)
        XCTAssertNotNil(ISO8601DateFormatter().date(from: (artifact["createdAt"] as? String ?? "").replacingOccurrences(of: "\\.\\d+", with: "", options: .regularExpression)))
        XCTAssertEqual(T3BrowserArtifacts.slug("https://Docs.Example.com:8443/x"), "docs-example-com")
        XCTAssertEqual(T3BrowserArtifacts.slug("about:blank"), "site")
        XCTAssertEqual(T3BrowserArtifacts.slug("https://" + String(repeating: "a.", count: 60) + "com").count, 79, "at most 80, a cut-off dash dropped")
        view.destroy()
    }

    func testArtifactActionsOnlyTouchFilesInsideTheArtifactDirectory() throws {
        let inside = artifacts.appendingPathComponent("browser-screenshot-test.png")
        let image = NSImage(size: NSSize(width: 4, height: 4)); image.lockFocus(); NSColor.red.setFill(); NSRect(x: 0, y: 0, width: 4, height: 4).fill(); image.unlockFocus()
        try XCTUnwrap(T3BrowserArtifacts.png(image)).data.write(to: inside)
        let reveal = op(["op": "browserArtifact", "action": "reveal", "path": inside.path])
        XCTAssertEqual(value(reveal)["revealed"] as? String, inside.resolvingSymlinksInPath().path, "an agent run records the reveal instead of opening Finder")
        XCTAssertTrue(sessions.log.contains { $0.hasPrefix("artifact reveal browser-screenshot-test.png") })
        let pasteboard = T3BrowserArtifacts.pasteboard(agent: true)
        XCTAssertNotEqual(pasteboard.name, NSPasteboard.general.name, "an agent run never writes the person's clipboard")
        XCTAssertEqual(value(op(["op": "browserArtifact", "action": "copy-image", "path": inside.path]))["copied"] as? Bool, true)
        XCTAssertNotNil(pasteboard.data(forType: .png))
        XCTAssertEqual(value(op(["op": "browserArtifact", "action": "copy-path", "path": inside.path]))["copied"] as? Bool, true)
        XCTAssertEqual(pasteboard.string(forType: .string), inside.resolvingSymlinksInPath().path)
        for outside in ["/tmp/t3/dev/settings.json", artifacts.appendingPathComponent("../escape.png").path, "relative.png"] {
            let refused = op(["op": "browserArtifact", "action": "reveal", "path": outside])
            XCTAssertEqual(refused["ok"] as? Bool, false, outside)
            XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "ArtifactPath")
        }
        try Data("x".utf8).write(to: artifacts.appendingPathComponent("broken.png"))
        XCTAssertEqual((op(["op": "browserArtifact", "action": "copy-image", "path": artifacts.appendingPathComponent("broken.png").path])["error"] as? [String: Any])?["kind"] as? String, "ArtifactImage")
    }

    func testAHeldPageKeepsPaintingOffScreenAndComesBackToItsView() throws {
        fixture.page("/tick", "<!doctype html><title>Tick</title><h1 id=t>0</h1><script>let n=0;setInterval(()=>{document.getElementById('t').textContent=String(++n)},16)</script>")
        let (view, session) = mounted("tab-held", url: "\(fixture.base)/tick")
        loaded(session)
        sessions.parking.hold(session, "recording")
        XCTAssertTrue(session.web.window === window, "a page a view shows is not parked")
        view.destroy()
        XCTAssertNotNil(session.web.window, "a held page that loses its view moves to the host window")
        XCTAssertNotEqual(session.web.window, window)
        XCTAssertTrue(sessions.parking.parked.contains("tab-held"))
        let first = Int(CaptureSpin.evaluate(session.web, "document.getElementById('t').textContent")) ?? 0
        CaptureSpin.wait(0.5)
        let later = Int(CaptureSpin.evaluate(session.web, "document.getElementById('t').textContent")) ?? 0
        XCTAssertGreaterThan(later - first, 10, "the parked page keeps its timers running (\(first) → \(later))")
        let (again, _) = mounted("tab-held", url: "")
        XCTAssertTrue(session.web.window === window, "a view takes the parked page back")
        XCTAssertFalse(sessions.parking.parked.contains("tab-held"))
        again.destroy()
        XCTAssertNotNil(session.web.window, "still held: parked again")
        sessions.parking.release(session, "recording")
        XCTAssertNil(session.web.window, "released: no window, as before part 3")
        XCTAssertFalse(sessions.parking.parked.contains("tab-held"))
    }

    func testTheSeparateWindowFloatsTheLivePageAndFollowsItsAspect() throws {
        fixture.page("/pip", "<!doctype html><title>Floating page</title><body style='margin:0;background:#0a0'><h1>pip</h1>")
        let (view, session) = mounted("tab-pip", url: "\(fixture.base)/pip")
        view.host.frame = NSRect(x: 0, y: 0, width: 640, height: 360)
        loaded(session)
        XCTAssertEqual(value(op(["op": "browserPip", "tab": "tab-pip", "action": "open"]))["open"] as? Bool, true)
        let pip = try XCTUnwrap(session.pip)
        XCTAssertEqual(session.report["pip"] as? Bool, true)
        XCTAssertEqual(pip.window.title, "Preview · Floating page")
        XCTAssertEqual(pip.window.level, .floating)
        XCTAssertTrue(pip.window.collectionBehavior.contains(.canJoinAllSpaces))
        XCTAssertEqual(pip.window.contentMinSize, NSSize(width: 240, height: 160))
        XCTAssertTrue(pip.window.standardWindowButton(.miniaturizeButton)?.isHidden ?? true)
        CaptureSpin.until({ pip.frames >= 3 }, timeout: 5)
        XCTAssertGreaterThanOrEqual(pip.frames, 3, "about 12 frames a second")
        let content = pip.window.contentRect(forFrameRect: pip.window.frame).size
        XCTAssertEqual(Double(content.width / content.height), 640.0 / 360.0, accuracy: 0.03, "the window follows the page's aspect ratio")
        view.destroy()
        XCTAssertTrue(sessions.parking.parked.contains("tab-pip"), "the floating page keeps painting while the panel is hidden")
        let before = pip.frames
        CaptureSpin.wait(0.5)
        XCTAssertGreaterThan(pip.frames, before)
        XCTAssertEqual(value(op(["op": "browserPip", "tab": "tab-pip", "action": "open"]))["open"] as? Bool, true, "opening again shows the same window")
        XCTAssertTrue(session.pip === pip)
        pip.window.performClose(nil)
        CaptureSpin.until({ session.pip == nil }, timeout: 3)
        XCTAssertNil(session.pip, "closing the window ends it")
        XCTAssertEqual(session.report["pip"] as? Bool, false)
        XCTAssertFalse(sessions.parking.parked.contains("tab-pip"))
        XCTAssertEqual(value(op(["op": "browserPip", "tab": "tab-pip", "action": "close"]))["open"] as? Bool, false)
    }

    func testTheRecordingOpsMakeAnArtifactAndTheLeaseLetsGo() throws {
        fixture.page("/rec", "<!doctype html><title>Rec</title><body style='margin:0'><h1 id=t>0</h1><script>let n=0;setInterval(()=>{document.getElementById('t').textContent=String(++n);document.body.style.background=n%2?'#f00':'#00f'},30)</script>")
        let (view, session) = mounted("tab-rec", url: "\(fixture.base)/rec")
        view.host.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        loaded(session)
        XCTAssertEqual(value(op(["op": "browserRecord", "tab": "tab-rec", "action": "arm", "showKeyPresses": true, "showMousePresses": true, "theme": ["primary": "#2563eb"]]))["armed"] as? Bool, true)
        XCTAssertEqual((session.report["recording"] as? [String: Any])?["armed"] as? Bool, true)
        let stream = value(op(["op": "browserRecord", "tab": "tab-rec", "action": "capture", "frameRate": 30]))
        XCTAssertEqual(stream["frameRate"] as? Int, 30)
        XCTAssertGreaterThan(stream["width"] as? Int ?? 0, 0)
        XCTAssertEqual(value(op(["op": "browserRecord", "tab": "tab-rec", "action": "decorate", "showKeyPresses": true, "showMousePresses": true, "primaryColor": "#2563eb"]))["active"] as? Bool, true)
        XCTAssertEqual(value(op(["op": "browserRecord", "tab": "tab-rec", "action": "begin", "mimeType": "video/mp4;codecs=avc1", "bitsPerSecond": 2_500_000]))["mimeType"] as? String, "video/mp4;codecs=avc1")
        view.destroy() // the panel hides mid-recording: the page is held off screen
        XCTAssertTrue(sessions.parking.parked.contains("tab-rec"))
        CaptureSpin.wait(1)
        XCTAssertEqual(value(op(["op": "browserRecord", "tab": "tab-rec", "action": "disarm"]))["armed"] as? Bool, false)
        let encoded = value(op(["op": "browserRecord", "tab": "tab-rec", "action": "finish"]))
        XCTAssertGreaterThan(encoded["frames"] as? Int ?? 0, 15, "\(encoded)")
        _ = op(["op": "browserRecord", "tab": "tab-rec", "action": "release"])
        XCTAssertFalse(sessions.parking.parked.contains("tab-rec"), "the lease lets go with the stream")
        let refused = op(["op": "browserRecord", "tab": "tab-rec", "action": "save", "mimeType": "video/mp4", "path": "/etc/hosts"])
        XCTAssertEqual(refused["ok"] as? Bool, false, "only the encoder's own file is saved")
        let saved = value(op(["op": "browserRecord", "tab": "tab-rec", "action": "save", "mimeType": encoded["mimeType"] as? String ?? "", "path": encoded["path"] as? String ?? ""]))
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

    func testAnArmedRecordingThatNeverBeginsIsCancelled() {
        fixture.page("/idle", "<!doctype html><title>Idle</title>")
        let (view, session) = mounted("tab-arm", url: "\(fixture.base)/idle")
        loaded(session)
        _ = op(["op": "browserRecord", "tab": "tab-arm", "action": "arm"])
        _ = op(["op": "browserRecord", "tab": "tab-arm", "action": "cancel"])
        XCTAssertEqual((session.report["recording"] as? [String: Any])?["armed"] as? Bool, false)
        let refused = op(["op": "browserRecord", "tab": "tab-missing", "action": "arm"])
        XCTAssertEqual((refused["error"] as? [String: Any])?["kind"] as? String, "NotFound")
        XCTAssertFalse(sessions.performCapture(["op": "browserSync"]) { _ in }, "other ops pass through")
        view.destroy()
    }
}
