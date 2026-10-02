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
    #if os(iOS) || os(tvOS)
    /// `ease` as Core Animation's curve, for the render server's ease.
    static let timing = CAMediaTimingFunction(controlPoints: 0.16, 1, 0.3, 1)
    #endif
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
    /// The factor on screen now; 1 outside a press.
    var pressFactor: CGFloat { press.idle ? 1 : press.factor(at: CACurrentMediaTime()) }
    /// The factor `applyTransform` folds into the scale: on iOS the press's
    /// target, which Core Animation eases toward (`easePress`); on macOS
    /// the factor now, a frame at a time (`PressClock`). LLP 1061 D2.
    var pressModelFactor: CGFloat {
        #if os(iOS) || os(tvOS)
        press.idle ? 1 : press.to
        #else
        pressFactor
        #endif
    }

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
        if release, press.to != 1, now - press.start < PressFeedback.unseen {
            press.releaseHeld = true
            #if os(iOS) || os(tvOS)
            releaseWhenSeen()
            #else
            PressClock.shared.run(self)
            #endif
            return
        }
        guard target != press.to else { return }
        #if os(iOS) || os(tvOS)
        let shown = pressFactor
        press.aim(target, at: now)
        easePress(from: shown)
        #else
        press.aim(target, at: now)
        applyTransform()
        PressClock.shared.run(self)
        #endif
    }

    /// The used `translate`: its lengths, and its percentages of the border
    /// box resolved against the box as it stands now, as CSS resolves them,
    /// so a box that changes size stays where `-50% -50%` puts it.
    var translate: CGPoint {
        CGPoint(x: translatePx.x + translatePercent.x / 100 * bounds.width, y: translatePx.y + translatePercent.y / 100 * bounds.height)
    }

    #if os(iOS) || os(tvOS)
    /// The press reaches the screen as Core Animation's, with no frame of
    /// it on the main thread: the model takes the target at once
    /// (`applyTransform`), and an additive animation eases the difference
    /// from what shows to nothing on the render server. An engine write
    /// mid-ease changes the model under it, so neither overwrites the
    /// other; a re-aim replaces it from what shows.
    private func easePress(from shown: CGFloat) {
        applyTransform()
        let to = press.to
        guard to > 0, shown != to else { layer.removeAnimation(forKey: "press"); return }
        let ease = CABasicAnimation(keyPath: "transform")
        ease.isAdditive = true
        ease.fromValue = CATransform3DMakeAffineTransform(pressScale(shown / to))
        ease.toValue = CATransform3DIdentity
        ease.duration = PressFeedback.duration
        ease.timingFunction = PressFeedback.timing
        layer.add(ease, forKey: "press")
        idleWhenSettled()
    }
    /// A scale by `k` about `transform-origin`, in the layer's own space:
    /// before the model's translate and rotate, as the press's own factor.
    private func pressScale(_ k: CGFloat) -> CGAffineTransform {
        let o = transformOriginPoint, d = CGPoint(x: o.x - bounds.midX, y: o.y - bounds.midY)
        return CGAffineTransform(translationX: d.x, y: d.y).scaledBy(x: k, y: k).translatedBy(x: -d.x, y: -d.y)
    }
    /// A held release goes once the press has eased all the way in.
    private func releaseWhenSeen() {
        let wait = max(0, press.start + PressFeedback.duration - CACurrentMediaTime())
        DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in
            guard let self, press.releaseHeld else { return }
            press.releaseHeld = false
            let shown = pressFactor
            press.aim(1, at: CACurrentMediaTime())
            easePress(from: shown)
        }
    }
    /// A settled release is idle again, its transform exactly the engine's.
    private func idleWhenSettled() {
        let start = press.start
        DispatchQueue.main.asyncAfter(deadline: .now() + PressFeedback.duration) { [weak self] in
            guard let self, press.start == start, press.to == 1, !press.releaseHeld else { return }
            press = PressFeedback()
        }
    }
    #endif

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

    #if os(iOS) || os(tvOS)
    func applyTransform() {
        // Flying scaled whole in its clip (LLP 1013.000 D4.4): the
        // flight's scale only.
        if let s = flightLook?.scale {
            transform = CGAffineTransform(scaleX: s, y: s)
            return
        }
        // CSS's individual transforms: translate, then rotate, then scale,
        // about `transform-origin` — offset from the centre, UIKit's anchor;
        // a press folds into the scale.
        // Outermost, a layout transition's offset; its size is the surface's
        // alone (`Surface.swift`, LLP 1063).
        let o = transformOriginPoint, d = CGPoint(x: o.x - bounds.midX, y: o.y - bounds.midY), s = scale * pressModelFactor
        // A sticky box's scroll offset moves it there too (LLP 1083).
        let outer = CGAffineTransform(translationX: layoutOffset.x + stickyOffset.x, y: layoutOffset.y - keyboardLift + stickyOffset.y).concatenating(contextTransform)
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
        let p = local(touch.location(in: nil)), f = pressModelFactor, o = transformOriginPoint
        return bounds.contains(CGPoint(x: o.x + (p.x - o.x) * f, y: o.y + (p.y - o.y) * f))
    }
    #else
    func applyTransform() {
        // Flying scaled whole in its clip (LLP 1013.000 D4.4): the flight's
        // scale only, about the layer's origin, the clip's top left.
        if let s = flightLook?.scale {
            layer?.setAffineTransform(CGAffineTransform(scaleX: s, y: s))
            return
        }
        // A lifted Arrange row moves by its frame: AppKit paints and culls a
        // view where its frame is, never where its layer was moved. So does
        // a sticky box by its scroll offset (LLP 1083).
        let lift = presenter?.reorder?.lifts(id) == true ? translate : .zero
        let shift = CGPoint(x: lift.x + stickyOffset.x, y: lift.y + stickyOffset.y)
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
        if let space = spaceTransform(origin: o, scale: s, shift: lift) {
            // A 3D rotation or a z translation (LLP 1077 D8).
            layer?.transform = CATransform3DConcat(space, CATransform3DMakeTranslation(layoutOffset.x, layoutOffset.y, 0))
            return
        }
        var t = CGAffineTransform(translationX: translate.x - lift.x, y: translate.y - lift.y)
        t = t.translatedBy(x: o.x, y: o.y).rotated(by: rotate * .pi / 180).scaledBy(x: s, y: s).translatedBy(x: -o.x, y: -o.y)
        // Outermost, a layout transition's offset; its size is the surface's
        // alone (`Surface.swift`, LLP 1063).
        layer?.setAffineTransform(t.concatenating(CGAffineTransform(translationX: layoutOffset.x, y: layoutOffset.y)))
    }
    /// Whether a window point is inside the box as it stands unpressed.
    /// `local` reaches a transformed box through its plane, the press
    /// included (`descend`), so the point is scaled back out as iOS's is —
    /// once. An untransformed box has no press in it to undo.
    func pressInside(_ windowPoint: NSPoint) -> Bool {
        let p = local(windowPoint), f = plane == nil ? 1 : pressFactor, o = transformOriginPoint
        return bounds.contains(CGPoint(x: o.x + (p.x - o.x) * f, y: o.y + (p.y - o.y) * f))
    }
    #endif
}

#if os(macOS)
/// Frames for presses in flight (macOS; iOS eases on the render server,
/// `easePress`): one display link for every session, alive only while some
/// press is still easing, at the panel's full rate so the 120 ms reads as
/// motion on ProMotion (LLP 1061 D4).
final class PressClock: NSObject {
    static let shared = PressClock()
    private var link: CADisplayLink?
    private let views = NSHashTable<NodeView>.weakObjects()

    func run(_ view: NodeView) {
        views.add(view)
        guard link == nil else { return }
        let l = view.displayLink(target: self, selector: #selector(tick(_:)))
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
#endif
