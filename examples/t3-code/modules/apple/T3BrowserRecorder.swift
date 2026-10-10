#if os(macOS)
import AppKit
import AVFoundation
import CoreText
import WebKit

/// A Browser tab's recording (browser-surface part 3, capture; X1 path B). MIT reference, see LICENSE-T3, T3 Code
/// 1e2ecbd975: apps/desktop/src/preview/RecordingCursor.ts and RecordingInput.ts (the in-page cursor and inputs,
/// here assets/browser-recording.js), apps/web/src/browser/recordingCompositor.ts (`RecordingDecorations`,
/// `createRecordingCompositor`), apps/web/src/browser/browserRecording.ts (`preferredMimeTypes`,
/// `createMediaRecorder`, `captureTabMediaStream`) and apps/desktop/src/preview/Manager.ts (`startRecording`,
/// `stopRecording`, `saveRecording`, `recordingFileExtension`, `restoreRecordingCursor`).
///
/// The reference records a tab with Chromium's display media (`getDisplayMedia` routed to the armed guest) and a
/// `MediaRecorder`, decorating a detached canvas when key or mouse presses are shown. Here the module does each
/// part natively, behind the same seams TypeScript's lifecycle drives (browser-recording.ts):
/// - **The capture lease** (`startScreencast` / `stopScreencast`): `arm` installs the page script in the module's
///   own content world (`.defaultClient`; the page cannot see or call it) — the drawn cursor and the key and pointer
///   inputs — and warms the source with one snapshot; `disarm` removes it; `documentReady` re-installs it after a
///   navigation (Manager's `restoreRecordingCursor` on dom-ready).
/// - **The stream** (`getDisplayMedia`): the frame source is `WKWebView.takeSnapshot` polled at the recording's
///   frame rate, pipelined with at most two snapshots in flight (a tick is skipped while two are out). It needs no
///   system permission (ScreenCaptureKit would ask for Screen Recording), captures the page's own pixels only (no
///   other window, no native pointer, which the page script draws instead), and works while the window is covered
///   or the tab's page is parked off screen; measured at ~60 snapshots a second on a 1,280-point page. A frame is
///   the snapshot at its native pixel size (the backing scale included).
/// - **The recorder** (`MediaRecorder`): an `AVAssetWriter` writes H.264 into an MP4 in a temporary file, each
///   frame at its capture time (variable frame rate; the first at 0), at the caller's bit rate. `finish` repeats the
///   last frame at the stop time, so the file lasts until the stop, and finishes the file; TypeScript then saves it
///   into the artifact directory.
/// - **The compositor** (`createRecordingCompositor`): with key or mouse presses shown, each frame is drawn into the
///   encoder's buffer and `T3RecordingDecorations` (a port of `RecordingDecorations`, same numbers) draws the press
///   rings and key badges over it; with both off frames go straight through and no compositor exists.
final class T3BrowserRecording: NSObject {
    static let handlerName = "t3BrowserRecording"
    /// browserRecording.ts `preferredMimeTypes`, in order.
    static let preferredMimeTypes = [
        "video/mp4;codecs=avc1",
        "video/mp4;codecs=avc1.640028",
        "video/mp4;codecs=avc1.42e01e",
        "video/webm;codecs=vp9",
        "video/webm;codecs=vp8",
        "video/webm",
    ]
    /// The encoder writes H.264 in MP4 (VideoToolbox); WebM is not written (`MediaRecorder.isTypeSupported` false).
    static func supports(_ mimeType: String) -> Bool { mp4Profiles[mimeType.replacingOccurrences(of: " ", with: "").lowercased()] != nil }
    private static let mp4Profiles: [String: String] = [
        "video/mp4": AVVideoProfileLevelH264HighAutoLevel,
        "video/mp4;codecs=avc1": AVVideoProfileLevelH264HighAutoLevel,
        "video/mp4;codecs=avc1.640028": AVVideoProfileLevelH264HighAutoLevel,
        "video/mp4;codecs=avc1.42e01e": AVVideoProfileLevelH264BaselineAutoLevel,
    ]
    /// The recorder's own format when the caller names none (the browser's choice in the reference).
    static let defaultMimeType = "video/mp4;codecs=avc1"
    /// H.264's largest frame side here; a larger snapshot is scaled down to fit.
    static let maximumSide = 4096

    weak var page: WKWebView?
    var changed: (() -> Void)?

