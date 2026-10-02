// Press feedback (LLP 1061 D2): while a finger (a mouse button on macOS)
// holds a pressable node down inside its box, the node shows its
// `press-scale`, eased in over 120 ms and eased back on release or cancel.
// The host owns it end to end — touch-down to the first scaled frame never
// waits for the runner — and it composes with the motion engine by folding
// into the one transform every writer goes through (`applyTransform`): an
// engine write mid-press keeps the press, a press mid-transition keeps the
// engine's value. Reduced motion keeps this feedback. The tap is unchanged:
// `pressed` and the scroll view's cancel decide it, as before. Every
// transform turns about the node's `transform-origin` (LLP 1061 D6).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// One node's press factor, eased from `from` to `to` since `start`.
struct PressFeedback {
    var from: CGFloat = 1, to: CGFloat = 1, start: CFTimeInterval = 0
    /// A release that came before the press was ever on screen, held until
    /// the press has eased all the way in (D2): UIKit's delayed touches hand
    /// a quick tap in a scroll view its down and its up in one turn.
    var releaseHeld = false
    /// In and back alike: a fast settle that reads as a physical give.
    static let duration: CFTimeInterval = 0.12
    /// Shorter than one 60 Hz frame: a press this young was never presented.
    static let unseen: CFTimeInterval = 1.0 / 60

    var idle: Bool { from == 1 && to == 1 }
    func factor(at t: CFTimeInterval) -> CGFloat {
        let p = min(1, max(0, (t - start) / Self.duration))
        return from + (to - from) * CGFloat(Self.ease(p))
    }
    func settled(at t: CFTimeInterval) -> Bool { t - start >= Self.duration }
    /// Re-aim from wherever the factor is now: a release mid-ease never jumps.
    mutating func aim(_ target: CGFloat, at t: CFTimeInterval) {
        guard target != to else { return }
        from = factor(at: t); to = target; start = t
    }
    /// CSS `cubic-bezier(.16, 1, .3, 1)` at `x`: Newton's method on the
    /// curve's x, bisection when the slope is too flat to trust.
    static func ease(_ x: Double) -> Double {
        if x <= 0 { return 0 }
        if x >= 1 { return 1 }
        let (x1, y1, x2, y2) = (0.16, 1.0, 0.3, 1.0)
        let cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx
        let cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by
        func curveX(_ t: Double) -> Double { ((ax * t + bx) * t + cx) * t }
        func curveY(_ t: Double) -> Double { ((ay * t + by) * t + cy) * t }
        var t = x
        for _ in 0..<8 {
            let error = curveX(t) - x
            if abs(error) < 1e-7 { return curveY(t) }
            let slope = (3 * ax * t + 2 * bx) * t + cx
            if abs(slope) < 1e-6 { break }
            t -= error / slope
        }
        var (lo, hi) = (0.0, 1.0)
        t = x
        while hi - lo > 1e-7 {
            if curveX(t) < x { lo = t } else { hi = t }
            t = (lo + hi) / 2
        }
        return curveY(t)
    }
}

extension NodeView {
    /// The factor `applyTransform` folds into the scale now; 1 outside a press.
    var pressFactor: CGFloat { press.idle ? 1 : press.factor(at: CACurrentMediaTime()) }

    /// `pressed` changed: ease toward the pressed scale, or back to 1.
    func pressChanged() {
        if pressed { pressHaptic() }
        aimPress(pressed, release: !pressed)
    }
    /// The pointer moved while pressed: the feedback follows whether it is
    /// still inside, as the tap's own acceptance does on release.
    func pressFollows(inside: Bool) { if pressed { aimPress(inside) } }

    private func aimPress(_ down: Bool, release: Bool = false) {
        let target = down ? number("press_scale", 1) : 1
        let now = CACurrentMediaTime()
        guard target > 0 else { return }
        // Under the agent's clock (LLP 1012) nothing moves between two
        // operations: the press lands, as UIKit's animations are skipped.
        if ExactEnv.agentFreezes {
            if target != press.to { press = target == 1 ? PressFeedback() : PressFeedback(from: target, to: target, start: now); applyTransform() }
            return
        }
        if down { press.releaseHeld = false }
        if release, press.to != 1, now - press.start < PressFeedback.unseen { press.releaseHeld = true; PressClock.shared.run(self); return }
        guard target != press.to else { return }
        press.aim(target, at: now)
        applyTransform()
        PressClock.shared.run(self)
    }

    /// `transform-origin` in the box's own coordinates (LLP 1061 D6): each
    /// axis points or `{"pct": n}` of the border box; the centre when unset.
    var transformOriginPoint: CGPoint {
        let axes = style["transform_origin"]?.array ?? []
        func axis(_ i: Int, _ size: CGFloat) -> CGFloat {
            guard axes.count == 2 else { return size / 2 }
            if let points = axes[i].number { return points }
            if case .object(let o) = axes[i], let pct = o["pct"]?.number { return size * pct / 100 }
            return size / 2
        }
        return CGPoint(x: bounds.minX + axis(0, bounds.width), y: bounds.minY + axis(1, bounds.height))
    }

