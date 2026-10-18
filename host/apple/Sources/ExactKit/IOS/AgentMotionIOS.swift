// A picture at rest on UIKit (LLP 1116.003 D8): before a screenshot (and a
// `check`), UIKit's own motion on the wall clock runs to its end — the
// agent's clock holds the app's Contract motion, but not a segment's thumb or a switch's
// knob, which a screenshot right after a tap caught on its old side (round
// 5's polls). A film's frames (`screenshot … over … every …`) are taken as
// they stand. A native module's motion on the wall clock goes on under the
// agent's: `clock` waits for its animations to end, and a tap for an overlay
// the app laid over the screen to go.
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// UIKit's own motion on the wall clock (a segment's selection, a
    /// switch's knob, a bar's fade) runs to its end before a picture, for at
    /// most `bound` seconds: a screenshot right after a tap showed the old
    /// segment (round 5's polls). At rest is no animation in flight, no
    /// layer's geometry or opacity changing and, over UIKit's own controls,
    /// no pixel changing, for a quiet spell: iOS 26 moves a segment's thumb
    /// in the render server, with nothing on the layers to see, starting a
    /// frame or five after the tap (the quiet spell outlasts that). A
    /// repeating animation (a caret's blink, a spinner) and one on a paused
    /// layer (the agent's clock holds the app's own motion) are never waited
    /// for. Whether it came to rest within the bound.
    /// A share sheet or document picker shown for real (`--touch platform`)
    /// still arriving: UIKit presents it on a later turn, then animates it in.
    var systemSheetArriving: Bool {
        #if os(iOS)
        ShareSheet.arriving || session.picker.documentArriving
        #else
        false
        #endif
    }

    @discardableResult func settleMotion(bound: TimeInterval = 1.5) -> Bool {
        guard let window = presenter.viewport.window else { return true }
        // A document picker's first presentation starts its view service, measured past 1.5 s on the iOS 27 simulator.
        let arrival = Date(timeIntervalSinceNow: 5)
        while systemSheetArriving && Date() < arrival { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.016)) }
        let deadline = Date(timeIntervalSinceNow: bound)
        let controls = Agent.ownControls(in: window), quiet: TimeInterval = controls == nil ? 0.05 : 0.2
        let print = { (Agent.restingPrint(under: window.layer), controls.map { Agent.pixels(of: window, in: $0) } ?? Data()) }
        var last = print(), still: Date? = nil
        while true {
            if Date() >= deadline { return false }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.016))
            let now = print()
            guard now == last, !Agent.wallMotion(under: window.layer) else { still = nil; last = now; continue }
            if let since = still { if Date().timeIntervalSince(since) >= quiet { return true } } else { still = Date() }
        }
    }

    /// The app's own motion on the wall clock — a `UIView.animate` or a
    /// Core Animation animation on a running layer of its window, a native
    /// module's included — runs to its end, until `deadline`, and then its
    /// completion: `clock` waits for it (LLP 1012 §2), so an overlay a
    /// module fades away is gone after `clock settle`, as it is for a
    /// person. Core Animation's time is the wall's, which the agent's clock
    /// cannot seek. A completion that starts more motion (a second fade) is
    /// waited for too, within the same deadline. Whether it came to rest
    /// (false: still running at the deadline).
    @discardableResult func awaitWallMotion(until deadline: Date) -> Bool {
        guard let window = presenter.viewport.window else { return true }
        while true {
            if !Agent.wallMotion(under: window.layer) {
                // An animation's completion runs on a turn after its last frame.
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.016))
                if !Agent.wallMotion(under: window.layer) { return true }
            }
            if Date() >= deadline { return false }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.016))
        }
    }

    /// Whether `hit` is a view the app's native code laid over the screen:
    /// outside every session and every UIKit presentation, as a module's
    /// boot splash added to the window is.
    static func appOverlay(_ hit: UIView) -> Bool {
        guard let win = hit.window, win.rootViewController?.presentedViewController == nil,
              !routes.contains(where: { shows($0.1, hit) || $0.1.view?.isDescendant(of: hit) == true }) else { return false }
        var at: UIView? = hit
        while let v = at, v !== win {
            // UIKit's own containers (a menu's, a transition's) are not the app's.
            if v is NodeView || NSStringFromClass(Swift.type(of: v)).hasPrefix("_UI") { return false }
            at = v.superview
        }
        return true
    }

    /// A tap at `p` (in the window) waits while a view the app laid on the
    /// window, outside every session and every UIKit presentation, is over
    /// that point and alive, as a person waits for a splash to go: it goes on
    /// the wall's time (a `UIView.animate`, a display link), which the
    /// agent's clock does not move. One that takes the touch there (hit
    /// there) is alive while anything in it animates or changes from frame
    /// to frame: a splash types on in one place and fades over all. One that
    /// does not (a container that takes no touch, a fade whose model alpha
    /// is already 0) is alive only while a layer of it shown over the point
    /// moves: a toast moving in another corner delays nothing. It stops
    /// when nothing alive is there for half a second (a view that stays is
    /// then refused by the tap's own check, named), or after `bound`
    /// seconds. Whether anything moved (it waited).
    @discardableResult func awaitAppOverlay(at p: CGPoint, in win: UIWindow, bound: TimeInterval = 3) -> Bool {
        // The app's overlays shown over the point, and the one taking the touch there.
        let over = { win.subviews.filter { sub in
            let shown = sub.layer.presentation() ?? sub.layer
            return !sub.isHidden && shown.opacity > 0.01 && shown.frame.contains(p) && Agent.appOverlay(sub)
        } }
        let taking = { () -> UIView? in
            guard let hit = win.hitTest(p, with: nil), Agent.appOverlay(hit) else { return nil }
            var top = hit
            while let up = top.superview, up !== win { top = up }
            return top
        }
        guard !over().isEmpty else { return false }
        var prints: [ObjectIdentifier: Int] = [:]
        func moving() -> Bool {
            let now = CACurrentMediaTime(), whole = taking()
            var moved = false, next: [ObjectIdentifier: Int] = [:]
            Agent.pruneUnbegun(now: now)
            var pending = over().map { ($0.layer, $0 === whole) }
            while let (layer, anywhere) = pending.popLast() {
                guard !layer.isHidden else { continue }
                let shown = layer.presentation() ?? layer
                if anywhere || shown.opacity > 0.01 && (layer.superlayer ?? layer).convert(shown.frame, to: win.layer).contains(p) {
                    let key = ObjectIdentifier(layer), print = Agent.restingPrint(of: layer)
                    next[key] = print
                    if prints[key].map({ $0 != print }) == true { moved = true }
                    for name in layer.animationKeys() ?? [] {
                        if let a = layer.animation(forKey: name), Agent.inFlight(a, on: layer, key: name, now: now) { moved = true }
                    }
                }
                pending.append(contentsOf: (layer.sublayers ?? []).map { ($0, anywhere) })
            }
            prints = next
            return moved
        }
        // One taking the touch is given the quiet spell to show it is alive
        // (a display link's changes come a few frames apart); any other, two
        // frames to show a change from one to the next.
        let first = moving()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.016))
        var moved = moving() || first
        var still = moved || taking() != nil ? Date() : Date.distantPast
        let deadline = Date(timeIntervalSinceNow: bound)
        while Date() < deadline, !over().isEmpty, Date().timeIntervalSince(still) < 0.5 {
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.016))
            if moving() { still = Date(); moved = true }
        }
        return moved
    }

    static func wallMotion(under root: CALayer) -> Bool {
        let now = CACurrentMediaTime()
        var pending: [(CALayer, Float)] = [(root, 1)], moving = false
        pruneUnbegun(now: now)
        // Every animation is looked at, not only up to the first running:
        // one with no begin time is timed from when it is first seen, so two
        // begun together are seen together (`inFlight`).
        while let (layer, speed) = pending.popLast() {
            let s = speed * layer.speed
            guard s != 0, !layer.isHidden else { continue }
            for key in layer.animationKeys() ?? [] {
                if let a = layer.animation(forKey: key), inFlight(a, on: layer, key: key, now: now) { moving = true }
            }
            pending.append(contentsOf: (layer.sublayers ?? []).map { ($0, s) })
        }
        return moving
    }

    /// Whether `a`, on `layer` under `key`, is still running on the wall
    /// clock: finite (a caret's blink and a spinner repeat for ever), not
    /// held at speed 0 (Contract motion the agent's clock seeks), and not
    /// past its end, though kept on the layer to hold its last frame
    /// (`isRemovedOnCompletion` false). Its begin time is the one Core
    /// Animation recorded when it was attached; one still at 0 (not yet
    /// committed, or kept with none) runs for its span from when it was
    /// first seen, the nearest to its attachment a reader can know. Callers
    /// look at every animation in one pass, so ones attached together are
    /// first seen together.
    static func inFlight(_ a: CAAnimation, on layer: CALayer, key: String, now: CFTimeInterval) -> Bool {
        guard a.speed != 0, a.repeatCount < 1e6, a.repeatDuration < 1e6, a.duration < 60 else { return false }
        let span = max(a.duration * Double(max(1, a.repeatCount)) * (a.autoreverses ? 2 : 1), a.repeatDuration) / Double(abs(a.speed))
        guard a.beginTime == 0 else { return layer.convertTime(now, from: nil) < a.beginTime + span }
        let seen = "\(ObjectIdentifier(layer).hashValue)/\(key)/\(ObjectIdentifier(a).hashValue)"
        let first = unbegun[seen]?.first ?? now
        unbegun[seen] = (first, now)
        return now - first < span
    }
    /// When each animation with no begin time was first and last seen (`inFlight`).
    nonisolated(unsafe) private static var unbegun: [String: (first: CFTimeInterval, last: CFTimeInterval)] = [:]
    /// Forgets, before a pass, an animation not seen for a minute (the
    /// longest `inFlight` counts): it is gone, and a reused identity is a
    /// new one. Never during a pass, so every one in it is timed alike.
    static func pruneUnbegun(now: CFTimeInterval) { unbegun = unbegun.filter { now - $0.value.last < 60 } }

    /// One layer's model geometry and opacity, hashed: what a frame-by-frame
    /// animation (a display link's) changes.
    static func restingPrint(of layer: CALayer) -> Int {
        var h = Hasher()
        let t = layer.transform
        for v in [layer.position.x, layer.position.y, layer.bounds.minX, layer.bounds.minY, layer.bounds.width, layer.bounds.height, CGFloat(layer.opacity), t.m11, t.m12, t.m21, t.m22, t.m41, t.m42] { h.combine(v) }
        return h.finalize()
    }

    /// Where UIKit's own controls that move by themselves are shown (a
    /// segmented control, a switch, a slider, a stepper, a tab bar, whose
    /// selection glides to the tab pressed), in the window, or nil.
    static func ownControls(in window: UIWindow) -> CGRect? {
        var union: CGRect? = nil, pending: [UIView] = [window]
        while let v = pending.popLast() {
            guard !v.isHidden, v.alpha > 0.01 else { continue }
            #if os(iOS)
            let moves = v is UISegmentedControl || v is UISwitch || v is UISlider || v is UIStepper || v is UITabBar
            #else
            let moves = v is UISegmentedControl
            #endif
            if moves {
                let r = v.convert(v.bounds, to: nil).insetBy(dx: -4, dy: -4).intersection(window.bounds)
                if !r.isNull, r.width > 0, r.height > 0 { union = union.map { $0.union(r) } ?? r }
                continue
            }
            pending.append(contentsOf: v.subviews)
        }
        return union
    }

    /// The window's pixels over `rect`, as shown now (the render server's motion included).
    static func pixels(of window: UIWindow, in rect: CGRect) -> Data {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1; format.opaque = true; format.preferredRange = .standard
        let image = UIGraphicsImageRenderer(size: rect.size, format: format).image { ctx in
            ctx.cgContext.translateBy(x: -rect.minX, y: -rect.minY)
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: false)
        }
        return image.cgImage?.dataProvider?.data as Data? ?? Data()
    }

    /// The model geometry and opacity of every shown layer, hashed: what a
    /// frame-by-frame animation changes.
    static func restingPrint(under root: CALayer) -> Int {
        var h = Hasher(), pending = [root]
        while let layer = pending.popLast() {
            guard !layer.isHidden else { continue }
            let t = layer.transform
            h.combine(ObjectIdentifier(layer))
            for v in [layer.position.x, layer.position.y, layer.bounds.minX, layer.bounds.minY, layer.bounds.width, layer.bounds.height, CGFloat(layer.opacity), t.m11, t.m12, t.m21, t.m22, t.m41, t.m42] { h.combine(v) }
            pending.append(contentsOf: layer.sublayers ?? [])
        }
        return h.finalize()
    }
}
#endif
