// Press feedback (LLP 1061 D2): while a finger holds a pressable node down
// inside its box, the node shows its `press-scale` about its centre, eased in
// over 120 ms and eased back on release or cancel. The host owns it end to
// end — touch-down to the first scaled frame never waits for the runner — and
// it composes with the motion engine by folding into the one transform every
// writer goes through (`applyTransform`): an engine write mid-press keeps the
// press, a press mid-transition keeps the engine's value. Reduced motion
// drops it. The tap itself is unchanged: `pressed` and the scroll view's
// cancel decide it, as before.
#if os(iOS)
import UIKit

/// One node's press factor, eased from `from` to `to` since `start`.
struct PressFeedback {
    var from: CGFloat = 1, to: CGFloat = 1, start: CFTimeInterval = 0
    /// In and back alike: a fast settle that reads as a physical give.
    static let duration: CFTimeInterval = 0.12

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
    func pressChanged() { aimPress(pressed) }
    /// The finger moved while pressed: the feedback follows whether it is
    /// still inside, as the tap's own acceptance does on release.
    func pressMoved(_ touches: Set<UITouch>) {
        if pressed, let touch = touches.first { aimPress(pressInside(touch)) }
    }
    /// Whether a touch is inside the box as it stands unpressed. The pressed
    /// box is smaller, so testing against it would release a finger resting
    /// between the two edges, which re-grows the box under it, which presses
    /// again: a flicker. The unpressed point is the pressed one scaled back
    /// out about the centre.
    func pressInside(_ touch: UITouch) -> Bool {
        let p = local(touch.location(in: nil)), f = pressFactor
        let c = CGPoint(x: bounds.midX, y: bounds.midY)
        return bounds.contains(CGPoint(x: c.x + (p.x - c.x) * f, y: c.y + (p.y - c.y) * f))
    }
    private func aimPress(_ down: Bool) {
        let target = down && !DisplayPreferences.reducedMotion ? number("press_scale", 1) : 1
        guard target > 0, target != press.to else { return }
        press.aim(target, at: CACurrentMediaTime())
        applyTransform()
        PressClock.shared.run(self)
    }
}

/// Frames for presses in flight: one display link for every session, alive
/// only while some press is still easing, at the panel's full rate so the
/// 120 ms reads as motion on ProMotion (LLP 1061 D3).
final class PressClock: NSObject {
    static let shared = PressClock()
    private var link: CADisplayLink?
    private let views = NSHashTable<NodeView>.weakObjects()

    func run(_ view: NodeView) {
        views.add(view)
        guard link == nil else { return }
        let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
        l.preferredFrameRateRange = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)
        l.add(to: .main, forMode: .common)
        link = l
    }
    @objc private func tick(_ link: CADisplayLink) {
        let now = CACurrentMediaTime()
        for view in views.allObjects {
            // A settled release goes back to idle before its last write, so
            // the transform ends exactly where the engine left it.
            if view.press.settled(at: now) {
                views.remove(view)
                if view.press.to == 1 { view.press = PressFeedback() }
            }
            view.applyTransform()
        }
        if views.allObjects.isEmpty { link.invalidate(); self.link = nil }
    }
}
#endif
