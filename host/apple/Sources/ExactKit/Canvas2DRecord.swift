// @ref LLP 1056 §8.4 — a canvas that animates is drawn by Core Animation's
// recording. The bitmap (§8.3) spends its time rasterising: a full-screen
// canvas of 3,000 arcs took 57 ms of Core Graphics a frame on an iPhone 13
// Pro Max at 3x, 17 redraws a second. The same Core Graphics calls made into
// a layer that `drawsAsynchronously` are recorded, not rasterised, and Core
// Animation rasterises the recording on its own threads: 60 redraws a
// second, as SwiftUI's `Canvas` manages, in Core Graphics' own semantics.
//
// A canvas keeps no bitmap while it is recorded. What it has drawn is the
// lists since it was last covered whole (an opaque `fillRect` over every
// pixel, a `reset`, or a fresh bitmap), replayed from the state they started
// in (`Canvas2DReplayState`), so each redraw records only what can still
// show. A tracker, a replayer that follows state without painting, watches
// every list on the replay queue for the covers and for what recording draws
// differently from a bitmap (shadows, conic gradients, the operators that
// reach outside the shape); an explicit bitmap stretched to its box is
// refused too. The policy is `Canvas2DHost.run`'s.
import CoreGraphics
import Foundation
import QuartzCore

/// What a tracker saw of one list.
struct Canvas2DTracking {
    /// The list's first paint covers every pixel: nothing drawn before it
    /// can show.
    var covers = false
    /// It drew something a recording draws differently from a bitmap.
    var refused = false
    /// A paint has been seen in this list.
    var painted = false
    /// Clips now in force, and at each saved level.
    var clips = 0
    var clipStack: [Int] = []
}

/// A replayer's state between lists: what a replay starts from.
struct Canvas2DReplayState {
    var state: Canvas2DState
    var stack: [Canvas2DState]
    var gradients: [UInt32: Canvas2DGradient]
    var patterns: [UInt32: Canvas2DPattern]
    var imageSources: [UInt32: String]
    var path: CGPath
    var scratch: CGPath
}

extension Canvas2DReplayer {
    /// A tracker for a canvas `width`×`height` pixels at `scale`: its context
    /// is one pixel, and it paints nothing.
    convenience init(trackingWidth width: Int, height: Int, scale: Double, lifetime: UInt64, generation: UInt32) {
        let one = CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 0, space: canvas2DSRGB, bitmapInfo: canvas2DBitmapInfo)
        let base = CGAffineTransform(a: CGFloat(scale), b: 0, c: 0, d: -CGFloat(scale), tx: 0, ty: CGFloat(height))
        self.init(context: one, base: base, width: width, height: height, lifetime: lifetime, generation: generation)
        tracking = Canvas2DTracking()
    }

    /// A replayer into a recording layer's context, whose user space is the
    /// bitmap's pixels (`Canvas2DRecordLayer.draw`); `yDown` when the
    /// context's origin is already top left.
    convenience init(recording c: CGContext, scale: Double, yDown: Bool, width: Int, height: Int) {
        let s = CGFloat(scale)
        let base = yDown ? CGAffineTransform(scaleX: s, y: s) : CGAffineTransform(a: s, b: 0, c: 0, d: -s, tx: 0, ty: CGFloat(height))
        self.init(context: c, base: base, width: width, height: height, lifetime: 0, generation: 0)
    }

    /// Follow one list; what it saw.
    func track(_ data: Data) -> Canvas2DTracking {
        let clips = tracking?.clips ?? 0, clipStack = tracking?.clipStack ?? []
        tracking = Canvas2DTracking(clips: clips, clipStack: clipStack)
        _ = apply(data)
        return tracking ?? Canvas2DTracking()
    }

    /// A tracker's step: true when it consumed `op` (every paint and clip);
    /// state operations fall through to the replayer's own.
    func track(_ op: Canvas2DOp, _ n: [Double]) -> Bool {
        guard var t = tracking else { return false }
        defer { tracking = t }
        switch op {
        case .save:
            t.clipStack.append(t.clips)
            return false
        case .restore:
            if !stack.isEmpty { t.clips = t.clipStack.popLast() ?? 0 }
            return false
        case .reset:
            if !t.painted { t.covers = true }
            t.painted = true; t.clips = 0; t.clipStack = []
            return false
        case .conicGradient:
            t.refused = true
            return false
        case .clip, .clipPath:
            t.clips += 1
            if op == .clipPath { scratch = CGMutablePath() }
            return true
        case .fill, .fillPath, .stroke, .strokePath, .fillRect, .strokeRect, .clearRect, .fillText, .strokeText, .drawImage, .putImageData:
            if !t.painted {
                t.painted = true
                if (op == .fillRect || op == .clearRect) && t.clips == 0 && covers(n, clear: op == .clearRect) { t.covers = true }
            }
            let shadows = state.shadowColor.alpha > 0 && (state.shadowBlur > 0 || state.shadowOffset != .zero)
            if shadows || canvas2DClipsExtent(state.composite) { t.refused = true }
            if op == .fillPath || op == .strokePath { scratch = CGMutablePath() }
            return true
        default:
            return false
        }
    }

    /// Whether a `fillRect` (or `clearRect`) of `n` now covers every pixel:
    /// an opaque colour drawn source-over, or a clear, under an axis-aligned
    /// matrix.
    private func covers(_ n: [Double], clear: Bool) -> Bool {
        let a = state.author
        guard a.b == 0, a.c == 0 else { return false }
        if !clear {
            guard case .color(let c) = state.fill, c.alpha == 1, state.alpha == 1, state.composite == 0 else { return false }
        }
        let r = CGRect(x: n[0], y: n[1], width: n[2], height: n[3]).standardized.applying(a)
        let (w, h) = (CGFloat(width) / scale, CGFloat(height) / scale)
        return r.minX <= 0 && r.minY <= 0 && r.maxX >= w && r.maxY >= h
    }

    /// The state the next list starts from.
    func snapshot() -> Canvas2DReplayState {
        Canvas2DReplayState(state: state, stack: stack, gradients: gradients, patterns: patterns, imageSources: imageSources,
                            path: path.copy() ?? CGMutablePath(), scratch: scratch.copy() ?? CGMutablePath())
    }

    /// Start from `s`: its state, and its saved levels on the context.
    func restore(_ s: Canvas2DReplayState) {
        state = s.state; stack = s.stack; gradients = s.gradients; patterns = s.patterns; imageSources = s.imageSources
        path = s.path.mutableCopy() ?? CGMutablePath(); scratch = s.scratch.mutableCopy() ?? CGMutablePath()
        for _ in stack { context?.saveGState() }
    }
}

