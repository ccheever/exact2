import AppKit
import AVFoundation
import WebKit
import XCTest

// browser-surface part 3 (capture): the recording (T3BrowserRecorder.swift, assets/browser-recording.js).
// Ported reference tests (T3 Code 1e2ecbd975, MIT, see LICENSE-T3), under their own names:
// apps/web/src/browser/recordingCompositor.test.ts (7: "recording decorations" 5, "detached recording
// compositor" 2, the it.each rows as …False / …True). Substitutions: the canvas context is a T3RecordingCanvas
// mock; the detached compositor is `useDecorations` (no compositor, no input subscription with both off) and its
// release is the recording's `cancel` (disposal) or a recorder that cannot start (the playback failure). The
// BrowserRecordingTests rows are the clone's own, against a loopback page: the in-page cursor, the snapshot
// stream, the MP4 the encoder writes, the inputs, a navigation and pop-ups.

/// A canvas recording the calls RecordingDecorations makes (the reference test's `context()`).
final class MockRecordingCanvas: T3RecordingCanvas {
    var ellipses: [[Double]] = []
    var texts: [(String, Double, Double, Double)] = []
    var fills = 0, strokes = 0, roundRects = 0, saves = 0, restores = 0
    var globalAlpha: Double = 1
    var strokeStyle = ""
    var fillStyle = ""
    var lineWidth: Double = 1
    var font = ""
    var textAlign = ""
    var textBaseline = ""
    func save() { saves += 1 }
    func restore() { restores += 1 }
    func beginPath() {}
    func ellipse(x: Double, y: Double, rx: Double, ry: Double) { ellipses.append([x, y, rx, ry]) }
    func fill() { fills += 1 }
    func stroke() { strokes += 1 }
    func roundRect(x: Double, y: Double, width: Double, height: Double, radius: Double) { roundRects += 1 }
    func fillText(_ text: String, x: Double, y: Double, maxWidth: Double) { texts.append((text, x, y, maxWidth)) }
    func measureText(_ text: String) -> Double { 40 }
}

private let primaryColor = "oklch(0.65 0.2 310)"
private func pointer(_ phase: String, _ x: Double = 100) -> [String: Any] { ["type": "pointer", "phase": phase, "x": x, "y": 80, "width": 800, "height": 600] }

/// A small page in a window, its web view sharing a configuration the recording registers on.
private final class RecordingPage {
    let window: NSWindow
    let web: WKWebView
    let recording: T3BrowserRecording
    let configuration = WKWebViewConfiguration()
    init(width: CGFloat = 640, height: CGFloat = 400) {
        configuration.websiteDataStore = .nonPersistent()
        recording = T3BrowserRecording(configuration: configuration)
        web = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: height), configuration: configuration)
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        window = NSWindow(contentRect: NSRect(x: 60, y: 60, width: width, height: height), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = web
        window.orderFrontRegardless()
        recording.page = web
    }
    func load(_ url: String) {
        let done = Loaded()
        web.navigationDelegate = done
        web.load(URLRequest(url: URL(string: url)!))
        CaptureSpin.until({ done.finished }, timeout: 10)
        web.navigationDelegate = nil
    }
    func loadHTML(_ html: String) {
        let done = Loaded()
        web.navigationDelegate = done
        web.loadHTMLString(html, baseURL: URL(string: "http://localhost/"))
        CaptureSpin.until({ done.finished }, timeout: 10)
        web.navigationDelegate = nil
    }
    func close() { recording.close(); window.orderOut(nil) }
    final class Loaded: NSObject, WKNavigationDelegate {
        var finished = false
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { finished = true }
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { finished = true }
        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { finished = true }
    }
}

private func arm(_ page: RecordingPage, keys: Bool = true, mouse: Bool = true) -> Error? {
    var result: Error?? = .none
    page.recording.arm(showKeyPresses: keys, showMousePresses: mouse, theme: ["primary": "#7c3aed", "background": "white"], controller: "none") { result = .some($0) }
    CaptureSpin.until({ result != nil }, timeout: 10)
    return result ?? T3BrowserRecordingError("arm timed out")
}

