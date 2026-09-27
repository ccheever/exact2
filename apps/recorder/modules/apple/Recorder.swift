// The recorder on Apple (LLP 1067.000): one object, the microphone, shared
// by `<waveform-view>` and `later`. The view draws the recorder's own levels
// every frame; the source only starts, stops and lists takes. The host makes
// one Recorder per session and calls it on the main thread (Q5).
//
// Under the agent the microphone is substituted before the OS is asked
// (Q7): a synthetic level on the session's clock, which is the agent's, so a
// screenshot after `clock +N` is the same every run and the web's; takes stay
// in memory. Otherwise `AVAudioEngine` taps the input, and takes are kept in
// the app's data directory, as `recorder/<name>.caf` beside
// `recorder/takes.json`.
import AVFoundation
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class Recorder: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["waveform-view": ExactNativeFactory(snapshot: true) { module, _, events in WaveformView(recorder: module as! Recorder, events: events) }]
    }

    static let rate = 20.0 // levels a second
    static let keep = 20 * 60 * 10 // ten minutes of levels

    private(set) var recording = false
    private var started = 0.0
    private var levels: [Double] = [] // real input, one per 1/rate s
    private(set) var last: [Double] = [] // the finished take's, shown when idle
    private var memory: [[String: Any]] = [] // the agent's takes
    private var engine: AVAudioEngine?
    private var file: AVAudioFile?
    private var fileName = ""

    // A level for step `i` of the synthetic input: the web module's formula.
    private static func synthetic(_ i: Int) -> Double {
        let x = Double(i)
        return min(1, 0.12 + 0.75 * abs(sin(x * 0.37)) * (0.55 + 0.45 * sin(x * 0.05)))
    }

    var elapsed: Double { recording ? max(0, context.now() - started) : 0 }

    /// The levels to draw: the live take's while recording, else the last one's.
    func visible() -> [Double] {
        guard recording else { return last }
        guard context.agent else { return levels }
        let steps = Int(elapsed * Recorder.rate / 1000)
        return (max(0, steps - Recorder.keep)..<steps).map(Recorder.synthetic)
    }

    private func status(_ message: String? = nil) -> [String: Any] {
        let text = message ?? (recording ? "Recording…" : context.agent ? "Ready (agent input)" : "Ready")
        return ["available": true, "recording": recording, "message": text]
    }

    override func later(_ request: [String: Any], reply: ExactReply) {
        switch request["op"] as? String {
        case "status": reply.send(status())
        case "takes": reply.send(["takes": list().map { ["name": $0["name"] as? String ?? "", "label": label($0)] }])
        case "start": start(reply)
        case "stop":
            do { reply.send(try stop()) } catch { reply.fail(String(describing: error)) }
        case let op: reply.fail("the recorder answers no \(op.map { "\"\($0)\"" } ?? "null")")
        }
    }

    private func start(_ reply: ExactReply) {
        if recording { return reply.send(status()) }
        if context.agent { return begin(reply) }
        permission { granted in
            DispatchQueue.main.async {
                guard granted else { return reply.fail("microphone access was refused") }
                do { try self.open() } catch { return reply.fail("the microphone did not open: \(error)") }
                self.begin(reply)
            }
        }
    }

    private func begin(_ reply: ExactReply) {
        started = context.now()
        recording = true
        context.changed("status")
        reply.send(status())
    }

    private func permission(_ done: @escaping (Bool) -> Void) {
        #if os(macOS)
        AVCaptureDevice.requestAccess(for: .audio, completionHandler: done)
        #else
        AVAudioApplication.requestRecordPermission(completionHandler: done)
        #endif
    }

    private func open() throws {
        #if os(iOS)
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.record, mode: .default)
        try session.setActive(true)
        #endif
        let engine = AVAudioEngine()
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        let folder = try self.folder()
        fileName = "take-\(list().count + 1).caf"
        file = try AVAudioFile(forWriting: folder.appendingPathComponent(fileName), settings: format.settings)
        levels = []
        let window = max(1, Int(format.sampleRate / Recorder.rate))
        var peak: Float = 0, counted = 0
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            try? self?.file?.write(from: buffer)
            guard let samples = buffer.floatChannelData?[0] else { return }
            var out: [Double] = []
            for i in 0..<Int(buffer.frameLength) {
                peak = max(peak, abs(samples[i])); counted += 1
                if counted >= window { out.append(min(1, Double(peak) * 1.4)); peak = 0; counted = 0 }
            }
            guard !out.isEmpty else { return }
            DispatchQueue.main.async {
                guard let self, self.recording else { return }
                self.levels.append(contentsOf: out)
                if self.levels.count > Recorder.keep { self.levels.removeFirst(self.levels.count - Recorder.keep) }
            }
        }
        try engine.start()
        self.engine = engine
    }

    private func stop() throws -> [String: Any] {
        guard recording else { throw ExactNativeRefusal("not recording") }
        let ms = elapsed
        last = visible()
        recording = false
        if let engine {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
            self.engine = nil
            file = nil
            #if os(iOS)
            try? AVAudioSession.sharedInstance().setActive(false)
            #endif
        }
        var takes = list()
        let name = "take-\(takes.count + 1)"
        let take: [String: Any] = ["name": name, "seconds": (ms / 100).rounded() / 10]
        takes.append(take)
        try save(takes)
        context.changed("status")
        context.changed("takes")
        return ["name": name, "message": "Saved \(label(take))"]
    }

    private func label(_ take: [String: Any]) -> String {
        let name = (take["name"] as? String ?? "").replacingOccurrences(of: "take-", with: "Take ")
        return "\(name) · \(String(format: "%.1f", take["seconds"] as? Double ?? 0)) s"
    }

    private func folder() throws -> URL {
        let url = context.data.appendingPathComponent("recorder", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func list() -> [[String: Any]] {
        if context.agent { return memory }
        guard let url = try? folder().appendingPathComponent("takes.json"), let data = try? Data(contentsOf: url),
              let takes = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [] }
        return takes
    }

    private func save(_ takes: [[String: Any]]) throws {
        if context.agent { memory = takes; return }
        try JSONSerialization.data(withJSONObject: takes).write(to: folder().appendingPathComponent("takes.json"), options: .atomic)
    }

    override func destroy() {
        engine?.inputNode.removeTap(onBus: 0)
        engine?.stop()
        engine = nil
    }
}