    #if os(iOS)
    func applyTransform() {
        // CSS's individual transforms: translate, then rotate, then scale,
        // about `transform-origin` — offset from the centre, UIKit's anchor;
        // a press folds into the scale.
        // Outermost, a layout transition's offset; its size is the surface's
        // alone (`Surface.swift`, LLP 1063).
        let o = transformOriginPoint, d = CGPoint(x: o.x - bounds.midX, y: o.y - bounds.midY), s = scale * pressFactor
        let outer = CGAffineTransform(translationX: layoutOffset.x, y: layoutOffset.y).concatenating(contextTransform)
        if let space = spaceTransform(origin: d, scale: s) {
            // A 3D rotation or a z translation (LLP 1077 D8): the layer's own
            // transform, the view's affine one left at identity.
            if transform != .identity { transform = .identity }
            layer.transform = CATransform3DConcat(space, CATransform3DMakeAffineTransform(outer))
            return
        }
        let own = CGAffineTransform(translationX: translate.x + d.x, y: translate.y + d.y).rotated(by: rotate * .pi / 180).scaledBy(x: s, y: s).translatedBy(x: -d.x, y: -d.y)
        transform = own.concatenating(outer)
    }
    /// Whether a touch is inside the box as it stands unpressed. The pressed
    /// box is smaller, so testing against it would release a finger resting
    /// between the two edges, which re-grows the box under it, which presses
    /// again: a flicker. The unpressed point is the pressed one scaled back
    /// out about the origin.
    func pressInside(_ touch: UITouch) -> Bool {
        let p = local(touch.location(in: nil)), f = pressFactor, o = transformOriginPoint
        return bounds.contains(CGPoint(x: o.x + (p.x - o.x) * f, y: o.y + (p.y - o.y) * f))
    }
    #else
    func applyTransform() {
        // A lifted Arrange row moves by its frame: AppKit paints and culls a
        // view where its frame is, never where its layer was moved.
        let shift = presenter?.reorder?.lifts(id) == true ? translate : .zero
        if shift != arrangeShift {
            // Recorded first: moving the frame can lay the view out again,
            // which comes back here (now from `layout()`).
            let was = arrangeShift
            arrangeShift = shift
            setFrameOrigin(NSPoint(x: frame.minX - was.x + shift.x, y: frame.minY - was.y + shift.y))
        }
        // The layer turns about its own origin: move `transform-origin`
        // there, turn, move it back. A press folds into the scale.
        let o = transformOriginPoint, s = scale * pressFactor
        if let space = spaceTransform(origin: o, scale: s, shift: shift) {
            // A 3D rotation or a z translation (LLP 1077 D8).
            layer?.transform = CATransform3DConcat(space, CATransform3DMakeTranslation(layoutOffset.x, layoutOffset.y, 0))
            return
        }
        var t = CGAffineTransform(translationX: translate.x - shift.x, y: translate.y - shift.y)
        t = t.translatedBy(x: o.x, y: o.y).rotated(by: rotate * .pi / 180).scaledBy(x: s, y: s).translatedBy(x: -o.x, y: -o.y)
        // Outermost, a layout transition's offset; its size is the surface's
        // alone (`Surface.swift`, LLP 1063).
        layer?.setAffineTransform(t.concatenating(CGAffineTransform(translationX: layoutOffset.x, y: layoutOffset.y)))
    }
    /// Whether a window point is inside the box. AppKit's conversion ignores
    /// the layer's transform, so this is the box as it stands unpressed.
    func pressInside(_ windowPoint: NSPoint) -> Bool { bounds.contains(local(windowPoint)) }
    #endif
}

/// Frames for presses in flight: one display link for every session, alive
/// only while some press is still easing, at the panel's full rate so the
/// 120 ms reads as motion on ProMotion (LLP 1061 D4).
final class PressClock: NSObject {
    static let shared = PressClock()
    private var link: CADisplayLink?
    private let views = NSHashTable<NodeView>.weakObjects()

    func run(_ view: NodeView) {
        views.add(view)
        guard link == nil else { return }
        #if os(macOS)
        let l = view.displayLink(target: self, selector: #selector(tick(_:)))
        #else
        let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
        #endif
        l.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
        l.add(to: .main, forMode: .common)
        link = l
    }
    @objc private func tick(_ link: CADisplayLink) {
        let now = CACurrentMediaTime()
        for view in views.allObjects {
            // A release held for an unseen press goes once the press is in.
            if view.press.releaseHeld, view.press.settled(at: now) {
                view.press.releaseHeld = false
                view.press.aim(1, at: now)
            }
            // A settled release goes back to idle before its last write, so
            // the transform ends exactly where the engine left it.
            if view.press.settled(at: now), !view.press.releaseHeld {
                views.remove(view)
                if view.press.to == 1 { view.press = PressFeedback() }
            }
            view.applyTransform()
        }
        if views.allObjects.isEmpty { link.invalidate(); self.link = nil }
    }
}