private func capture(_ page: RecordingPage, fps: Int) -> [String: Any]? {
    var result: Result<[String: Any], Error>?
    page.recording.capture(frameRate: fps) { result = $0 }
    CaptureSpin.until({ result != nil }, timeout: 10)
    if case .success(let value)? = result { return value }
    return nil
}

private func finish(_ page: RecordingPage) -> Result<[String: Any], Error>? {
    var result: Result<[String: Any], Error>?
    page.recording.finish { result = $0 }
    CaptureSpin.until({ result != nil }, timeout: 15)
    return result
}

final class RecordingDecorationsTests: XCTestCase {
    private let options = (keys: true, mouse: true, fps: 30)
    private func decorations(keys: Bool = true, mouse: Bool = true) -> T3RecordingDecorations {
        T3RecordingDecorations(showKeyPresses: keys, showMousePresses: mouse, frameRate: 30, primaryColor: primaryColor)
    }

    // describe("recording decorations")
    func testKeepsRingsAlignedThroughDraggingAndStopsFollowingTheCursorAfterRelease() {
        let decorations = decorations(), ctx = MockRecordingCanvas()
        decorations.apply(pointer("down"), now: 0)
        decorations.apply(pointer("move", 120), now: 10)
        decorations.draw(ctx, width: 1600, height: 1200, now: 10)
        XCTAssertEqual(ctx.ellipses.first, [240, 160, 40, 40])
        XCTAssertEqual(ctx.strokeStyle, primaryColor)
        XCTAssertEqual(ctx.fillStyle, primaryColor)
        XCTAssertNil(decorations.nextRedraw(now: 10))
        decorations.apply(pointer("up", 130), now: 20)
        decorations.apply(pointer("move", 300), now: 30)
        decorations.draw(ctx, width: 1600, height: 1200, now: 320)
        XCTAssertEqual(ctx.ellipses.count > 1 ? ctx.ellipses[1] : [], [260, 160, 50, 50])
        ctx.ellipses.removeAll()
        decorations.draw(ctx, width: 1600, height: 1200, now: 620)
        XCTAssertTrue(ctx.ellipses.isEmpty)
        XCTAssertNil(decorations.nextRedraw(now: 620))
    }

    func testPulsesAgentClicksAndClearsDecorationsOnBlurOrNavigation() {
        let decorations = decorations(), ctx = MockRecordingCanvas()
        decorations.apply(pointer("click"), now: 0)
        decorations.draw(ctx, width: 800, height: 600, now: 300)
        XCTAssertEqual(ctx.ellipses.count, 1)
        decorations.apply(["type": "clear"], now: 301)
        ctx.ellipses.removeAll()
        decorations.draw(ctx, width: 800, height: 600, now: 302)
        XCTAssertTrue(ctx.ellipses.isEmpty)
        XCTAssertNil(decorations.nextRedraw(now: 302))
    }

    func testHoldsShortcutBadgesUntilReleaseThenExpiresThemEvenOnAStaticPage() {
        let decorations = decorations(), ctx = MockRecordingCanvas()
        let key: [String: Any] = ["type": "key", "label": "⌘C", "held": true, "width": 800]
        decorations.apply(key, now: 0)
        decorations.draw(ctx, width: 1600, height: 1200, now: 5000)
        XCTAssertEqual(ctx.texts.first.map { [$0.1, $0.2] }, [800, 1098])
        XCTAssertEqual(ctx.texts.first?.0, "⌘C")
        var released = key; released["held"] = false
        decorations.apply(released, now: 5000)
        XCTAssertEqual(decorations.nextRedraw(now: 5500), 400)
        ctx.texts.removeAll()
        decorations.draw(ctx, width: 1600, height: 1200, now: 5900)
        XCTAssertTrue(ctx.texts.isEmpty)
    }

    func testRemovesThePreviousKeyBadgeOnPasswordFocus() {
        let decorations = decorations(), ctx = MockRecordingCanvas()
        decorations.apply(["type": "key", "label": "A", "held": true, "width": 800], now: 0)
        decorations.apply(["type": "key", "label": NSNull(), "held": true, "width": 800], now: 1)
        decorations.draw(ctx, width: 800, height: 600, now: 2)
        XCTAssertTrue(ctx.texts.isEmpty)
    }

