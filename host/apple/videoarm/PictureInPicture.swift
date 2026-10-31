// @ref LLP 1042 §9. An image's pictures in the platform's picture-in-picture window: each
// picture its source names is decoded into a sample buffer for an AVSampleBufferDisplayLayer,
// which AVPictureInPictureController floats as a live source (no seeking, never paused).
import Foundation
import AVFoundation
import AVKit
import CoreMedia
import ImageIO
#if os(macOS)
import AppKit

/// On macOS the PiP panel does not take the display layer in, as iOS does: it mirrors the
/// layer's pixels at 1:1 from the window that holds it, so a layer larger than the panel
/// shows only its lower-left corner and one in an app's own window shows that window's
/// part. The layer therefore lives in a window of its own, invisible and click-through,
/// kept at the panel's size (Apple Developer Forums 821582, FB22411168).
private final class PictureInPictureArm: NSObject, AVPictureInPictureSampleBufferPlaybackDelegate, AVPictureInPictureControllerDelegate {
    private let context: UnsafeMutableRawPointer?
    private let callback: VideoCallback
    private let layer = AVSampleBufferDisplayLayer()
    private let holder: NSWindow
    private var controller: AVPictureInPictureController?
    private var panel: NSWindow?
    private var resized: NSObjectProtocol?
    private var started = false
    private var invalid = false
    /// One decode at a time; a path that arrives meanwhile waits, replacing any before it.
    private var decoding = false
    private var waiting: String?
    /// The pixel size of the pictures in the window. Each change of the video's size makes
    /// AVKit lay its video layer out again at that size, and only the panel's next layout fits
    /// it back into the panel: when no panel layout follows, the picture is left outside the
    /// panel, which shows blank. A stream's pictures differ by a pixel as the page is scaled
    /// (922 × 1280 after a seed of 1078 × 1496), so a picture of nearly the same aspect is
    /// drawn at this size. A new aspect resizes the panel, whose layout then fits the video.
    private var pixels: (width: Int, height: Int)?
    private static let decoder = DispatchQueue(label: "exact.video.picture-in-picture", qos: .userInitiated)
    /// The longest side a picture is decoded at: the panel is at most a quarter of the screen.
    private static let maxPixel = 1920
    /// `EXACT_PIP_LOG=<file>`: a diagnostic trace of the renderer, the holder window and the panel.
    private static let trace: FileHandle? = ProcessInfo.processInfo.environment["EXACT_PIP_LOG"].flatMap { path in
        FileManager.default.createFile(atPath: path, contents: nil)
        return FileHandle(forWritingAtPath: path)
    }
    private let born = CACurrentMediaTime()
    private var enqueued = 0
    private var firstTime: CMTime = .invalid, lastTime: CMTime = .invalid
    private var panelFound = -1.0
    private var traceTimer: Timer?