    private(set) var armed = false
    private(set) var capturing = false
    private(set) var frameRate = 30
    /// The compositor (nil with both decorations off).
    private(set) var decorations: T3RecordingDecorations?
    private var options = (showKeyPresses: false, showMousePresses: false)
    private var theme: [String: String] = [:]
    private var controller = "none"
    private var closed = false
    private var timer: Timer?
    private var inFlight = 0
    private var requested = 0, accepted = 0
    private var firstFrame: ((Result<[String: Any], Error>) -> Void)?
    private var firstFrameDeadline: DispatchWorkItem?
    /// The newest frame and its capture time (ms, monotonic).
    private var latest: (image: CGImage, at: Double)?
    private var frameSize = CGSize.zero
    private var writer: Writer?
    private var stopping = false
    private(set) var inputs = 0
    private(set) var dropped = 0
    private let lock = NSLock()
    private weak var controllerRef: WKUserContentController?

    /// The script message handler holds this object weakly: WKUserContentController retains it.
    private final class Relay: NSObject, WKScriptMessageHandler {
        weak var owner: T3BrowserRecording?
        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) { owner?.received(message) }
    }

    init(configuration: WKWebViewConfiguration) {
        super.init()
        let relay = Relay(); relay.owner = self
        configuration.userContentController.add(relay, contentWorld: .defaultClient, name: Self.handlerName)
        controllerRef = configuration.userContentController
    }

    static func now() -> Double { ProcessInfo.processInfo.systemUptime * 1000 }

    private func note(_ line: String) { FileHandle.standardError.write(Data("t3.browser: record \(line.prefix(300))\n".utf8)) }

    // MARK: The capture lease (startScreencast / stopScreencast)

    func arm(showKeyPresses: Bool, showMousePresses: Bool, theme: [String: String], controller: String, completion: @escaping (Error?) -> Void) {
        guard let page, !closed else { return completion(T3BrowserRecordingError("The browser page is gone.")) }
        options = (showKeyPresses, showMousePresses)
        self.theme = theme
        self.controller = ["agent", "human", "none"].contains(controller) ? controller : "none"
        install(on: page) { [weak self] error in
            guard let self else { return }
            if let error { return completion(error) }
            self.armed = true
            self.note("arm keys=\(showKeyPresses) mouse=\(showMousePresses)")
            self.changed?()
            // Manager recording.warmSource: one capture before the stream starts (a failure is not fatal).
            page.takeSnapshot(with: nil) { _, _ in completion(nil) }
        }
    }

    func disarm() {
        guard armed else { return }
        armed = false
        page?.evaluateJavaScript("globalThis.__t3codeRecording ? globalThis.__t3codeRecording.stop() : true", in: nil, in: .defaultClient) { _ in }
        note("disarm")
        changed?()
    }

    /// A committed or finished main-frame load: the new document has no cursor until it is installed again.
    func documentReady() {
        guard armed, let page else { return }
        install(on: page) { _ in }
    }

    /// The page script (assets/browser-recording.js, idempotent per document) and `start` with the options.
    private func install(on page: WKWebView, completion: @escaping (Error?) -> Void) {
        guard let url = T3TerminalAssets.fileURL("browser-recording.js"), let source = try? String(contentsOf: url, encoding: .utf8) else {
            return completion(T3BrowserRecordingError("The recording overlay is missing (assets/browser-recording.js)."))
        }
        page.evaluateJavaScript(source + "\n;true", in: nil, in: .defaultClient) { [weak self] result in
            guard let self else { return }
            if case .failure(let error) = result { return completion(error) }
            let arguments: [String: Any] = ["options": ["showKeyPresses": self.options.showKeyPresses, "showMousePresses": self.options.showMousePresses],
                                            "theme": self.theme, "controller": self.controller]
            page.callAsyncJavaScript("return globalThis.__t3codeRecording ? globalThis.__t3codeRecording.start(options, theme, controller) : false",
                                     arguments: arguments, in: nil, in: .defaultClient) { result in
                switch result {
                case .success(let value) where (value as? Bool) == true: completion(nil)
                case .success: completion(T3BrowserRecordingError("The recording overlay did not start."))
                case .failure(let error): completion(error)
                }
            }
        }
    }

    // MARK: The page's inputs (Manager's RECORDING_INPUT_CHANNEL handler)

    private func received(_ message: WKScriptMessage) {
        guard message.webView === page, armed || writer != nil, let input = Self.recordingInput(message.body) else { return }
        // Manager: a key only with key presses on, a pointer only with mouse presses on.
        if input["type"] as? String == "key", !options.showKeyPresses { return }
        if input["type"] as? String == "pointer", !options.showMousePresses { return }
        inputs += 1
        lock.lock(); let compositor = decorations; lock.unlock()
        guard let compositor else { return }
        let now = Self.now()
        lock.lock(); compositor.apply(input, now: now); lock.unlock()
        // recordingCompositor's `draw` on every input: the press shows at once, not at the next snapshot.
        redraw(at: now)
    }

    /// `DesktopPreviewRecordingInputSchema`: the three input shapes, anything else dropped.
    static func recordingInput(_ body: Any) -> [String: Any]? {
        guard let value = body as? [String: Any], let type = value["type"] as? String else { return nil }
        let finite = { (key: String) -> Double? in (value[key] as? NSNumber).map(\.doubleValue).flatMap { $0.isFinite ? $0 : nil } }
        switch type {
        case "clear": return ["type": "clear"]
        case "key":
            guard let held = value["held"] as? Bool, let width = finite("width"), width > 0 else { return nil }
            let label = value["label"] as? String
            if value["label"] != nil, !(value["label"] is NSNull), label == nil { return nil }
            if let label, label.count > 100 { return nil }
            return ["type": "key", "label": label ?? NSNull(), "held": held, "width": width]
        case "pointer":
            guard let phase = value["phase"] as? String, ["move", "down", "up", "click"].contains(phase), let x = finite("x"), let y = finite("y"),
                  let width = finite("width"), width > 0, let height = finite("height"), height > 0 else { return nil }
            return ["type": "pointer", "phase": phase, "x": x, "y": y, "width": width, "height": height]
        default: return nil
        }
    }

    // MARK: The stream (getDisplayMedia)

    func capture(frameRate: Int, completion: @escaping (Result<[String: Any], Error>) -> Void) {
        guard page != nil, !closed else { return completion(.failure(T3BrowserRecordingError("The browser page is gone."))) }
        if capturing, let latest {
            return completion(.success(["width": latest.image.width, "height": latest.image.height, "frameRate": self.frameRate]))
        }
        self.frameRate = max(1, min(120, frameRate))
        capturing = true
        firstFrame = completion
        let deadline = DispatchWorkItem { [weak self] in
            guard let self, let waiting = self.firstFrame else { return }
            self.firstFrame = nil
            self.releaseCapture()
            waiting(.failure(T3BrowserRecordingError("The browser page gave no frame within 5 seconds.")))
        }
        firstFrameDeadline = deadline
        DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: deadline)
        let timer = Timer(timeInterval: 1 / Double(self.frameRate), repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
        tick()
        changed?()
    }

    func releaseCapture() {
        guard capturing else { return }
        capturing = false
        timer?.invalidate(); timer = nil
        firstFrameDeadline?.cancel(); firstFrameDeadline = nil
        if let waiting = firstFrame { firstFrame = nil; waiting(.failure(T3BrowserRecordingError("The capture was released before its first frame."))) }
        note("release frames=\(writerFrames()) dropped=\(dropped)")
        changed?()
    }

    private func tick() {
        guard capturing, let page else { return releaseCapture() }
        if inFlight >= 2 {
            lock.lock(); dropped += 1; lock.unlock()
            // A wedged page still shows and expires its decorations in the file.
            lock.lock(); let due = decorations?.nextRedraw(now: Self.now()) != nil; lock.unlock()
            if due { redraw(at: Self.now()) }
            return
        }
        inFlight += 1
        requested += 1
        let sequence = requested
        page.takeSnapshot(with: nil) { [weak self] image, _ in
            guard let self else { return }
            self.inFlight -= 1
            guard self.capturing, sequence > self.accepted, let image, let frame = Self.cgImage(image) else { return }
            self.accepted = sequence
            let at = Self.now()
            self.latest = (frame, at)
            if let waiting = self.firstFrame {
                self.firstFrame = nil
                self.firstFrameDeadline?.cancel(); self.firstFrameDeadline = nil
                self.note("capture \(self.frameRate)fps \(frame.width)x\(frame.height)")
                waiting(.success(["width": frame.width, "height": frame.height, "frameRate": self.frameRate]))
            }
            if self.writer != nil, !self.stopping { self.append(frame, at: at) }
        }
    }

    /// The snapshot's largest bitmap: its native pixel size.
    static func cgImage(_ image: NSImage) -> CGImage? {
        guard let rep = image.representations.max(by: { $0.pixelsWide * $0.pixelsHigh < $1.pixelsWide * $1.pixelsHigh }) else { return nil }
        var rect = CGRect(x: 0, y: 0, width: rep.pixelsWide, height: rep.pixelsHigh)
        return rep.cgImage(forProposedRect: &rect, context: nil, hints: nil) ?? image.cgImage(forProposedRect: nil, context: nil, hints: nil)
    }

    // MARK: The compositor (createRecordingCompositor)

    /// False, with no compositor and no input subscription, when both decorations are off.
    @discardableResult
    func useDecorations(showKeyPresses: Bool, showMousePresses: Bool, primaryColor: String) -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard showKeyPresses || showMousePresses else { decorations = nil; return false }
        decorations = T3RecordingDecorations(showKeyPresses: showKeyPresses, showMousePresses: showMousePresses, frameRate: frameRate, primaryColor: primaryColor)
        return true
    }

    /// The latest frame again, decorated at `now` (an input, or a due expiry while snapshots stall).
    private func redraw(at now: Double) {
        guard let latest, writer != nil, !stopping else { return }
        append(latest.image, at: now)
    }

    // MARK: The recorder (MediaRecorder)

    /// One encoder session: its file, the writer and its input, on its own queue.
    private final class Writer {
        let url: URL
        let writer: AVAssetWriter
        let input: AVAssetWriterInput
        let adaptor: AVAssetWriterInputPixelBufferAdaptor
        let mimeType: String
        let width: Int, height: Int
        let queue = DispatchQueue(label: "com.exact.t3code.browser.recording", qos: .userInitiated)
        var start: Double?
        /// The last presentation time written, in `timescale` units (strictly increasing).
        var lastTime: CMTimeValue = -1
        var frames = 0
        var failed = false
        init(url: URL, writer: AVAssetWriter, input: AVAssetWriterInput, adaptor: AVAssetWriterInputPixelBufferAdaptor, mimeType: String, width: Int, height: Int) {
            self.url = url; self.writer = writer; self.input = input; self.adaptor = adaptor; self.mimeType = mimeType; self.width = width; self.height = height
        }
    }

    func begin(mimeType: String?, bitsPerSecond: Int) throws -> String {
        do { return try beginWriting(mimeType: mimeType, bitsPerSecond: bitsPerSecond) }
        catch {
            // cleanupFailedRecordingStart: a recorder that cannot start leaves no compositor and no file behind.
            lock.lock(); decorations = nil; lock.unlock()
            throw error
        }
    }

    private func beginWriting(mimeType: String?, bitsPerSecond: Int) throws -> String {
        guard writer == nil else { throw T3BrowserRecordingError("The recorder is already running.") }
        guard capturing, let latest else { throw T3BrowserRecordingError("The recording has no stream to encode.") }
        let actual = mimeType ?? Self.defaultMimeType
        guard let profile = Self.mp4Profiles[actual.replacingOccurrences(of: " ", with: "").lowercased()] else {
            throw T3BrowserRecordingError("The recorder cannot write \(actual).")
        }
        let (width, height) = Self.encodedSize(width: latest.image.width, height: latest.image.height)
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-browser-recording-\(UUID().uuidString.lowercased()).mp4")
        let assetWriter = try AVAssetWriter(outputURL: url, fileType: .mp4)
        let settings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: width, AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: max(100_000, bitsPerSecond), AVVideoProfileLevelKey: profile,
                                              AVVideoExpectedSourceFrameRateKey: frameRate, AVVideoMaxKeyFrameIntervalKey: frameRate * 2],
        ]
        let input = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
        input.expectsMediaDataInRealTime = true
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA, kCVPixelBufferWidthKey as String: width, kCVPixelBufferHeightKey as String: height,
            kCVPixelBufferCGImageCompatibilityKey as String: true, kCVPixelBufferCGBitmapContextCompatibilityKey as String: true,
        ])
        guard assetWriter.canAdd(input) else { throw T3BrowserRecordingError("The recorder cannot encode \(width)×\(height).") }
        assetWriter.add(input)
        guard assetWriter.startWriting() else { throw assetWriter.error ?? T3BrowserRecordingError("The recorder did not start.") }
        assetWriter.startSession(atSourceTime: .zero)
        writer = Writer(url: url, writer: assetWriter, input: input, adaptor: adaptor, mimeType: actual, width: width, height: height)
        stopping = false
        frameSize = CGSize(width: width, height: height)
        note("begin \(actual) \(width)x\(height) \(bitsPerSecond)bps \(frameRate)fps")
        append(latest.image, at: Self.now())
        changed?()
        return actual
    }

    /// Even sides no larger than `maximumSide`, the aspect kept.
    static func encodedSize(width: Int, height: Int) -> (Int, Int) {
        let scale = min(1, Double(maximumSide) / Double(max(width, height, 1)))
        let even = { (value: Double) -> Int in max(2, Int(value) & ~1) }
        return (even(Double(width) * scale), even(Double(height) * scale))
    }

    private func writerFrames() -> Int { guard let writer else { return 0 }; return writer.queue.sync { writer.frames } }

    /// One frame at its capture time (ms), decorated when a compositor exists, on the writer's queue.
    private func append(_ image: CGImage, at: Double) {
        guard let writer else { return }
        lock.lock(); let compositor = decorations; lock.unlock()
        writer.queue.async { [weak self] in
            guard let self else { return }
            let start = writer.start ?? at
            writer.start = start
            let time = Self.time(at - start)
            guard time.value > writer.lastTime else { return }
            guard writer.input.isReadyForMoreMediaData, let pool = writer.adaptor.pixelBufferPool else {
                if writer.writer.status == .failed, !writer.failed {
                    writer.failed = true
                    self.note("encoder failed: \(Self.describe(writer.writer.error))")
                }
                self.lock.lock(); self.dropped += 1; self.lock.unlock(); return
            }
            var buffer: CVPixelBuffer?
            guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &buffer) == kCVReturnSuccess, let buffer else { return }
            CVPixelBufferLockBaseAddress(buffer, [])
            if let context = CGContext(data: CVPixelBufferGetBaseAddress(buffer), width: writer.width, height: writer.height, bitsPerComponent: 8,
                                       bytesPerRow: CVPixelBufferGetBytesPerRow(buffer), space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                       bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue) {
                context.setFillColor(CGColor(gray: 0, alpha: 1))
                context.fill(CGRect(x: 0, y: 0, width: writer.width, height: writer.height))
                context.interpolationQuality = .high
                context.draw(image, in: Self.fit(image, width: writer.width, height: writer.height))
                if let compositor {
                    let canvas = T3RecordingCGCanvas(context: context, height: writer.height)
                    self.lock.lock(); compositor.draw(canvas, width: Double(writer.width), height: Double(writer.height), now: at); self.lock.unlock()
                }
            }
            CVPixelBufferUnlockBaseAddress(buffer, [])
            if writer.adaptor.append(buffer, withPresentationTime: time) {
                writer.lastTime = time.value
                writer.frames += 1
            }
        }
    }

    /// A frame's presentation time: milliseconds since the first frame, in 1/90,000 s (two frames never share one).
    static let timescale: CMTimeScale = 90_000
    static func time(_ milliseconds: Double) -> CMTime { CMTime(value: CMTimeValue((max(0, milliseconds) * Double(timescale) / 1000).rounded()), timescale: timescale) }

    /// An encoder error with its underlying cause (AVFoundation's own words are generic).
    static func describe(_ error: Error?) -> String {
        guard let error = error as NSError? else { return "unknown" }
        let underlying = error.userInfo[NSUnderlyingErrorKey] as? NSError
        return "\(error.domain) \(error.code)\(underlying.map { " (\($0.domain) \($0.code))" } ?? "")"
    }

    /// The image aspect-fit in the frame (a page resized mid-recording keeps its proportions).
    static func fit(_ image: CGImage, width: Int, height: Int) -> CGRect {
        let scale = min(Double(width) / Double(image.width), Double(height) / Double(image.height))
        let w = Double(image.width) * scale, h = Double(image.height) * scale
        return CGRect(x: (Double(width) - w) / 2, y: (Double(height) - h) / 2, width: w, height: h)
    }

    func finish(completion: @escaping (Result<[String: Any], Error>) -> Void) {
        guard let writer, !stopping else { return completion(.failure(T3BrowserRecordingError("The recorder is not running."))) }
        stopping = true
        let stopAt = Self.now()
        let last = latest?.image
        lock.lock(); let compositor = decorations; lock.unlock()
        // The last frame again at the stop time, then the file is finished.
        writer.queue.async { [weak self] in
            guard let self else { return }
            if let last, let start = writer.start, Self.time(stopAt - start).value > writer.lastTime, writer.input.isReadyForMoreMediaData, let pool = writer.adaptor.pixelBufferPool {
                var buffer: CVPixelBuffer?
                if CVPixelBufferPoolCreatePixelBuffer(nil, pool, &buffer) == kCVReturnSuccess, let buffer {
                    CVPixelBufferLockBaseAddress(buffer, [])
                    if let context = CGContext(data: CVPixelBufferGetBaseAddress(buffer), width: writer.width, height: writer.height, bitsPerComponent: 8,
                                               bytesPerRow: CVPixelBufferGetBytesPerRow(buffer), space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                               bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue) {
                        context.setFillColor(CGColor(gray: 0, alpha: 1))
                        context.fill(CGRect(x: 0, y: 0, width: writer.width, height: writer.height))
                        context.draw(last, in: Self.fit(last, width: writer.width, height: writer.height))
                        if let compositor {
                            self.lock.lock(); compositor.draw(T3RecordingCGCanvas(context: context, height: writer.height), width: Double(writer.width), height: Double(writer.height), now: stopAt); self.lock.unlock()
                        }
                    }
                    CVPixelBufferUnlockBaseAddress(buffer, [])
                    let time = Self.time(stopAt - start)
                    if writer.adaptor.append(buffer, withPresentationTime: time) { writer.lastTime = time.value; writer.frames += 1 }
                }
            }
            let frames = writer.frames
            guard frames > 0 else {
                writer.input.markAsFinished()
                writer.writer.cancelWriting()
                try? FileManager.default.removeItem(at: writer.url)
                return DispatchQueue.main.async {
                    if self.writer === writer { self.writer = nil }
                    self.stopping = false
                    completion(.failure(T3BrowserRecordingError("The recording has no frames.")))
                }
            }
            writer.input.markAsFinished()
            writer.writer.finishWriting {
                DispatchQueue.main.async {
                    if self.writer === writer { self.writer = nil }
                    self.stopping = false
                    self.changed?()
                    guard writer.writer.status == .completed else {
                        try? FileManager.default.removeItem(at: writer.url)
                        return completion(.failure(writer.writer.error ?? T3BrowserRecordingError("The recording could not be finished.")))
                    }
                    let size = (try? FileManager.default.attributesOfItem(atPath: writer.url.path)[.size] as? NSNumber)?.intValue ?? 0
                    self.note("finish frames=\(frames) bytes=\(size)")
                    completion(.success(["path": writer.url.path, "mimeType": writer.mimeType, "sizeBytes": size, "frames": frames,
                                         "width": writer.width, "height": writer.height]))
                }
            }
        }
    }

    /// Capture, encoder, compositor and script off; the temporary file removed. Safe to repeat.
    func cancel() {
        releaseCapture()
        if let writer {
            self.writer = nil
            writer.queue.async {
                if writer.writer.status == .writing { writer.input.markAsFinished(); writer.writer.cancelWriting() }
                try? FileManager.default.removeItem(at: writer.url)
            }
            note("cancel")
        }
        stopping = false
        lock.lock(); decorations = nil; lock.unlock()
        disarm()
        latest = nil
        changed?()
    }

    var recording: Bool { writer != nil }
    /// The encoder's temporary file while it writes.
    var temporaryURL: URL? { writer?.url }

    var report: [String: Any] {
        lock.lock(); let dropped = self.dropped; lock.unlock()
        return ["armed": armed, "capturing": capturing, "recording": writer != nil, "frames": writerFrames(), "dropped": dropped, "inputs": inputs, "frameRate": frameRate,
                "width": Int(frameSize.width), "height": Int(frameSize.height)]
    }

    /// The tab closed: everything off and the handler removed.
    func close() {
        guard !closed else { return }
        cancel()
        closed = true
        controllerRef?.removeScriptMessageHandler(forName: Self.handlerName, contentWorld: .defaultClient)
    }
}