    func testHonorsIndependentOptInFlags() {
        let decorations = decorations(mouse: false), ctx = MockRecordingCanvas()
        decorations.apply(pointer("down"), now: 0)
        decorations.apply(["type": "key", "label": "⌘C", "held": true, "width": 800], now: 0)
        decorations.draw(ctx, width: 800, height: 600, now: 1)
        XCTAssertTrue(ctx.ellipses.isEmpty)
        XCTAssertEqual(ctx.texts.count, 1)
    }

    // describe("detached recording compositor")
    func testKeepsNativeCaptureWhenBothDecorationsAreOff() {
        let configuration = WKWebViewConfiguration()
        let recording = T3BrowserRecording(configuration: configuration)
        XCTAssertFalse(recording.useDecorations(showKeyPresses: false, showMousePresses: false, primaryColor: primaryColor))
        XCTAssertNil(recording.decorations, "no compositor is allocated")
        recording.close()
    }

    /// it.each([false, true]): released on disposal (cancel, twice) or when the recorder fails to start.
    private func releasesTheDetachedOutput(failPlayback: Bool) throws {
        let page = RecordingPage(width: 320, height: 200)
        defer { page.close() }
        page.loadHTML("<!doctype html><body style='background:#2a6'>frame</body>")
        if failPlayback {
            XCTAssertTrue(page.recording.useDecorations(showKeyPresses: true, showMousePresses: true, primaryColor: primaryColor))
            XCTAssertThrowsError(try page.recording.begin(mimeType: nil, bitsPerSecond: 2_500_000), "no stream: the recorder cannot start")
        } else {
            XCTAssertNotNil(capture(page, fps: 30))
            XCTAssertTrue(page.recording.useDecorations(showKeyPresses: true, showMousePresses: true, primaryColor: primaryColor))
            XCTAssertNotNil(page.recording.decorations)
            _ = try page.recording.begin(mimeType: nil, bitsPerSecond: 2_500_000)
            CaptureSpin.wait(0.2)
            page.recording.cancel()
            page.recording.cancel()
        }
        XCTAssertNil(page.recording.decorations, "the compositor is released")
        XCTAssertEqual(page.recording.report["recording"] as? Bool, false)
        XCTAssertEqual(page.recording.report["capturing"] as? Bool, false)
    }
    func testReleasesTheDetachedOutputOnDisposalOrPlaybackFailureFalse() throws { try releasesTheDetachedOutput(failPlayback: false) }
    func testReleasesTheDetachedOutputOnDisposalOrPlaybackFailureTrue() throws { try releasesTheDetachedOutput(failPlayback: true) }
}

final class BrowserRecordingTests: XCTestCase {
    private var fixture: CaptureFixture!
    private var page: RecordingPage!

    override func setUpWithError() throws {
        fixture = try CaptureFixture()
        fixture.page("/static", "<!doctype html><title>Static</title><body style='margin:0;background:#fff'><input id=t><input id=p type=password></body>")
        fixture.page("/next", "<!doctype html><title>Next</title><body>next</body>")
        fixture.page("/anim", "<!doctype html><title>Anim</title><body style='margin:0'><div id=b style='width:100vw;height:100vh'></div><script>let n=0;const f=()=>{document.getElementById('b').style.background='hsl('+(n++%360)+',80%,50%)';requestAnimationFrame(f)};f()</script></body>")
        page = RecordingPage(width: 640, height: 400)
    }
    override func tearDown() { page.close() }

