// A live scan (LLP 1133.027 D5): an observer of a video track that takes no
// lease. While a consumer's lease runs the capture, the scan's
// `AVCaptureVideoDataOutput` keeps only the newest frame, and each poll hands
// that one frame to Vision; one still in Vision when the next poll comes is
// waited for, not queued behind. Under the agent no capture exists and the
// frame is the camera's agent picture (D17 item 2).
import AVFoundation
import ExactPackage
import Foundation
import Vision

#if !os(tvOS)
final class Scan: NSObject, ExactTrackConsumer, AVCaptureVideoDataOutputSampleBufferDelegate {
    /// How many distinct codes `seen` keeps, the newest `lastSeen` first.
    static let kept = 32
    static func key(_ track: String, _ keep: Set<String>?) -> String { "\(track)|\(keep.map { $0.sorted().joined(separator: ",") } ?? "*")" }

    private weak var module: BarcodeDetectionModule?
    let track: String, keep: Set<String>?, interval: Double
    private var subscription: ExactSubscription?
    private var state = "idle", frame = 0, width = 0.0, height = 0.0, error: String?
    private var barcodes: [[String: Any]] = []
    /// Distinct codes by `format` and value, in the order last seen.
    private var seen: [(key: String, value: [String: Any])] = []
    private var busy: [([String: Any]) -> Void]?
    // The capture's output while it runs, and the newest frame it delivered (its queue's, under `lock`).
    private var source: ExactVideoSource?
    private var output: AVCaptureVideoDataOutput?
    private let frames = DispatchQueue(label: "exact.barcode-detection.frames")
    private let lock = NSLock()
    private var newest: CVPixelBuffer?

    init(module: BarcodeDetectionModule, track: String, keep: Set<String>?, interval: Double) {
        self.module = module; self.track = track; self.keep = keep; self.interval = interval
    }

    func start() throws { subscription = try module?.context.tracks.observe(track, self) }

    func stop() {
        subscription?.cancel(); subscription = nil
        detach()
    }

    // MARK: the track

    func live(video source: ExactVideoSource?) {
        if source !== self.source { detach(); if let source { attach(source) } }
        set("scanning")
    }
    func muted(_ reason: String) { set("idle") }
    func unmuted() { set("scanning") }
    func ended() { detach(); set("ended") }

    private func set(_ next: String) {
        guard state != next else { return }
        state = next
        module?.context.changed("barcode-detection.detectedBarcodes")
    }

    private func attach(_ source: ExactVideoSource) {
        self.source = source
        let output = AVCaptureVideoDataOutput()
        output.alwaysDiscardsLateVideoFrames = true
        output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange]
        output.setSampleBufferDelegate(self, queue: frames)
        self.output = output
        let device = source.device
        source.configure { session in
            guard session.canAddOutput(output) else { return }
            session.addOutput(output)
            // The frame upright as the viewfinder shows it (D5): the angle a level capture takes.
            if let connection = output.connection(with: .video) {
                let angle = AVCaptureDevice.RotationCoordinator(device: device, previewLayer: nil).videoRotationAngleForHorizonLevelCapture
                if connection.isVideoRotationAngleSupported(angle) { connection.videoRotationAngle = angle }
            }
            BarcodeDetectionModule.videoOutputHatch?(output)
        }
    }

    private func detach() {
        guard let source, let output else { return }
        self.source = nil; self.output = nil
        output.setSampleBufferDelegate(nil, queue: nil)
        source.configure { session in session.removeOutput(output) }
        lock.lock(); newest = nil; lock.unlock()
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let buffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        lock.lock(); newest = buffer; lock.unlock()
    }

    // MARK: analysis

    /// One analysed frame, the newest, then the scan's answer.
    func analyse(_ reply: @escaping ([String: Any]) -> Void) {
        guard let module else { return reply(["error": "InvalidStateError"]) }
        if busy != nil { busy?.append(reply); return }
        guard state == "scanning" else { return reply(answer()) }
        let handler: VNImageRequestHandler, w: Double, h: Double
        if module.context.agent {
            guard let picture = module.context.tracks.lookup(track)?.picture else { return reply(answer()) }
            handler = VNImageRequestHandler(cgImage: picture, orientation: .up)
            w = Double(picture.width); h = Double(picture.height)
        } else {
            lock.lock(); let buffer = newest; newest = nil; lock.unlock()
            guard let buffer else { return reply(answer()) }
            handler = VNImageRequestHandler(cvPixelBuffer: buffer, orientation: .up)
            w = Double(CVPixelBufferGetWidth(buffer)); h = Double(CVPixelBufferGetHeight(buffer))
        }
        busy = [reply]
        let keep = keep
        module.work.async { [weak self, weak module] in
            let found: Result<[[String: Any]], Error> = Result { try module?.detect(handler, width: w, height: h, keep: keep ?? [], all: keep == nil) ?? [] }
            DispatchQueue.main.async {
                guard let self else { return }
                self.took(found, width: w, height: h)
                let waiting = self.busy ?? []
                self.busy = nil
                let answer = self.answer()
                for r in waiting { r(answer) }
            }
        }
    }

    private func took(_ found: Result<[[String: Any]], Error>, width w: Double, height h: Double) {
        switch found {
        case .failure(let e):
            // One frame Vision fails on is the answer's `error`; the next clears it (D2).
            error = e.localizedDescription
        case .success(let codes):
            let at = module?.context.now() ?? 0
            frame += 1; width = w; height = h; barcodes = codes; error = nil
            for b in codes {
                let format = b["format"] as? String ?? "unknown", raw = b["rawValue"] as? String ?? ""
                let key = "\(format)\u{0}\(raw)"
                let was = seen.first { $0.key == key }?.value
                seen.removeAll { $0.key == key }
                seen.append((key, ["rawValue": raw, "format": format, "firstSeen": was?["firstSeen"] ?? at, "lastSeen": at,
                                   "frames": ((was?["frames"] as? Int) ?? 0) + 1]))
            }
            if seen.count > Self.kept { seen.removeFirst(seen.count - Self.kept) }
        }
    }

    private func answer() -> [String: Any] {
        var a: [String: Any] = ["state": state, "barcodes": barcodes, "seen": seen.reversed().map(\.value), "frame": frame, "width": width, "height": height]
        if let error { a["error"] = error }
        return a
    }
}
#endif