struct T3BrowserRecordingError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

/// The 2D drawing calls `RecordingDecorations.draw` makes (a canvas context in the reference).
protocol T3RecordingCanvas: AnyObject {
    func save()
    func restore()
    func beginPath()
    func ellipse(x: Double, y: Double, rx: Double, ry: Double)
    func fill()
    func stroke()
    func roundRect(x: Double, y: Double, width: Double, height: Double, radius: Double)
    func fillText(_ text: String, x: Double, y: Double, maxWidth: Double)
    func measureText(_ text: String) -> Double
    var globalAlpha: Double { get set }
    var strokeStyle: String { get set }
    var fillStyle: String { get set }
    var lineWidth: Double { get set }
    var font: String { get set }
    var textAlign: String { get set }
    var textBaseline: String { get set }
}

/// recordingCompositor.ts `RecordingDecorations`: input timing and coordinates independent of frame delivery.
final class T3RecordingDecorations {
    struct Ring { var x, y, width, height: Double; var held: Bool; var releasedAt: Double? }
    struct Key { var label: String; var width: Double; var expiresAt: Double? }
    private(set) var ring: Ring?
    private(set) var key: Key?
    let showKeyPresses: Bool, showMousePresses: Bool, frameRate: Int, primaryColor: String

    init(showKeyPresses: Bool, showMousePresses: Bool, frameRate: Int, primaryColor: String) {
        self.showKeyPresses = showKeyPresses; self.showMousePresses = showMousePresses; self.frameRate = max(1, frameRate); self.primaryColor = primaryColor
    }