    private func cursorPresent() -> Bool {
        CaptureSpin.evaluate(page.web, "String(document.querySelector('[data-t3code-recording-cursor]') !== null && document.querySelector('[data-t3code-recording-agent-cursor]') !== null)") == "true"
    }
    private func outputCopy(_ path: String, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"] else { return }
        let target = URL(fileURLWithPath: dir).appendingPathComponent(name)
        try? FileManager.default.removeItem(at: target)
        try? FileManager.default.copyItem(at: URL(fileURLWithPath: path), to: target)
    }
    private func videoTrack(_ path: String) -> (size: CGSize, seconds: Double, samples: Int)? {
        var out: (CGSize, Double, Int)?
        var done = false
        Task {
            defer { done = true }
            let asset = AVURLAsset(url: URL(fileURLWithPath: path))
            guard let track = try? await asset.loadTracks(withMediaType: .video).first,
                  let size = try? await track.load(.naturalSize), let duration = try? await asset.load(.duration),
                  let reader = try? AVAssetReader(asset: asset) else { return }
            let output = AVAssetReaderTrackOutput(track: track, outputSettings: nil)
            reader.add(output)
            reader.startReading()
            var samples = 0
            while let sample = output.copyNextSampleBuffer() { if CMSampleBufferGetNumSamples(sample) > 0 { samples += 1 } }
            out = (size, duration.seconds, samples)
        }
        CaptureSpin.until({ done }, timeout: 15)
        return out.map { (size: $0.0, seconds: $0.1, samples: $0.2) }
    }

    func testArmInstallsTheDrawnCursorAndDisarmRemovesIt() throws {
        page.load("\(fixture.base)/static")
        XCTAssertFalse(cursorPresent())
        XCTAssertNil(arm(page))
        XCTAssertTrue(cursorPresent(), "the drawn human and agent cursors are in the page")
        XCTAssertEqual(CaptureSpin.evaluate(page.web, "typeof globalThis.__t3codeRecording"), "undefined", "the script lives in the module's own content world")
        XCTAssertEqual(CaptureSpin.evaluate(page.web, "typeof globalThis.__t3codeRecording", world: .defaultClient), "object")
        XCTAssertEqual(CaptureSpin.evaluate(page.web, "getComputedStyle(document.body).cursor"), "none", "the native cursor is hidden while recording")
        XCTAssertEqual(page.recording.report["armed"] as? Bool, true)
        page.recording.disarm()
        CaptureSpin.until({ !self.cursorPresent() }, timeout: 3)
        XCTAssertFalse(cursorPresent())
        XCTAssertNotEqual(CaptureSpin.evaluate(page.web, "getComputedStyle(document.body).cursor"), "none")
    }

    func testCaptureReportsTheSnapshotPixelSizeAtItsFrameRate() throws {
        page.load("\(fixture.base)/static")
        let stream = try XCTUnwrap(capture(page, fps: 30))
        let scale = page.window.backingScaleFactor
        XCTAssertEqual(stream["width"] as? Int, Int(640 * scale))
        XCTAssertEqual(stream["height"] as? Int, Int(400 * scale))
        XCTAssertEqual(stream["frameRate"] as? Int, 30)
        XCTAssertEqual(page.recording.report["capturing"] as? Bool, true)
        page.recording.releaseCapture()
        XCTAssertEqual(page.recording.report["capturing"] as? Bool, false)
    }

    func testBeginAndFinishWriteAnMP4OfTheStreamSizeAndDuration() throws {
        page.load("\(fixture.base)/anim")
        XCTAssertNil(arm(page, keys: false, mouse: false))
        let stream = try XCTUnwrap(capture(page, fps: 30))
        XCTAssertFalse(page.recording.useDecorations(showKeyPresses: false, showMousePresses: false, primaryColor: "#2563eb"))
        XCTAssertFalse(T3BrowserRecording.supports("video/webm;codecs=vp9"))
        XCTAssertThrowsError(try page.recording.begin(mimeType: "video/webm", bitsPerSecond: 2_500_000))
        let started = Date()
        XCTAssertEqual(try page.recording.begin(mimeType: "video/mp4;codecs=avc1", bitsPerSecond: 3_110_400), "video/mp4;codecs=avc1")
        CaptureSpin.wait(1.5)
        let result = try XCTUnwrap(finish(page)).get()
        let elapsed = Date().timeIntervalSince(started)
        let path = try XCTUnwrap(result["path"] as? String)
        defer { try? FileManager.default.removeItem(atPath: path) }
        outputCopy(path, "recording-30fps.mp4")
        XCTAssertEqual(result["mimeType"] as? String, "video/mp4;codecs=avc1")
        XCTAssertGreaterThan(result["sizeBytes"] as? Int ?? 0, 1000)
        let track = try XCTUnwrap(videoTrack(path))
        XCTAssertEqual(Int(track.size.width), stream["width"] as? Int)
        XCTAssertEqual(Int(track.size.height), stream["height"] as? Int)
        XCTAssertEqual(track.seconds, elapsed, accuracy: 0.35, "the file lasts until the stop")
        XCTAssertEqual(track.samples, result["frames"] as? Int)
        print("recording 30 fps: \(result["frames"] ?? 0) frames in \(String(format: "%.2f", elapsed)) s, \(track.size), \(String(format: "%.2f", track.seconds)) s")
        page.recording.releaseCapture()
        page.recording.disarm()
    }