// `<waveform-view>`: draws the recorder's levels, newest at the right, and
// the elapsed time. It holds the session's Recorder and reads it every frame.
final class WaveformView: ExactNativeInstance {
    private let canvas: Canvas

    init(recorder: Recorder, events: ExactNativeEvents) {
        canvas = Canvas(recorder: recorder)
        super.init(events: events)
    }

    override var view: ExactNativeView { canvas }
    override func destroy() { canvas.stop() }

    // A capture asks for the recorder as it is now: the layer's contents may
    // be a frame behind the agent's clock (iOS captures layers, not draws).
    override func snapshot() throws -> Data {
        let size = canvas.bounds.size
        guard size.width > 0, size.height > 0 else { throw ExactNativeRefusal("no bounds yet") }
        let scale = 2.0, w = Int(size.width * scale), h = Int(size.height * scale)
        guard let g = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { throw ExactNativeRefusal("no context") }
        g.translateBy(x: 0, y: CGFloat(h))
        g.scaleBy(x: scale, y: -scale)
        #if os(macOS)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: g, flipped: true)
        canvas.paint(g, size)
        NSGraphicsContext.restoreGraphicsState()
        guard let image = g.makeImage(), let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])
        else { throw ExactNativeRefusal("no PNG") }
        #else
        UIGraphicsPushContext(g)
        canvas.paint(g, size)
        UIGraphicsPopContext()
        guard let image = g.makeImage(), let png = UIImage(cgImage: image).pngData() else { throw ExactNativeRefusal("no PNG") }
        #endif
        return png
    }
}

#if os(macOS)
typealias PlatformColor = NSColor
#else
typealias PlatformColor = UIColor
#endif

private final class Canvas: ExactNativeView {
    private let recorder: Recorder
    private var link: CADisplayLink?

    init(recorder: Recorder) {
        self.recorder = recorder
        super.init(frame: .zero)
        #if os(macOS)
        wantsLayer = true
        link = displayLink(target: self, selector: #selector(tick))
        #else
        isUserInteractionEnabled = false
        contentMode = .redraw
        isOpaque = false
        link = CADisplayLink(target: self, selector: #selector(tick))
        #endif
        link?.add(to: .main, forMode: .common)
    }

    required init?(coder: NSCoder) { fatalError("not from a coder") }

    #if os(macOS)
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    #endif

    @objc private func tick() {
        #if os(macOS)
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }

    func stop() { link?.invalidate(); link = nil }

    private func color(_ hex: UInt32) -> PlatformColor {
        PlatformColor(red: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: 1)
    }

    override func draw(_ rect: CGRect) {
        #if os(macOS)
        guard let g = NSGraphicsContext.current?.cgContext else { return }
        #else
        guard let g = UIGraphicsGetCurrentContext() else { return }
        #endif
        paint(g, bounds.size)
    }

    /// The picture, y down, into a context that is also the current one
    /// (the elapsed time draws as text).
    func paint(_ g: CGContext, _ size: CGSize) {
        let w = size.width, h = size.height, mid = h / 2
        let background = CGPath(roundedRect: CGRect(origin: .zero, size: size), cornerWidth: 12, cornerHeight: 12, transform: nil)
        g.addPath(background)
        g.setFillColor(color(0x0b0f13).cgColor)
        g.fillPath()
        let bar = 3.0, gap = 2.0
        let fit = max(0, Int((w - 16) / (bar + gap)))
        let shown = recorder.visible().suffix(fit)
        g.setFillColor(color(recorder.recording ? 0xe5484d : 0x2f81f7).cgColor)
        for (i, level) in shown.enumerated() {
            let x = w - 8 - CGFloat(shown.count - i) * (bar + gap)
            let half = max(1, CGFloat(level) * (mid - 12))
            g.fill(CGRect(x: x, y: mid - half, width: bar, height: half * 2))
        }
        if shown.isEmpty {
            g.setFillColor(color(0x2a333d).cgColor)
            g.fill(CGRect(x: 8, y: mid - 0.5, width: w - 16, height: 1))
        }
        guard recorder.recording else { return }
        let s = recorder.elapsed / 1000
        let text = String(format: "● %d:%04.1f", Int(s / 60), s.truncatingRemainder(dividingBy: 60))
        let attributes: [NSAttributedString.Key: Any] = [.font: PlatformFont.systemFont(ofSize: 13), .foregroundColor: color(0xe8edf2)]
        (text as NSString).draw(at: CGPoint(x: 10, y: 8), withAttributes: attributes)
    }
}

#if os(macOS)
typealias PlatformFont = NSFont
#else
typealias PlatformFont = UIFont
#endif

let exactModule: ExactModule.Type = Recorder.self