    init(context: UnsafeMutableRawPointer?, callback: @escaping VideoCallback) {
        self.context = context; self.callback = callback
        holder = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 400), styleMask: [.borderless], backing: .buffered, defer: false)
        super.init()
        holder.isReleasedWhenClosed = false
        holder.alphaValue = 0
        holder.ignoresMouseEvents = true
        holder.hasShadow = false
        holder.canHide = false
        holder.isExcludedFromWindowsMenu = true
        holder.collectionBehavior = [.transient, .ignoresCycle, .canJoinAllSpaces]
        let view = NSView(frame: holder.contentLayoutRect)
        view.wantsLayer = true
        holder.contentView = view
        layer.videoGravity = .resizeAspect
        layer.frame = view.bounds
        view.layer?.addSublayer(layer)
        if Self.trace != nil {
            log("created holder=\(holder.windowNumber)")
            traceTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
                guard let self else { return }
                self.log("tick " + self.snapshot())
            }
        }
    }

    private func log(_ text: String) {
        guard let trace = Self.trace else { return }
        trace.write(Data(String(format: "%7.3f %@\n", CACurrentMediaTime() - born, text).utf8))
    }

    private func snapshot() -> String {
        let renderer = layer.sampleBufferRenderer
        let occluded = holder.occlusionState.contains(.visible) ? "visible" : "occluded"
        return "status=\(renderer.status.rawValue) error=\(renderer.error.map { "\($0)" } ?? "-") flushNeeded=\(renderer.requiresFlushToResumeDecoding) ready=\(renderer.isReadyForMoreMediaData)"
            + " enqueued=\(enqueued) first=\(firstTime.seconds) last=\(lastTime.seconds) now=\(CMClockGetTime(CMClockGetHostTimeClock()).seconds)"
            + " holder=\(NSStringFromRect(holder.frame)) screen=\(holder.screen.map { NSStringFromRect($0.frame) } ?? "none") \(occluded) onScreen=\(holder.isVisible) level=\(holder.level.rawValue)"
            + " above=\(windowsAbove()) layer=\(NSStringFromRect(layer.frame)) panel=\(panel.map { NSStringFromRect($0.frame) } ?? "none")@\(panelFound)"
            + " possible=\(controller?.isPictureInPicturePossible ?? false) active=\(controller?.isPictureInPictureActive ?? false)"
    }

    /// The app's on-screen windows ordered above the holder (window numbers, front first).
    private func windowsAbove() -> String {
        let order = NSWindow.windowNumbers(options: []) ?? []
        guard let at = order.firstIndex(of: NSNumber(value: holder.windowNumber)) else { return "-" }
        return order[..<at].map { "\($0)" }.joined(separator: ",")
    }

    private func say(_ text: String) {
        guard !invalid else { return }
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { callback(context, $0.baseAddress, $0.count) }
    }

    /// The source's next picture: an absolute file path.
    func frame(_ path: String) {
        guard !invalid else { return }
        if decoding { waiting = path; return }
        decoding = true
        let keep = pixels
        Self.decoder.async { [weak self] in
            let buffer = Self.sample(path, keeping: keep)
            DispatchQueue.main.async {
                guard let self else { return }
                self.decoding = false
                if buffer == nil { self.log("decode failed \(path)") }
                if let buffer, !self.invalid { self.show(buffer) }
                // The first picture opens the window: one that cannot be read fails the request. A stream
                // deletes a picture once it names the next, sometimes before it is read here, so a file
                // that is gone fails the request only when no picture opens the window within a second.
                else if buffer == nil, !self.started, self.waiting == nil {
                    if FileManager.default.fileExists(atPath: path) { self.say("error the picture could not be read") }
                    else {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                            if let self, !self.started { self.say("error the picture could not be read") }
                        }
                    }
                }
                if let next = self.waiting { self.waiting = nil; self.frame(next) }
            }
        }
    }

    private func show(_ buffer: CMSampleBuffer) {
        if layer.sampleBufferRenderer.status == .failed { log("flush after failure " + snapshot()); layer.sampleBufferRenderer.flush() }
        layer.sampleBufferRenderer.enqueue(buffer)
        if let format = CMSampleBufferGetFormatDescription(buffer) {
            let size = CMVideoFormatDescriptionGetDimensions(format)
            pixels = (Int(size.width), Int(size.height))
        }
        enqueued += 1
        let time = CMSampleBufferGetPresentationTimeStamp(buffer)
        if !firstTime.isValid { firstTime = time; log("first enqueue inWindow=\(layer.superlayer != nil && holder.isVisible) " + snapshot()) }
        lastTime = time
        guard !started else { return }
        started = true
        // The panel opens at the layer's aspect: the first picture's.
        if let format = CMSampleBufferGetFormatDescription(buffer) {
            let size = CMVideoFormatDescriptionGetDimensions(format)
            let scale = min(1, 640 / Double(max(size.width, size.height, 1)))
            fit(NSSize(width: Double(size.width) * scale, height: Double(size.height) * scale))
        }
        holder.orderFront(nil)
        let controller = AVPictureInPictureController(contentSource: .init(sampleBufferDisplayLayer: layer, playbackDelegate: self))
        controller.delegate = self
        self.controller = controller
        // A new controller reports possible on the next turn.
        DispatchQueue.main.async { [weak self] in self?.start(tries: 20) }
    }

    private func start(tries: Int) {
        guard !invalid, let controller else { return }
        if controller.isPictureInPicturePossible { log("start after \(20 - tries) waits " + snapshot()); controller.startPictureInPicture(); return }
        if tries == 0 { log("never possible " + snapshot()); say("error picture in picture is not possible"); return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.start(tries: tries - 1) }
    }

    private func fit(_ size: NSSize) {
        guard size.width > 0, size.height > 0 else { return }
        holder.setContentSize(size)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        layer.frame = CGRect(origin: .zero, size: size)
        CATransaction.commit()
    }

    /// The layer follows the panel's content size, which the person may drag.
    private func follow() {
        if panel == nil, let found = NSApp.windows.first(where: { String(describing: type(of: $0)).contains("PIPPanel") }) {
            panel = found
            panelFound = CACurrentMediaTime() - born
            log("panel found " + snapshot())
            resized = NotificationCenter.default.addObserver(forName: NSWindow.didResizeNotification, object: found, queue: .main) { [weak self] _ in self?.follow() }
        }
        if let size = panel?.contentView?.bounds.size { fit(size) }
    }

    func invalidate() {
        log("invalidate " + snapshot())
        traceTimer?.invalidate()
        invalid = true
        if let resized { NotificationCenter.default.removeObserver(resized) }
        if controller?.isPictureInPictureActive == true { controller?.stopPictureInPicture() }
        controller = nil
        layer.sampleBufferRenderer.flush(removingDisplayedImage: true)
        holder.close()
    }

    // A live source: every time is now, and it never pauses.
    func pictureInPictureController(_ c: AVPictureInPictureController, setPlaying playing: Bool) {}
    func pictureInPictureControllerTimeRangeForPlayback(_ c: AVPictureInPictureController) -> CMTimeRange {
        CMTimeRange(start: .negativeInfinity, duration: .positiveInfinity)
    }
    func pictureInPictureControllerIsPlaybackPaused(_ c: AVPictureInPictureController) -> Bool { false }
    func pictureInPictureController(_ c: AVPictureInPictureController, didTransitionToRenderSize size: CMVideoDimensions) { follow() }
    func pictureInPictureController(_ c: AVPictureInPictureController, skipByInterval interval: CMTime) async {}

    func pictureInPictureControllerDidStartPictureInPicture(_ c: AVPictureInPictureController) {
        log("did start " + snapshot())
        follow()
        // The panel settles its size after its opening animation.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { [weak self] in self?.follow() }
        say("enter")
    }
    func pictureInPictureController(_ c: AVPictureInPictureController, failedToStartPictureInPictureWithError error: Error) {
        log("failed to start \(error)")
        say("error \(error.localizedDescription)")
    }
    /// The panel's button that returns to the app: the app comes forward, as a browser's tab does.
    func pictureInPictureController(_ c: AVPictureInPictureController, restoreUserInterfaceForPictureInPictureStopWithCompletionHandler done: @escaping (Bool) -> Void) {
        NSApp.activate(ignoringOtherApps: true)
        done(true)
    }
    /// The panel's own close brings the app forward, as a press on Chromium's window, which is
    /// the browser's own, activates the browser. `exitPictureInPicture()` and the image's removal
    /// end the window from the app, which `invalid` marks, and leave the app where it is.
    func pictureInPictureControllerDidStopPictureInPicture(_ c: AVPictureInPictureController) {
        holder.orderOut(nil)
        if !invalid { NSApp.activate(ignoringOtherApps: true) }
        say("leave")
    }

    /// The file's picture as a BGRA sample buffer shown as soon as it is enqueued, drawn at
    /// `keep` when its aspect is within 2% of that size's.
    private static func sample(_ path: String, keeping keep: (width: Int, height: Int)?) -> CMSampleBuffer? {
        autoreleasepool {
            let options = [kCGImageSourceShouldCache: false] as CFDictionary
            guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, options),
                  let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                      kCGImageSourceCreateThumbnailFromImageAlways: true,
                      kCGImageSourceCreateThumbnailWithTransform: true,
                      kCGImageSourceThumbnailMaxPixelSize: maxPixel] as CFDictionary) else { return nil }
            var (width, height) = (image.width, image.height)
            if let keep, abs(Double(width * keep.height) / Double(height * keep.width) - 1) < 0.02 { (width, height) = keep }
            var pixels: CVPixelBuffer?
            let attributes = [kCVPixelBufferIOSurfacePropertiesKey: [:]] as CFDictionary
            guard CVPixelBufferCreate(nil, width, height, kCVPixelFormatType_32BGRA, attributes, &pixels) == kCVReturnSuccess,
                  let pixels else { return nil }
            CVPixelBufferLockBaseAddress(pixels, [])
            let drawn = CGContext(data: CVPixelBufferGetBaseAddress(pixels), width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: CVPixelBufferGetBytesPerRow(pixels), space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
            drawn?.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            CVPixelBufferUnlockBaseAddress(pixels, [])
            // The pixels were drawn in sRGB; untagged, the layer would read them in the display's space.
            CVBufferSetAttachment(pixels, kCVImageBufferCGColorSpaceKey, CGColorSpace(name: CGColorSpace.sRGB)!, .shouldPropagate)
            guard drawn != nil else { return nil }
            var format: CMVideoFormatDescription?
            CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: pixels, formatDescriptionOut: &format)
            guard let format else { return nil }
            var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()), decodeTimeStamp: .invalid)
            var buffer: CMSampleBuffer?
            CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: pixels, formatDescription: format, sampleTiming: &timing, sampleBufferOut: &buffer)
            if let buffer, let attachments = CMSampleBufferGetSampleAttachmentsArray(buffer, createIfNecessary: true) as? [NSMutableDictionary] {
                attachments.first?[kCMSampleAttachmentKey_DisplayImmediately] = true
            }
            return buffer
        }
    }
}