    func testSixtyFramesASecondWritesMoreFramesThanThirty() throws {
        page.load("\(fixture.base)/anim")
        var counts: [Int: Int] = [:]
        for fps in [30, 60] {
            XCTAssertNotNil(capture(page, fps: fps))
            _ = try page.recording.begin(mimeType: nil, bitsPerSecond: 2_764_800)
            CaptureSpin.wait(2)
            let result = try XCTUnwrap(finish(page)).get()
            counts[fps] = result["frames"] as? Int ?? 0
            if let path = result["path"] as? String { outputCopy(path, "recording-\(fps)fps-anim.mp4"); try? FileManager.default.removeItem(atPath: path) }
            page.recording.releaseCapture()
        }
        print("recording frames over 2 s: 30 fps \(counts[30] ?? 0), 60 fps \(counts[60] ?? 0)")
        XCTAssertGreaterThan(counts[30] ?? 0, 40)
        XCTAssertGreaterThan(Double(counts[60] ?? 0), Double(counts[30] ?? 0) * 1.4)
    }

    func testKeyPressesReachTheDecorationsAndAPasswordFieldGivesNoLabel() throws {
        page.load("\(fixture.base)/static")
        XCTAssertNil(arm(page, keys: true, mouse: true))
        XCTAssertNotNil(capture(page, fps: 30))
        XCTAssertTrue(page.recording.useDecorations(showKeyPresses: true, showMousePresses: true, primaryColor: "#7c3aed"))
        _ = try page.recording.begin(mimeType: nil, bitsPerSecond: 2_500_000)
        let press = { (target: String, key: String, meta: Bool) in
            _ = CaptureSpin.evaluate(self.page.web, "document.getElementById('\(target)').focus(); document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key: '\(key)', metaKey: \(meta), bubbles: true})); 'ok'")
        }
        press("t", "c", true)
        CaptureSpin.until({ (self.page.recording.report["inputs"] as? Int ?? 0) >= 1 }, timeout: 3)
        let ctx = MockRecordingCanvas()
        page.recording.decorations?.draw(ctx, width: 1280, height: 800, now: T3BrowserRecording.now())
        XCTAssertEqual(ctx.texts.first?.0, "⌘C", "the shortcut is labelled as on a Mac")
        press("p", "s", false)
        CaptureSpin.until({ (self.page.recording.report["inputs"] as? Int ?? 0) >= 2 }, timeout: 3)
        let after = MockRecordingCanvas()
        page.recording.decorations?.draw(after, width: 1280, height: 800, now: T3BrowserRecording.now())
        XCTAssertTrue(after.texts.isEmpty, "a password field's key clears the badge and shows nothing")
        _ = CaptureSpin.evaluate(page.web, "window.dispatchEvent(new PointerEvent('pointerdown', {clientX: 40, clientY: 30, pointerType: 'mouse'})); 'ok'")
        CaptureSpin.until({ (self.page.recording.report["inputs"] as? Int ?? 0) >= 3 }, timeout: 3)
        let ring = MockRecordingCanvas()
        page.recording.decorations?.draw(ring, width: 1280, height: 800, now: T3BrowserRecording.now())
        XCTAssertEqual(ring.ellipses.first?[0] ?? 0, 80, accuracy: 0.5, "the press ring is placed in frame pixels")
        press("t", "c", true) // the badge again, held to the end: the file's last frames show it and the ring
        CaptureSpin.wait(0.4)
        let result = try XCTUnwrap(finish(page)).get()
        if let path = result["path"] as? String { outputCopy(path, "recording-decorated.mp4"); try? FileManager.default.removeItem(atPath: path) }
        page.recording.cancel()
    }

