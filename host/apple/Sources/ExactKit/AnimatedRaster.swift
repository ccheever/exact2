// @ref LLP 1011.000 — animated GIF and WebP on Apple: what a decode learns
// about the animation (`RasterAnimation`), Chrome's schedule for it
// (`AnimationSchedule`, `AnimationClock`), and the frames themselves, decoded
// at the display size one after another (`AnimatedFrames`). The players that
// put them on screen are `AnimatedRasters.swift`.
import CoreGraphics
import Foundation
import ImageIO

/// What playing an animated image needs, read once by the decode that made
/// its first frame: the frame count, each frame's time on screen, how many
/// times it plays, and where its bytes are (the source's own file, kept by
/// `owner` while this lives). Immutable; the frames are decoded on demand.
final class RasterAnimation: @unchecked Sendable {
    let url: URL
    let count: Int
    let schedule: AnimationSchedule
    let plan: RasterDecodePlan
    let webp: Bool
    private let owner: (any RasterSourceOwner)?

    private init(url: URL, count: Int, schedule: AnimationSchedule, plan: RasterDecodePlan, webp: Bool, owner: (any RasterSourceOwner)?) {
        self.url = url; self.count = count; self.schedule = schedule; self.plan = plan; self.webp = webp; self.owner = owner
    }

    /// The bytes of one decoded frame at the plan's size.
    var frameBytes: Int { plan.outputBytes }

    /// A GIF or WebP with more than one frame; nil for anything else (a
    /// still, or an animated PNG, whose first frame paints as before).
    static func read(_ source: CGImageSource, url: URL, plan: RasterDecodePlan, owner: (any RasterSourceOwner)?) -> RasterAnimation? {
        guard let type = CGImageSourceGetType(source) as String?, type == "com.compuserve.gif" || type == "org.webmproject.webp" else { return nil }
        let count = CGImageSourceGetCount(source)
        guard count > 1 else { return nil }
        let webp = type == "org.webmproject.webp"
        let key = webp ? kCGImagePropertyWebPDictionary : kCGImagePropertyGIFDictionary
        let properties = CGImageSourceCopyProperties(source, nil) as? [CFString: Any]
        let container = properties?[key] as? [CFString: Any]
        // ImageIO counts plays, as Chrome does: a GIF's NETSCAPE loop count
        // n is n + 1 plays and none is one; a WebP's is its own; 0 is forever.
        let plays = (container?[webp ? kCGImagePropertyWebPLoopCount : kCGImagePropertyGIFLoopCount] as? NSNumber)?.intValue ?? 1
        let unclamped = webp ? kCGImagePropertyWebPUnclampedDelayTime : kCGImagePropertyGIFUnclampedDelayTime
        let clamped = webp ? kCGImagePropertyWebPDelayTime : kCGImagePropertyGIFDelayTime
        let info = container?[webp ? kCGImagePropertyWebPFrameInfoArray : kCGImagePropertyGIFFrameInfoArray] as? [[CFString: Any]]
        var delays: [Double] = []
        for i in 0..<count {
            let frame = info.flatMap { $0.indices.contains(i) ? $0[i] : nil }
                ?? ((CGImageSourceCopyPropertiesAtIndex(source, i, nil) as? [CFString: Any])?[key] as? [CFString: Any])
            let seconds = (frame?[unclamped] as? NSNumber ?? frame?[clamped] as? NSNumber)?.doubleValue ?? 0
            delays.append(seconds)
        }
        return RasterAnimation(url: url, count: count, schedule: AnimationSchedule(seconds: delays, plays: max(0, plays)),
                               plan: plan, webp: webp, owner: owner)
    }
}

/// Chrome's timing for an animated image (cc's `ImageAnimationController`,
/// Blink's `DeferredImageDecoder`): a frame of 10 ms or less shows for
/// 100 ms; frame i shows from the sum of the durations before it; a finite
/// animation holds its last frame after its last play. Times are whole
/// milliseconds, so the sums are exact.
struct AnimationSchedule: Equatable {
    /// Each frame's time on screen, ms, clamped.
    let durations: [Double]
    /// Each frame's end within one play.
    let ends: [Double]
    /// Plays in all; 0 is forever.
    let plays: Int

    init(seconds: [Double], plays: Int) {
        // Delays are whole milliseconds (a GIF's are centiseconds): round
        // before the clamp so 0.01 s is 10 ms, not a hair over it.
        durations = seconds.map { s in let ms = (s * 1000).rounded(); return ms <= 10 ? 100 : ms }
        var sum = 0.0
        ends = durations.map { sum += $0; return sum }
        self.plays = plays
    }

    var total: Double { ends.last ?? 0 }