    func apply(_ input: [String: Any], now: Double) {
        let number = { (key: String) -> Double in (input[key] as? NSNumber)?.doubleValue ?? 0 }
        switch input["type"] as? String {
        case "clear":
            ring = nil
            key = nil
        case "key" where showKeyPresses:
            if let label = input["label"] as? String, !label.isEmpty {
                key = Key(label: label, width: number("width"), expiresAt: input["held"] as? Bool == true ? nil : now + 900)
            } else { key = nil }
        case "pointer" where showMousePresses:
            let phase = input["phase"] as? String ?? ""
            if phase == "down" || phase == "click" {
                ring = Ring(x: number("x"), y: number("y"), width: number("width"), height: number("height"), held: phase == "down", releasedAt: phase == "click" ? now : nil)
            } else if var current = ring, current.held {
                current.x = number("x"); current.y = number("y"); current.width = number("width"); current.height = number("height")
                current.held = phase != "up"
                current.releasedAt = phase == "up" ? now : nil
                ring = current
            }
        default: break
        }
    }

    func nextRedraw(now: Double) -> Double? {
        if let releasedAt = ring?.releasedAt, now < releasedAt + 600 { return 1000 / Double(frameRate) }
        if let expiresAt = key?.expiresAt, now < expiresAt { return expiresAt - now }
        return nil
    }