    func testANavigationWhileArmedReinstallsTheCursorWhenTheDocumentIsReady() throws {
        page.load("\(fixture.base)/static")
        XCTAssertNil(arm(page))
        XCTAssertTrue(cursorPresent())
        page.load("\(fixture.base)/next")
        XCTAssertFalse(cursorPresent(), "a new document has no cursor")
        page.recording.documentReady()
        CaptureSpin.until({ self.cursorPresent() }, timeout: 5)
        XCTAssertTrue(cursorPresent())
        page.recording.disarm()
        page.recording.documentReady()
        CaptureSpin.wait(0.3)
        XCTAssertFalse(cursorPresent(), "disarmed, a load installs nothing")
    }

    func testCancelRemovesTheTemporaryFile() throws {
        page.load("\(fixture.base)/static")
        XCTAssertNotNil(capture(page, fps: 30))
        _ = try page.recording.begin(mimeType: nil, bitsPerSecond: 2_500_000)
        CaptureSpin.wait(0.3)
        let temporary = try XCTUnwrap(page.recording.temporaryURL)
        CaptureSpin.until({ FileManager.default.fileExists(atPath: temporary.path) }, timeout: 3)
        XCTAssertTrue(FileManager.default.fileExists(atPath: temporary.path), "the encoder writes a temporary file")
        page.recording.cancel()
        CaptureSpin.until({ !FileManager.default.fileExists(atPath: temporary.path) }, timeout: 3)
        XCTAssertFalse(FileManager.default.fileExists(atPath: temporary.path), "cancel removes it")
        XCTAssertNil(page.recording.temporaryURL)
        if case .failure? = finish(page) {} else { XCTFail("nothing left to finish") }
    }

    func testAPopupsMessagesAreIgnored() throws {
        page.load("\(fixture.base)/static")
        XCTAssertNil(arm(page))
        XCTAssertNotNil(capture(page, fps: 30))
        XCTAssertTrue(page.recording.useDecorations(showKeyPresses: true, showMousePresses: true, primaryColor: "#7c3aed"))
        // A pop-up shares the tab's configuration (and so the handler), as part 1's pop-ups do.
        let popup = WKWebView(frame: NSRect(x: 0, y: 0, width: 200, height: 200), configuration: page.configuration)
        let loaded = RecordingPage.Loaded()
        popup.navigationDelegate = loaded
        popup.loadHTMLString("<title>Popup</title>", baseURL: nil)
        CaptureSpin.until({ loaded.finished })
        _ = CaptureSpin.evaluate(popup, "window.webkit.messageHandlers.t3BrowserRecording.postMessage({type: 'key', label: 'X', held: true, width: 800}); 'ok'", world: .defaultClient)
        CaptureSpin.wait(0.3)
        XCTAssertEqual(page.recording.report["inputs"] as? Int, 0, "the pop-up's message is not the tab's")
        _ = CaptureSpin.evaluate(page.web, "window.webkit.messageHandlers.t3BrowserRecording.postMessage({type: 'key', label: 'X', held: true, width: 800}); 'ok'", world: .defaultClient)
        CaptureSpin.until({ (self.page.recording.report["inputs"] as? Int ?? 0) == 1 }, timeout: 3)
        XCTAssertEqual(page.recording.report["inputs"] as? Int, 1)
        _ = CaptureSpin.evaluate(page.web, "window.webkit.messageHandlers.t3BrowserRecording.postMessage({type: 'pointer', phase: 'warp', x: 1, y: 1, width: 1, height: 1}); 'ok'", world: .defaultClient)
        CaptureSpin.wait(0.2)
        XCTAssertEqual(page.recording.report["inputs"] as? Int, 1, "a malformed input is dropped")
        page.recording.cancel()
    }
}