    /// The frame at `elapsed` ms since the first began, and the elapsed
    /// time of the next change; nil when it holds for good.
    func at(_ elapsed: Double) -> (index: Int, next: Double?) {
        let count = durations.count, total = total
        guard count > 1, total > 0 else { return (0, nil) }
        let e = max(0, elapsed)
        if plays > 0 && e >= total * Double(plays) { return (count - 1, nil) }
        let cycle = (e / total).rounded(.down)
        let within = e - cycle * total
        var index = 0
        while index < count - 1 && within >= ends[index] { index += 1 }
        let next = cycle * total + ends[index]
        if plays > 0 && index == count - 1 && cycle + 1 >= Double(plays) { return (index, nil) }
        return (index, next)
    }

    /// When the frame showing at `elapsed` began, in elapsed time.
    func began(_ elapsed: Double) -> Double {
        let (index, _) = at(elapsed)
        let total = total
        guard total > 0 else { return 0 }
        var cycle = (max(0, elapsed) / total).rounded(.down)
        if plays > 0 { cycle = min(cycle, Double(plays - 1)) }
        return cycle * total + (index > 0 ? ends[index - 1] : 0)
    }
}

/// One view's play of an animation, on its own clock (the wall, or the
/// agent's): it starts the first time it is seen, and takes no time while it
/// is not. Off screen it does nothing; seen again it is where the time says,
/// with Chrome's two exceptions (`ImageAnimationController::AdvanceFrame`):
/// more than five minutes behind it resumes the frame it left, and a first
/// play that ended while unseen starts over from the first frame.
struct AnimationClock: Equatable {
    let schedule: AnimationSchedule
    private(set) var start: Double?
    private var hidden: Double?

    init(_ schedule: AnimationSchedule) { self.schedule = schedule }

    mutating func hide(at now: Double) {
        if start != nil && hidden == nil { hidden = now }
    }

    /// The frame to show at `now`, seen.
    mutating func show(at now: Double) -> Int {
        guard let begun = start else { start = now; return 0 }
        if let left = hidden {
            hidden = nil
            let (index, next) = schedule.at(left - begun)
            if let next, now - (begun + next) > 300_000 {
                // The frame it left starts again now.
                start = now - schedule.began(left - begun)
                return index
            }
            if schedule.plays != 1, left - begun < schedule.total,
               now >= begun + schedule.total + (schedule.durations.first ?? 0) {
                start = now
                return 0
            }
        }
        return schedule.at(now - (start ?? now)).index
    }

    /// When the frame changes next, on this clock; nil when it holds.
    func next(after now: Double) -> Double? {
        guard let start, hidden == nil, let next = schedule.at(now - start).next else { return nil }
        return start + next
    }
}

/// One animation's frames, decoded in order. Used from one queue at a time.
protocol AnimatedFrames: AnyObject {
    func frame(_ index: Int) -> CGImage?
}

extension RasterAnimation {
    /// A decoder for the frames: ImageIO for a GIF, which composes each frame
    /// over the last cheaply in order; for a WebP, its frames composed here
    /// (`WebPFrames`), since ImageIO composes a WebP's frame i from the first
    /// each time (98 ms for frame 99 of a 480-pixel animation on an M-series Mac).
    func frames() -> AnimatedFrames? {
        webp ? WebPFrames(self) : ImageIOFrames(self)
    }
}

/// A GIF's frames through ImageIO, which composes them (disposal and all)
/// and reduces each to the plan's size.
final class ImageIOFrames: AnimatedFrames {
    private let source: CGImageSource
    private let plan: RasterDecodePlan
    init?(_ animation: RasterAnimation) {
        guard let source = CGImageSourceCreateWithURL(animation.url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary) else { return nil }
        self.source = source; plan = animation.plan
    }
    func frame(_ index: Int) -> CGImage? {
        guard let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, index, RasterImage.thumbnailOptions(plan)) else { return nil }
        return RasterImage.normalized(thumbnail, plan: plan)
    }
}

/// An animated WebP composed as its container says (RFC 9649 §2.7): each
/// `ANMF` frame is its own bitstream, drawn at its offset over the canvas
/// (alpha-blended unless it says not to), and its rectangle cleared to
/// transparent after it shows when it asks to be disposed. The canvas is
/// the image's own size; each frame shown is reduced to the plan's.
final class WebPFrames: AnimatedFrames {
    private struct Frame {
        let rect: CGRect
        let dispose: Bool
        let blend: Bool
        let bitstream: Data
    }
    private let frames: [Frame]
    private let canvas: CGContext
    private let plan: RasterDecodePlan
    private var next = 0
    /// The canvas a composing decoder may hold: 4096 × 4096 pixels.
    static let canvasLimit = 64 * 1024 * 1024