/// One canvas's kept lists on the replay queue: its tracker, the lists since
/// it was last covered whole and the state they start from, and whether it
/// is recorded now.
final class Canvas2DKept {
    let tracker: Canvas2DReplayer
    var lists: [Data] = []
    var bytes = 0
    /// Where `lists` start; nil when they cannot be replayed alone.
    var start: Canvas2DReplayState?
    /// A list since `start` drew something a recording draws differently.
    var refused = false
    var recording = false

    /// The most kept, in bytes and lists: a canvas that never covers itself
    /// would otherwise record all it ever drew, every frame.
    static let maxBytes = 4 << 20, maxLists = 64

    init(_ tracker: Canvas2DReplayer) {
        self.tracker = tracker
        start = tracker.snapshot()
    }

    var bounded: Bool { bytes <= Canvas2DKept.maxBytes && lists.count <= Canvas2DKept.maxLists }

    /// Follow `data`; a cover starts the kept lists over at it.
    func keep(_ data: Data) {
        let before = tracker.snapshot()
        let seen = tracker.track(data)
        if seen.covers {
            lists = [data]; bytes = data.count; start = before; refused = seen.refused
        } else if start != nil {
            lists.append(data); bytes += data.count; refused = refused || seen.refused
        }
    }

    /// Forget the kept lists: only a cover starts them again.
    func drop() { lists = []; bytes = 0; start = nil; refused = false }
}

/// A recording of a canvas: the kept lists replayed into the layer's own
/// context, which `drawsAsynchronously` makes a list of commands Core
/// Animation rasterises off the main thread. It sits over the canvas's
/// bitmap, in the tree, and records on the main thread, when Core Animation
/// displays it. Recording off the main thread was measured and refused: a
/// layer out of the tree rasterises as it records (no faster than the
/// bitmap), and one in the tree recorded off the main thread raced Core
/// Animation's own drawing queue (a crash under a layer snapshot).
final class Canvas2DRecordLayer: CALayer {
    struct Frame {
        let lists: [Data]
        let start: Canvas2DReplayState
        let env: Canvas2DEnv
        let width: Int, height: Int, scale: Double
    }
    private var frame_: Frame?

    override init() {
        super.init()
        drawsAsynchronously = true
        anchorPoint = .zero
        position = .zero
    }

    override init(layer: Any) { super.init(layer: layer) }

    required init?(coder: NSCoder) { nil }

    override func action(forKey event: String) -> CAAction? { NSNull() }

    /// Show `f`, recorded at the next display (main thread).
    func show(_ f: Frame) {
        frame_ = f
        contentsScale = CGFloat(f.scale)
        bounds = CGRect(x: 0, y: 0, width: CGFloat(f.width) / CGFloat(f.scale), height: CGFloat(f.height) / CGFloat(f.scale))
        isHidden = false
        setNeedsDisplay()
    }

    /// Show the bitmap under it instead.
    func hide() {
        frame_ = nil
        isHidden = true
        contents = nil
    }

    override func draw(in ctx: CGContext) {
        guard let f = frame_, f.width > 0, f.height > 0 else { return }
        // To the bitmap's pixels, the space the bitmap replayer draws in.
        ctx.scaleBy(x: bounds.width / CGFloat(f.width), y: bounds.height / CGFloat(f.height))
        let r = Canvas2DReplayer(recording: ctx, scale: f.scale, yDown: ctx.ctm.d < 0, width: f.width, height: f.height)
        r.env = f.env
        r.restore(f.start)
        for l in f.lists { _ = r.apply(l) }
        r.env = nil
    }
}