private func pip(_ raw: UnsafeMutableRawPointer?) -> PictureInPictureArm? { raw.map { Unmanaged<PictureInPictureArm>.fromOpaque($0).takeUnretainedValue() } }
#endif

/// A floating window for one image; nil where the platform's window cannot show it yet
/// (iOS and tvOS, whose picture in picture needs the app's playback audio session).
@_cdecl("exact_pip_create")
public func pictureInPictureCreate(_ context: UnsafeMutableRawPointer?, _ callback: VideoCallback?) -> UnsafeMutableRawPointer? {
    #if os(macOS)
    guard let callback, AVPictureInPictureController.isPictureInPictureSupported() else { return nil }
    return Unmanaged.passRetained(PictureInPictureArm(context: context, callback: callback)).toOpaque()
    #else
    return nil
    #endif
}
@_cdecl("exact_pip_frame")
public func pictureInPictureFrame(_ raw: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ count: Int) {
    #if os(macOS)
    guard let bytes else { return }
    pip(raw)?.frame(String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self))
    #endif
}
@_cdecl("exact_pip_destroy")
public func pictureInPictureDestroy(_ raw: UnsafeMutableRawPointer?) {
    #if os(macOS)
    guard let raw else { return }
    Unmanaged<PictureInPictureArm>.fromOpaque(raw).takeRetainedValue().invalidate()
    #endif
}