    func draw(_ context: T3RecordingCanvas, width: Double, height: Double, now: Double) {
        // Guest coordinates are CSS pixels; native frames include zoom and display scale.
        let scale = width / (key?.width ?? ring?.width ?? 1280)
        if let ring, ring.held || (ring.releasedAt != nil && now < ring.releasedAt! + 600) {
            let progress = ring.releasedAt.map { min(1, (now - $0) / 600) } ?? 0
            context.save()
            let opacity = 0.9 * (1 - progress)
            context.strokeStyle = primaryColor
            context.fillStyle = primaryColor
            context.lineWidth = 2 * scale
            context.beginPath()
            context.ellipse(x: ring.x * width / ring.width, y: ring.y * height / ring.height,
                            rx: 20 * width / ring.width * (1 + progress * 0.5), ry: 20 * height / ring.height * (1 + progress * 0.5))
            context.globalAlpha = opacity * 0.15
            context.fill()
            context.globalAlpha = opacity
            context.stroke()
            context.restore()
        }
        if let key, key.expiresAt == nil || now < key.expiresAt! {
            context.save()
            context.font = "500 \(26 * scale)px system-ui, sans-serif"
            let badgeWidth = min(width - 32 * scale, context.measureText(key.label) + 36 * scale)
            let badgeHeight = 54 * scale
            let left = (width - badgeWidth) / 2
            let top = height - 24 * scale - badgeHeight
            context.fillStyle = "rgba(32,32,34,.86)"
            context.beginPath()
            context.roundRect(x: left, y: top, width: badgeWidth, height: badgeHeight, radius: 14 * scale)
            context.fill()
            context.fillStyle = "white"
            context.textAlign = "center"
            context.textBaseline = "middle"
            context.fillText(key.label, x: width / 2, y: top + badgeHeight / 2, maxWidth: badgeWidth - 24 * scale)
            context.restore()
        }
    }
}

