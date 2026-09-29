// @ref LLP 1056 D9 — Canvas 2D images and pixels on Apple, and the
// presenter's canvases. An image handle is the string an `image` node's
// `src` takes: an app asset, or an http(s) URL. The runner names the handles
// its draws asked for (the batch's `canvasImages`); the host decodes each
// off the main thread and answers `exact_canvas_image`, which redraws the
// canvases that asked. `putImageData` writes raw backing pixels: no
// transform, clip, alpha, compositing or shadow.
import CoreGraphics
import CoreText
import Foundation
import ImageIO
import QuartzCore

/// No implicit animations: new pixels appear with the batch that drew them.
private final class Instant: NSObject, CALayerDelegate {
    static let shared = Instant()
    func action(for layer: CALayer, forKey event: String) -> CAAction? { NSNull() }
}

extension Canvas2DReplayer {
    /// `drawImage`: `image sx sy sw sh dx dy dw dh`, the source already
    /// clipped to the image; the destination in user space.
    func drawImage(_ c: CGContext, _ n: [Double]) {
        guard let src = imageSources[UInt32(n[0])], let image = env?.canvasImage(src) else { return }
        let (sx, sy, sw, sh) = (n[1], n[2], n[3], n[4])
        let dest = CGRect(x: n[5], y: n[6], width: n[7], height: n[8])
        guard sw > 0, sh > 0 else { return }
        let (kx, ky) = (dest.width / sw, dest.height / sh)
        let whole = CGRect(x: dest.minX - sx * kx, y: dest.minY - sy * ky,
                           width: CGFloat(image.width) * kx, height: CGFloat(image.height) * ky)
        render(c) { c in
            c.concatenate(state.author)
            c.clip(to: dest)
            // Upright in the y-down space.
            c.translateBy(x: 0, y: whole.minY + whole.maxY)
            c.scaleBy(x: 1, y: -1)
            c.draw(image, in: whole)
        }
    }

    /// `putImageData`: `x y w h` in backing pixels, then w × h RGBA pixels.
    func putImageData(_ c: CGContext, _ n: [Double], _ count: Int) {
        let (x, y, w, h) = (Int(n[0]), Int(n[1]), Int(n[2]), Int(n[3]))
        guard w > 0, h > 0, count >= 4 + w * h else { return }
        var bytes = [UInt8](repeating: 0, count: w * h * 4)
        for i in 0..<(w * h) {
            let v = UInt32(n[4 + i])
            bytes[i * 4] = UInt8(v >> 24); bytes[i * 4 + 1] = UInt8((v >> 16) & 0xff)
            bytes[i * 4 + 2] = UInt8((v >> 8) & 0xff); bytes[i * 4 + 3] = UInt8(v & 0xff)
        }
        guard let provider = CGDataProvider(data: Data(bytes) as CFData),
              let image = CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: w * 4, space: canvas2DSRGB,
                                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue), provider: provider,
                                  decode: nil, shouldInterpolate: false, intent: .defaultIntent) else { return }
        c.saveGState()
        c.resetClip()
        c.concatenate(c.ctm.inverted())
        c.setBlendMode(.copy)
        c.setAlpha(1)
        c.setShadow(offset: .zero, blur: 0, color: nil)
        c.interpolationQuality = .none
        c.draw(image, in: CGRect(x: x, y: height - y - h, width: w, height: h))
        c.restoreGState()
    }
}

