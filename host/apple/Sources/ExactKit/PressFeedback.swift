// Press feedback (LLP 1061 D2): while a finger (a mouse button on macOS)
// holds a pressable node down inside its box, the node shows its
// `-exact-press-scale`, eased in over 120 ms and eased back on release or cancel.
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
    #if os(iOS) || os(tvOS)
    /// The origin's offset from the centre the render server's ease was
    /// built about; nil while none runs.
    var easedAbout: CGPoint?
    /// When the platform's dim (LLP 1115, `dimPress`) went down; nil while
    /// none is held.
    var dimmedAt: CFTimeInterval?
    #endif
    /// In and back alike: a fast settle that reads as a physical give.
    static let duration: CFTimeInterval = 0.12
    /// `ease` as Core Animation's curve, for the render server's ease.
    static let timing = CAMediaTimingFunction(controlPoints: 0.16, 1, 0.3, 1)
    /// Shorter than one 60 Hz frame: a press this young was never presented.
    static let unseen: CFTimeInterval = 1.0 / 60

    var idle: Bool { from == 1 && to == 1 }
    /// As a node that was never pressed holds it.
    var atRest: Bool { rests(at: 1) }
    /// Held at `value` since time 0, with nothing pending.
    func rests(at value: CGFloat) -> Bool {
        #if os(iOS) || os(tvOS)
        if easedAbout != nil || dimmedAt != nil { return false }
        #endif
        return from == value && to == value && start == 0 && !releaseHeld
    }
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

    /// `pressed` changed: ease toward the pressed scale and paint (LLP 1061
    /// D7, PressPaint.swift), or back; a node that declares neither shows
    /// the platform's press instead: iOS's dim (LLP 1115, `dimPress`), the
    /// Mac's pressed effect (LLP 1115.000 D6, `tintPress`,
    /// Mac/PressLookMac.swift).
    func pressChanged() {
        if !pressed { pressPaint.outside = false }
        // A press held (HeldState.swift) shows as long as it is held: a real
        // press down on the node as it was held is taken over, and its
        // release shows nothing (the click is still its own).
        if holds("active") { return }
        if pressed { pressHaptic() }
        showPress(pressed)
    }
    /// The press shown (`down`), or taken off: the scale and the paint, or
    /// the platform's own. A held press (HeldState.swift) shows it as a
    /// real press does, with no haptic.
    func showPress(_ down: Bool) {
        // What a press shows goes at its release whatever the style says now:
        // an authored look the style dropped while held included.
        if !down { paintPress(false); aimPress(false, release: true) }
        #if os(iOS)
        // A list's row is its cell: the cell's ground, not the dim (LLP 1115.000 D16).
        if platformPressDims, presenter?.rowCells.press(self, down) == true { return }
        if platformPressDims { dimPress(down); return }
        if !down { dimPress(false) }
        #elseif os(macOS)
        if platformPressTints { tintPress(down); return }
        if !down { tintPress(false) }
        #endif
        guard down else { return }
        paintPress(true)
        aimPress(true)
    }
    /// The pointer moved while pressed: the feedback follows whether it is
    /// still inside, as the tap's own acceptance does on release (a press
    /// held stays as it is).
    func pressFollows(inside: Bool) {
        guard pressed else { return }
        pressPaint.outside = !inside
        if holds("active") { return }
        if !inside { paintPress(false); aimPress(false) }
        #if os(iOS)
        if platformPressDims, presenter?.rowCells.press(self, inside) == true { return }
        if platformPressDims { dimPress(inside); return }
        #elseif os(macOS)
        if platformPressTints { tintPress(inside); return }
        #endif
        guard inside else { return }
        paintPress(true)
        aimPress(true)
    }

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
    package var translate: CGPoint {
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
        guard press.to > 0, shown != press.to else { stopPressEase(); return }
        addPressEase()
        idleWhenSettled()
    }
    /// The ease from the factor at the aim to the target, about the origin
    /// as it stands now, begun when the aim was: rebuilt part way through it
    /// keeps its progress.
    private func addPressEase() {
        // A flying view shows the flight's geometry alone.
        guard flightLook == nil else { return }
        // Composed before the model, the scale about the origin is inside a
        // box's rotation too, out of the screen's plane or not: it commutes
        // with the model's own scale there.
        let o = transformOriginPoint
        let ease = CABasicAnimation(keyPath: "transform")
        ease.isAdditive = true
        ease.fromValue = CATransform3DMakeAffineTransform(pressScale(press.from / press.to))
        ease.toValue = CATransform3DIdentity
        ease.beginTime = layer.convertTime(press.start, from: nil)
        ease.duration = PressFeedback.duration
        ease.timingFunction = PressFeedback.timing
        layer.add(ease, forKey: "press")
        press.easedAbout = CGPoint(x: o.x - bounds.midX, y: o.y - bounds.midY)
    }
    /// No ease of the press on the render server: a view taken for another
    /// node, or one a flight carries, shows its own transform alone.
    func stopPressEase() {
        #if os(iOS)
        dimPress(false, instantly: true)
        #endif
        guard press.easedAbout != nil || layer.animation(forKey: "press") != nil else { return }
        layer.removeAnimation(forKey: "press")
        press.easedAbout = nil
    }
    /// The model's origin moved under a running ease (the box resized):
    /// the ease again, about the new one.
    private func followPressEase(about d: CGPoint) {
        guard let about = press.easedAbout, about != d else { return }
        if press.settled(at: CACurrentMediaTime()) { press.easedAbout = nil; return }
        addPressEase()
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
        let start = press.start
        DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in
            guard let self, press.releaseHeld, press.start == start else { return }
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
    package func applyTransform() {
        // Flying scaled whole in its clip (LLP 1013.000 D4.4): the
        // flight's scale only.
        if flightLook != nil { stopPressEase() }
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
        followPressEase(about: d)
        let outer = hostPlacement
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
    /// Where the host moves the box beyond its layout and its own CSS
    /// transform: a layout transition's offset, a sticky box's scroll offset
    /// (LLP 1083), a keyboard toolbar's lift and a context preview's projection.
    var hostPlacement: CGAffineTransform {
        CGAffineTransform(translationX: layoutOffset.x + stickyOffset.x, y: layoutOffset.y - keyboardLift + stickyOffset.y).concatenating(contextTransform)
    }
    /// The border box in `viewport`'s visible space as an open top-layer box
    /// is told to the runner: before its own transform, which the runner
    /// applies, and after the host's placement, which the runner cannot know.
    /// UIKit transforms a view about its center.
    func topLayerFrame(in viewport: UIScrollView) -> CGRect {
        guard let parent = superview else { return .zero }
        let box = CGRect(x: -bounds.width / 2, y: -bounds.height / 2, width: bounds.width, height: bounds.height)
            .applying(hostPlacement).offsetBy(dx: center.x, dy: center.y)
        return parent.convert(box, to: viewport).offsetBy(dx: -viewport.pageOrigin.x, dy: -viewport.pageOrigin.y)
    }
    /// Whether a touch is inside the box as it stands unpressed. The pressed
    /// box is smaller, so testing against it would release a finger resting
    /// between the two edges, which re-grows the box under it, which presses
    /// again: a flicker. The unpressed point is the pressed one scaled back
    /// out about the origin.
    func pressInside(_ touch: UITouch) -> Bool { pressInside(at: touch.location(in: nil)) }
    /// The same for a point in the window (the agent's recognized finger).
    func pressInside(at windowPoint: CGPoint) -> Bool {
        let p = local(windowPoint), f = pressModelFactor, o = transformOriginPoint
        return bounds.contains(CGPoint(x: o.x + (p.x - o.x) * f, y: o.y + (p.y - o.y) * f))
    }
    #else
    package func applyTransform() {
        // A backdrop mirrors its box as the transform moves and scales it.
        defer { if !backdropOperations.isEmpty { applyBackdrop() } }
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

#if os(iOS)
/// The platform's press feedback (LLP 1115, which overrides LLP 1061's "UIKit
/// shows none" for iOS): a pressable node that is not a native control and
/// declares no feedback of its own dims while held, as a hand-built UIKit
/// screen's custom button (`UIButton` `.custom` dims its content) or a
/// SwiftUI plain-style button over a card does. Down is immediate and up
/// fades back, as `UIButton`'s highlight. It rides on `pressed`, so it keeps
/// UIKit's delayed touches: in a scroll view a touch that becomes a pan
/// never presses (`delaysContentTouches`), and a pan that takes one already
/// down cancels it, which fades back. A quick tap that arrives with its
/// down and up in one turn still flashes (`dimHold`). The dim is an additive
/// Core Animation on `opacity`, never the model's alpha, so it composes with
/// every engine write and never stays behind. Being a dissolve, not motion,
/// it is what Reduce Motion keeps (HIG, Motion). The Mac shows AppKit's own
/// pressed effect, which is no dim (LLP 1115.000 D6, Mac/PressLookMac.swift);
/// tvOS nothing: its focus is the feedback.
extension NodeView {
    /// The opacity a dimmed node shows, as a fraction of its own: UIKit's
    /// custom-button dim and SwiftUI's plain-style press sit near here.
    static let dimmedOpacity: Float = 0.65
    /// How long a dim fades back.
    static let dimFade: CFTimeInterval = 0.2
    /// How long a dim shows at least, so a quick tap still flashes.
    static let dimHold: CFTimeInterval = 0.1

    /// Whether a press shows the platform's dim: a node the host presses
    /// (a `press` handler, a link, or a custom invoker's visual contact),
    /// with no pressed look written (a
    /// `-exact-press-scale` or pressed paint, any value, 1 included, is the
    /// author's: `authorsPress`), no `pointerdown` or `pointerup` of its own
    /// (the author paints its pressed state), and not a scrim covering the
    /// window (a backdrop that closes a sheet shows nothing in UIKit). A
    /// native button is UIKit's own and never comes here.
    var platformPressDims: Bool {
        guard !authorsPress, !isNativeButton, !isSurfaceControl,
              handlers.contains("press") || defaultLink != nil || pressContact?.invokerLook == true,
              !handlers.contains("pointerdown"), !handlers.contains("pointerup") else { return false }
        if let w = window {
            let r = convert(bounds, to: w)
            if r.width >= w.bounds.width, r.height >= w.bounds.height * 0.9 { return false }
        }
        return true
    }

    /// Dim (`down`) or fade back. `instantly` drops it without the fade.
    func dimPress(_ down: Bool, instantly: Bool = false) {
        let key = "pressDim"
        let now = CACurrentMediaTime()
        if down {
            guard press.dimmedAt == nil else { return }
            press.dimmedAt = now
            let delta = -layer.opacity * (1 - Self.dimmedOpacity)
            let hold = CABasicAnimation(keyPath: "opacity")
            hold.isAdditive = true
            hold.fromValue = delta
            hold.toValue = delta
            hold.duration = .greatestFiniteMagnitude
            hold.isRemovedOnCompletion = false
            hold.fillMode = .both
            layer.add(hold, forKey: key)
            return
        }
        let at = press.dimmedAt
        press.dimmedAt = nil
        // Under the agent's clock (LLP 1012) the release lands, as UIKit's
        // animations are skipped; a view taken for another node or flying
        // drops a fade still running too.
        if instantly || ExactEnv.agentFreezes {
            if layer.animation(forKey: key) != nil { layer.removeAnimation(forKey: key) }
            return
        }
        guard let at, let hold = layer.animation(forKey: key) as? CABasicAnimation, hold.duration == .greatestFiniteMagnitude else { return }
        let back = CABasicAnimation(keyPath: "opacity")
        back.isAdditive = true
        back.fromValue = hold.fromValue
        back.toValue = 0
        back.duration = Self.dimFade
        back.timingFunction = CAMediaTimingFunction(name: .easeOut)
        back.beginTime = layer.convertTime(max(now, at + Self.dimHold), from: nil)
        back.fillMode = .backwards
        layer.add(back, forKey: key)
    }
}
#endif

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