/// A canvas over a `CGContext` with the canvas's top-left origin (y down). Colours are CSS hex, `rgb()`/`rgba()`,
/// `white` or `black`; anything else (an `oklch()` theme colour) draws in the reference's fallback blue.
final class T3RecordingCGCanvas: T3RecordingCanvas {
    private let context: CGContext
    private var path = CGMutablePath()
    private var stack: [(Double, String, String, Double, String, String, String)] = []
    var globalAlpha: Double = 1 { didSet { context.setAlpha(CGFloat(globalAlpha)) } }
    var strokeStyle = "#000000"
    var fillStyle = "#000000"
    var lineWidth: Double = 1
    var font = "10px sans-serif"
    var textAlign = "start"
    var textBaseline = "alphabetic"

    init(context: CGContext, height: Int) {
        self.context = context
        context.translateBy(x: 0, y: CGFloat(height))
        context.scaleBy(x: 1, y: -1)
    }

    func save() {
        context.saveGState()
        stack.append((globalAlpha, strokeStyle, fillStyle, lineWidth, font, textAlign, textBaseline))
    }
    func restore() {
        context.restoreGState()
        guard let state = stack.popLast() else { return }
        (globalAlpha, strokeStyle, fillStyle, lineWidth, font, textAlign, textBaseline) = state
    }
    func beginPath() { path = CGMutablePath() }
    func ellipse(x: Double, y: Double, rx: Double, ry: Double) {
        path.addEllipse(in: CGRect(x: x - rx, y: y - ry, width: 2 * rx, height: 2 * ry))
    }
    func roundRect(x: Double, y: Double, width: Double, height: Double, radius: Double) {
        let r = max(0, min(radius, width / 2, height / 2))
        path.addRoundedRect(in: CGRect(x: x, y: y, width: width, height: height), cornerWidth: r, cornerHeight: r)
    }
    func fill() {
        context.addPath(path)
        context.setFillColor(Self.color(fillStyle))
        context.fillPath()
    }
    func stroke() {
        context.addPath(path)
        context.setStrokeColor(Self.color(strokeStyle))
        context.setLineWidth(CGFloat(lineWidth))
        context.strokePath()
    }