    init?(_ animation: RasterAnimation) {
        guard let data = try? Data(contentsOf: animation.url, options: .alwaysMapped),
              let (width, height, frames) = Self.parse(data), frames.count == animation.count,
              width * height * 4 <= Self.canvasLimit,
              let canvas = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                                     space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                     bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        self.frames = frames; self.canvas = canvas; plan = animation.plan
    }

    func frame(_ index: Int) -> CGImage? {
        guard frames.indices.contains(index) else { return nil }
        if index < next { next = 0 }
        while next <= index {
            guard compose(next) else { return nil }
            next += 1
        }
        guard let whole = canvas.makeImage() else { return nil }
        return RasterImage.normalized(whole, plan: plan)
    }

    private func compose(_ k: Int) -> Bool {
        let full = CGRect(x: 0, y: 0, width: canvas.width, height: canvas.height)
        if k == 0 { canvas.clear(full) } else if frames[k - 1].dispose { canvas.clear(frames[k - 1].rect) }
        let f = frames[k]
        guard let source = CGImageSourceCreateWithData(f.bitstream as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return false }
        canvas.saveGState()
        canvas.setBlendMode(f.blend ? .normal : .copy)
        canvas.draw(image, in: f.rect)
        canvas.restoreGState()
        return true
    }

    /// The canvas size and each frame, from the RIFF container; nil when it
    /// is not an animated WebP this reads.
    private static func parse(_ d: Data) -> (Int, Int, [Frame])? {
        let b = [UInt8](d)
        func le24(_ i: Int) -> Int { Int(b[i]) | Int(b[i + 1]) << 8 | Int(b[i + 2]) << 16 }
        func le32(_ i: Int) -> Int { le24(i) | Int(b[i + 3]) << 24 }
        func tag(_ i: Int) -> String { String(bytes: b[i..<i + 4], encoding: .ascii) ?? "" }
        /// Chunks in `range`: tag, payload range, and the whole chunk's range.
        func chunks(_ range: Range<Int>) -> [(String, Range<Int>, Range<Int>)]? {
            var out: [(String, Range<Int>, Range<Int>)] = [], i = range.lowerBound
            while i + 8 <= range.upperBound {
                let size = le32(i + 4), end = i + 8 + size
                guard size >= 0, end <= range.upperBound else { return nil }
                out.append((tag(i), (i + 8)..<end, i..<min(range.upperBound, end + (size & 1))))
                i = end + (size & 1)
            }
            return out
        }
        guard b.count >= 30, tag(0) == "RIFF", tag(8) == "WEBP",
              let top = chunks(12..<min(b.count, 8 + le32(4))),
              let vp8x = top.first(where: { $0.0 == "VP8X" })?.1, vp8x.count >= 10 else { return nil }
        let width = le24(vp8x.lowerBound + 4) + 1, height = le24(vp8x.lowerBound + 7) + 1
        var frames: [Frame] = []
        for (name, p, _) in top where name == "ANMF" {
            guard p.count >= 16, let inner = chunks((p.lowerBound + 16)..<p.upperBound) else { return nil }
            let x = le24(p.lowerBound) * 2, y = le24(p.lowerBound + 3) * 2
            let w = le24(p.lowerBound + 6) + 1, h = le24(p.lowerBound + 9) + 1
            let flags = b[p.lowerBound + 15]
            // A frame's own file: its bitstream, with a `VP8X` header when an
            // `ALPH` chunk carries its alpha.
            var body = Data()
            let alpha = inner.contains { $0.0 == "ALPH" }
            if alpha {
                var header: [UInt8] = Array("VP8X".utf8) + [10, 0, 0, 0, 0x10, 0, 0, 0]
                for v in [w - 1, h - 1] { header += [UInt8(v & 0xff), UInt8(v >> 8 & 0xff), UInt8(v >> 16 & 0xff)] }
                body.append(contentsOf: header)
            }
            for (name, _, whole) in inner where ["ALPH", "VP8 ", "VP8L"].contains(name) { body.append(contentsOf: b[whole]) }
            var file = Data("RIFF".utf8)
            let size = body.count + 4
            file.append(contentsOf: [UInt8(size & 0xff), UInt8(size >> 8 & 0xff), UInt8(size >> 16 & 0xff), UInt8(size >> 24 & 0xff)])
            file.append(contentsOf: Array("WEBP".utf8)); file.append(body)
            // WebP's origin is the top left, Core Graphics' the bottom left.
            frames.append(Frame(rect: CGRect(x: x, y: height - y - h, width: w, height: h),
                                dispose: flags & 1 != 0, blend: flags & 2 == 0, bitstream: file))
        }
        return frames.isEmpty ? nil : (width, height, frames)
    }
}
