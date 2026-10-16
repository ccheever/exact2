// A picture at rest on UIKit (LLP 1116.003 D8): before a screenshot (and a
// `check`), UIKit's own motion on the wall clock runs to its end — the
// agent's clock holds the app's own, but not a segment's thumb or a switch's
// knob, which a screenshot right after a tap caught on its old side (round
// 5's polls). A film's frames (`screenshot … over … every …`) are taken as
// they stand.
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
    @discardableResult func settleMotion(bound: TimeInterval = 1.5) -> Bool {
        guard let window = presenter.viewport.window else { return true }
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

    static func wallMotion(under root: CALayer) -> Bool {
        let now = CACurrentMediaTime()
        var pending: [(CALayer, Float)] = [(root, 1)]
        while let (layer, speed) = pending.popLast() {
            let s = speed * layer.speed
            guard s != 0, !layer.isHidden else { continue }
            for key in layer.animationKeys() ?? [] {
                guard let a = layer.animation(forKey: key), a.repeatCount < 1e6, a.repeatDuration < 1e6, a.duration < 60 else { continue }
                let span = max(a.duration * Double(max(1, a.repeatCount)) * (a.autoreverses ? 2 : 1), a.repeatDuration)
                if a.beginTime == 0 || layer.convertTime(now, from: nil) < a.beginTime + span { return true }
            }
            pending.append(contentsOf: (layer.sublayers ?? []).map { ($0, s) })
        }
        return false
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