    private func line(_ text: String) -> CTLine {
        let attributes: [NSAttributedString.Key: Any] = [.font: Self.font(font), NSAttributedString.Key(kCTForegroundColorAttributeName as String): Self.color(fillStyle)]
        return CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attributes))
    }
    func measureText(_ text: String) -> Double { Double(CTLineGetTypographicBounds(line(text), nil, nil, nil)) }
    func fillText(_ text: String, x: Double, y: Double, maxWidth: Double) {
        let line = line(text)
        var ascent: CGFloat = 0, descent: CGFloat = 0
        let width = Double(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
        let squeeze = maxWidth > 0 && width > maxWidth ? maxWidth / width : 1
        let drawn = width * squeeze
        let startX = textAlign == "center" ? x - drawn / 2 : textAlign == "right" || textAlign == "end" ? x - drawn : x
        let baseline = textBaseline == "middle" ? y + Double(ascent - descent) / 2 : textBaseline == "top" ? y + Double(ascent) : y
        context.saveGState()
        context.translateBy(x: CGFloat(startX), y: CGFloat(baseline))
        context.scaleBy(x: CGFloat(squeeze), y: 1)
        context.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        context.textPosition = .zero
        CTLineDraw(line, context)
        context.restoreGState()
    }

    /// `500 26px system-ui, sans-serif`: the weight and the size; the family is the system's.
    static func font(_ spec: String) -> NSFont {
        let size = spec.range(of: "[0-9.]+(?=px)", options: .regularExpression).flatMap { Double(spec[$0]) } ?? 10
        let weightValue = spec.range(of: "^\\s*[0-9]{3}\\b", options: .regularExpression).flatMap { Int(spec[$0].trimmingCharacters(in: .whitespaces)) } ?? 400
        let weight: NSFont.Weight = weightValue >= 700 ? .bold : weightValue >= 600 ? .semibold : weightValue >= 500 ? .medium : .regular
        return NSFont.systemFont(ofSize: CGFloat(max(1, size)), weight: weight)
    }

    static func color(_ css: String) -> CGColor {
        let value = css.trimmingCharacters(in: .whitespaces).lowercased()
        let fallback = CGColor(srgbRed: 0x25 / 255, green: 0x63 / 255, blue: 0xeb / 255, alpha: 1)
        if value == "white" { return CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1) }
        if value == "black" { return CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1) }
        if value.hasPrefix("#") {
            var hex = String(value.dropFirst())
            if hex.count == 3 || hex.count == 4 { hex = hex.map { "\($0)\($0)" }.joined() }
            guard hex.count == 6 || hex.count == 8, let number = UInt64(hex, radix: 16) else { return fallback }
            let channels = hex.count == 8 ? [24, 16, 8, 0] : [16, 8, 0]
            let parts = channels.map { Double((number >> UInt64($0)) & 0xff) / 255 }
            return CGColor(srgbRed: parts[0], green: parts[1], blue: parts[2], alpha: parts.count == 4 ? parts[3] : 1)
        }
        if value.hasPrefix("rgb"), let open = value.firstIndex(of: "("), let close = value.lastIndex(of: ")") {
            let parts = value[value.index(after: open)..<close].split(whereSeparator: { $0 == "," || $0 == "/" || $0 == " " }).compactMap { Double($0) }
            guard parts.count >= 3 else { return fallback }
            return CGColor(srgbRed: parts[0] / 255, green: parts[1] / 255, blue: parts[2] / 255, alpha: parts.count > 3 ? min(1, max(0, parts[3])) : 1)
        }
        return fallback
    }
}
#endif