/// A presenter's 2D canvases, by view id: the `canvas2d` op, the decoded
/// image handles they draw, and cleanup when a view goes. The bitmap shows
/// in a sublayer below the view's children, framed to the content box.
final class Canvas2DHost: Canvas2DEnv {
    /// The replayers, by view id: touched only on `replay`. A canvas that is
    /// recorded (`Canvas2DRecord.swift`) has none.
    private var replayers: [UInt32: Canvas2DReplayer] = [:]
    /// Each canvas's tracker and kept lists: touched only on `replay`.
    private var kept: [UInt32: Canvas2DKept] = [:]
    /// Each recorded canvas's recording layer, over its bitmap.
    private var recorders: [UInt32: Canvas2DRecordLayer] = [:]
    /// Tests (and the parity smoke, EXACT_CANVAS_RECORD=always) record every
    /// canvas the policy allows, animating or not.
    static var recordAlways = ProcessInfo.processInfo.environment["EXACT_CANVAS_RECORD"] == "always"
    private var layers: [UInt32: CALayer] = [:]
    /// Replay runs off the main thread, in order (LLP 1056 §8 stage 4, as
    /// built): a list row's canvas cost 130 ms/s of an iPhone's main thread
    /// in a fling. The row's box, mask and scale apply with its batch; its
    /// pixels land when the replay ends, a frame later at most, and the
    /// agent's settle waits for them (`loadingCount`).
    ///
    /// A canvas has at most one replay waiting behind the one running. Lists
    /// that arrive while one waits join it (one replay, one image), and a
    /// fresh bitmap's lists replace it, since a new bitmap discards what was
    /// drawn before. While one waits the runner holds the canvas's frame
    /// request (`onHeld`), as a browser's animation frame waits for the last
    /// one to present: an animation drops frames instead of queueing them,
    /// and the newest replay done always shows.
    private let replay = DispatchQueue(label: "exact.canvas2d.replay", qos: .userInitiated)
    /// Replays dispatched and not yet shown.
    private var pending = 0
    /// Each canvas's latest arrival, and the newest one shown: an older
    /// replay's pixels are never shown over a newer one's.
    private var sequence: [UInt32: Int] = [:]
    private var shown: [UInt32: Int] = [:]
    /// Each canvas's replay not yet started, under `lock` (the replay queue
    /// takes it).
    private var waiting: [UInt32: Canvas2DJob] = [:]
    private let lock = NSLock()
    /// A canvas's replay is waiting (true) or has started (false): the
    /// runner holds its frame request meanwhile (`exact_canvas_held`).
    var onHeld: ((UInt32, Bool) -> Void)?
    /// Lists that could not be read, for `logs`.
    var errors: [String] = []
    /// The views' real scale differs from what the canvases were drawn at.
    var onScale: ((CGFloat) -> Void)?
    private var reported: CGFloat = 0
    /// The session's text engine (LLP 1056 D8).
    var textEngine: (() -> TextEngine?)?
    /// An app asset's bytes.
    var assetBytes: ((String) -> Data?)?
    /// A handle decoded (its image) or not (nil): the session tells the runtime.
    var onImage: ((String, CGImage?) -> Void)?
    private var images: [String: CGImage] = [:]
    /// Every font a list has set, as last resolved.
    private var resolved: [Canvas2DFont: CTFont] = [:]
    private var loading: Set<String> = []

    func canvasFont(_ f: Canvas2DFont) -> CTFont? { textEngine?()?.canvasText.font(f) }
    func canvasImage(_ src: String) -> CGImage? { images[src] }
    /// Handles being decoded and replays not yet shown: the agent's settle
    /// waits for them.
    var loadingCount: Int { loading.count + pending }

    /// Decode the handles the runner asked for, off the main thread.
    func load(_ srcs: [String]) {
        for src in srcs where images[src] == nil && !loading.contains(src) {
            loading.insert(src)
            let remote = URL(string: src).flatMap { $0.scheme == "http" || $0.scheme == "https" ? $0 : nil }
            let local = remote == nil ? assetBytes?(src.hasPrefix("/") ? String(src.dropFirst()) : src) : nil
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                var bytes = local
                if let remote {
                    let done = DispatchSemaphore(value: 0)
                    var request = URLRequest(url: remote); request.timeoutInterval = 20
                    URLSession.shared.dataTask(with: request) { data, response, _ in
                        if let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) { bytes = data }
                        done.signal()
                    }.resume()
                    done.wait()
                }
                let image = bytes.flatMap { CGImageSourceCreateWithData($0 as CFData, nil) }
                    .flatMap { CGImageSourceCreateImageAtIndex($0, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary) }
                DispatchQueue.main.async {
                    guard let self else { return }
                    self.loading.remove(src)
                    if let image { self.images[src] = image }
                    self.onImage?(src, image)
                }
            }
        }
    }

    func apply(_ id: UInt32, _ payload: [String: Any], layer parent: CALayer?) {
        guard let parent else { return }
        let num = { (key: String) -> Double in (payload[key] as? NSNumber)?.doubleValue ?? 0 }
        let lifetime = UInt64(num("lifetime")), generation = UInt32(num("generation"))
        let fresh = (payload["fresh"] as? NSNumber)?.boolValue == true
        let (w, h, scale) = (Int(num("w")), Int(num("h")), num("scale"))
        let lists: [Data?] = payload["lists"] as? [Data] ?? []
        let layer = layers[id] ?? {
            let l = CALayer(); l.delegate = Instant.shared; l.contentsGravity = .resize
            l.magnificationFilter = .linear; l.minificationFilter = .linear
            layers[id] = l; return l
        }()
        if layer.superlayer !== parent {
            #if os(macOS)
            // Replaced content paints over its box: above the box's own
            // sublayers where AppKit's box is layers (`BoxLayerMac.swift`).
            if let node = parent.delegate as? NodeView, let top = node.boxSublayersTop { parent.insertSublayer(layer, above: top) }
            else { parent.insertSublayer(layer, at: 0) }
            #else
            parent.insertSublayer(layer, at: 0)
            #endif
        }
        let box = (payload["box"] as? [Any])?.compactMap { ($0 as? NSNumber)?.doubleValue } ?? []
        if box.count == 4 { layer.frame = CGRect(x: box[0], y: box[1], width: box[2], height: box[3]) }
        // A rounded canvas clips its bitmap to the content edge's curve, as
        // the web clips replaced content.
        let radii = (payload["radii"] as? [Any])?.compactMap { ($0 as? NSNumber).map { CGFloat($0.doubleValue) } } ?? []
        if radii.count == 4, radii.contains(where: { $0 > 0 }) {
            let mask = (layer.mask as? CAShapeLayer) ?? CAShapeLayer()
            mask.path = Canvas2DHost.rounded(CGRect(origin: .zero, size: layer.bounds.size), radii)
            layer.mask = mask
        } else {
            layer.mask = nil
        }
        layer.contentsScale = max(1, scale)
        let actual = parent.contentsScale
        if (payload["stretch"] as? NSNumber)?.boolValue != true, actual >= 1, abs(actual - scale) > 0.01, actual != reported {
            reported = actual
            onScale?(actual)
        }
        // What the replay reads, resolved here: the decoded images, and the
        // fonts its text sets (the text engine is the main thread's).
        // A list sets a font only when it changes, and the replayer keeps it
        // from list to list: every font resolved before rides along, and the
        // ones these lists set are resolved again.
        var again: Set<Canvas2DFont> = []
        for case let data? in lists {
            for f in Canvas2DReplayer.fonts(in: data) where again.insert(f).inserted { resolved[f] = canvasFont(f) }
        }
        let fonts = resolved
        let seq = (sequence[id] ?? 0) + 1
        sequence[id] = seq
        let job = Canvas2DJob(fresh: fresh, w: w, h: h, scale: scale, lifetime: lifetime, generation: generation,
                              lists: lists, images: images, fonts: fonts, seq: seq)
        // The draw asked for the next frame: it animates.
        job.animating = (payload["animating"] as? NSNumber)?.boolValue == true || Canvas2DHost.recordAlways
        job.stretch = (payload["stretch"] as? NSNumber)?.boolValue == true
        lock.lock()
        let joined = waiting[id]?.absorb(job) ?? false
        if !joined { waiting[id] = job }
        lock.unlock()
        if joined { return }
        pending += 1
        onHeld?(id, true)
        replay.async { [weak self] in self?.run(id) }
    }

    /// The replay queue: take `id`'s waiting replay and run it.
    private func run(_ id: UInt32) {
        lock.lock()
        let taken = waiting.removeValue(forKey: id)
        lock.unlock()
        DispatchQueue.main.async { [weak self] in
            // Released unless another replay has come to wait since.
            guard let self else { return }
            self.lock.lock(); let still = self.waiting[id] != nil; self.lock.unlock()
            if !still { self.onHeld?(id, false) }
        }
        guard let job = taken else {
            DispatchQueue.main.async { [weak self] in self?.pending -= 1 }
            return
        }
        if job.fresh {
            replayers[id] = nil
            let t = Canvas2DReplayer(trackingWidth: job.w, height: job.h, scale: job.scale, lifetime: job.lifetime, generation: job.generation)
            kept[id] = Canvas2DKept(t)
        }
        guard let k = kept[id], k.tracker.lifetime == job.lifetime, k.tracker.generation == job.generation else {
            DispatchQueue.main.async { [weak self] in self?.pending -= 1 }
            return
        }
        let env = Canvas2DSnapshot(images: job.images, fonts: job.fonts)
        var unreadable = 0
        let lists = job.lists.compactMap { $0 }
        unreadable += job.lists.count - lists.count
        for data in lists { k.keep(data) }
        // The policy (LLP 1056 §8.4): recorded while it animates, from a
        // cover, drawing nothing a recording draws differently, within the
        // bound, at the display's own scale.
        let record = job.animating && !job.stretch && k.start != nil && !k.refused && k.bounded && job.w > 0 && job.h > 0
        var contents: Any?, recording: Canvas2DRecordLayer.Frame?
        if record, let start = k.start {
            replayers[id] = nil
            k.recording = true
            recording = .init(lists: k.lists, start: start, env: env, width: job.w, height: job.h, scale: job.scale)
        } else {
            let r: Canvas2DReplayer
            if let existing = replayers[id], !k.recording {
                r = existing
                r.env = env
                for data in lists where !r.apply(data) { unreadable += 1 }
            } else {
                // A new bitmap: from the kept lists when it was recorded (or
                // it is fresh), which carry every list since their start.
                r = Canvas2DReplayer(width: job.w, height: job.h, scale: job.scale, lifetime: job.lifetime, generation: job.generation)
                r.env = env
                if let start = k.start {
                    r.restore(start)
                    for data in k.lists where !r.apply(data) { unreadable += 1 }
                } else {
                    for data in lists where !r.apply(data) { unreadable += 1 }
                }
                replayers[id] = r
                k.recording = false
            }
            r.env = nil
            contents = r.image()
        }
        // Kept only while it may be recorded: a canvas that does not animate,
        // or kept past the bound, starts again at its next cover.
        if !k.bounded || (!job.animating && !k.recording) { k.drop() }
        withExtendedLifetime(env) {}
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.pending -= 1
            for _ in 0..<unreadable { self.errors.append("canvas \(id): unreadable list") }
            // The newest replay done of a canvas still mounted shows: the
            // bitmap, or the recording.
            guard self.sequence[id] != nil, job.seq > self.shown[id] ?? 0, let layer = self.layers[id] else { return }
            self.shown[id] = job.seq
            layer.contents = contents
            if let recording {
                let r = self.recorders[id] ?? { let r = Canvas2DRecordLayer(); layer.addSublayer(r); self.recorders[id] = r; return r }()
                r.show(recording)
            } else {
                self.recorders[id]?.hide()
            }
        }
    }

    /// A rectangle with four corner radii (top-left, top-right, bottom-right,
    /// bottom-left), in a y-down layer.
    static func rounded(_ r: CGRect, _ radii: [CGFloat]) -> CGPath {
        let p = CGMutablePath(), (x0, y0, x1, y1) = (r.minX, r.minY, r.maxX, r.maxY)
        let f = min(1, r.width / max(radii[0] + radii[1], radii[2] + radii[3], 1e-9), r.height / max(radii[0] + radii[3], radii[1] + radii[2], 1e-9))
        let (tl, tr, br, bl) = (radii[0] * f, radii[1] * f, radii[2] * f, radii[3] * f)
        p.move(to: CGPoint(x: x0 + tl, y: y0))
        p.addLine(to: CGPoint(x: x1 - tr, y: y0))
        p.addArc(tangent1End: CGPoint(x: x1, y: y0), tangent2End: CGPoint(x: x1, y: y0 + tr), radius: tr)
        p.addLine(to: CGPoint(x: x1, y: y1 - br))
        p.addArc(tangent1End: CGPoint(x: x1, y: y1), tangent2End: CGPoint(x: x1 - br, y: y1), radius: br)
        p.addLine(to: CGPoint(x: x0 + bl, y: y1))
        p.addArc(tangent1End: CGPoint(x: x0, y: y1), tangent2End: CGPoint(x: x0, y: y1 - bl), radius: bl)
        p.addLine(to: CGPoint(x: x0, y: y0 + tl))
        p.addArc(tangent1End: CGPoint(x: x0, y: y0), tangent2End: CGPoint(x: x0 + tl, y: y0), radius: tl)
        p.closeSubpath()
        return p
    }

    /// An agent's picture waits for the replays already dispatched to show.
    func waitForReplays(timeout: TimeInterval = 1) {
        let end = Date(timeIntervalSinceNow: timeout)
        while pending > 0 && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.002)) }
    }

    func forget(_ id: UInt32) {
        // Every retired view is forgotten here; only a canvas has a replayer.
        guard sequence.removeValue(forKey: id) != nil else { return }
        shown.removeValue(forKey: id)
        lock.lock(); waiting.removeValue(forKey: id); lock.unlock()
        recorders.removeValue(forKey: id)
        replay.async { [weak self] in self?.replayers.removeValue(forKey: id); self?.kept.removeValue(forKey: id) }
        layers.removeValue(forKey: id)?.removeFromSuperlayer()
    }
}

/// One canvas's replay not yet started: its lists in order, with what they
/// read.
private final class Canvas2DJob {
    var fresh: Bool
    var w: Int, h: Int, scale: Double, lifetime: UInt64, generation: UInt32
    var lists: [Data?]
    var images: [String: CGImage]
    var fonts: [Canvas2DFont: CTFont]
    var seq: Int
    /// The draw asked for the next frame.
    var animating = false
    /// An explicit bitmap, stretched to the box.
    var stretch = false
    init(fresh: Bool, w: Int, h: Int, scale: Double, lifetime: UInt64, generation: UInt32, lists: [Data?],
         images: [String: CGImage], fonts: [Canvas2DFont: CTFont], seq: Int) {
        self.fresh = fresh; self.w = w; self.h = h; self.scale = scale; self.lifetime = lifetime
        self.generation = generation; self.lists = lists; self.images = images; self.fonts = fonts; self.seq = seq
    }

    /// Take a later arrival into this replay; always true. A fresh bitmap's
    /// lists supersede everything before them.
    func absorb(_ later: Canvas2DJob) -> Bool {
        if later.fresh {
            fresh = true; w = later.w; h = later.h; scale = later.scale
            lifetime = later.lifetime; generation = later.generation; lists = later.lists
        } else {
            lists += later.lists
        }
        images = later.images
        fonts.merge(later.fonts) { _, new in new }
        seq = later.seq
        animating = later.animating
        stretch = later.stretch
        return true
    }
}

/// What one replay reads off the main thread: the images and fonts as they
/// were when its lists arrived.
final class Canvas2DSnapshot: Canvas2DEnv {
    let images: [String: CGImage]
    let fonts: [Canvas2DFont: CTFont]
    init(images: [String: CGImage], fonts: [Canvas2DFont: CTFont]) { self.images = images; self.fonts = fonts }
    func canvasFont(_ f: Canvas2DFont) -> CTFont? { fonts[f] }
    func canvasImage(_ src: String) -> CGImage? { images[src] }
}

