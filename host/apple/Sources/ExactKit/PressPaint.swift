// The author's pressed paint (LLP 1061 D7), on both platforms: while the
// press `pressed` tracks holds a node (the bit `-exact-press-scale` and the
// platform's dim use), it shows its `-exact-press-opacity` as a fraction of
// its own opacity, its `-exact-press-background-color` in place of its
// background, and its `-exact-press-color` as its text colour, which its
// subtree takes as CSS's `color` is taken (a descendant with a colour of its
// own keeps it). Eased in and back on the scale's curve and duration, at once
// under the agent's clock; a release before the press was ever on screen
// waits until it has eased in, as the scale's does. No runner round trip.
//
// The opacity is an additive Core Animation on `opacity`, as the platform's
// dim is (PressFeedback.swift), so it composes with every engine write. The
// colours stand in the views' styles for the keys they replace, the engine's
// values kept beside them (`pressPainted`, which an engine write passes
// through), and the change cross-fades.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// One view's part in a press's paint.
struct PressPaint {
    /// Whether this node's own paint is shown (or its release is held).
    var down = false
    /// When the paint last went down.
    var at: CFTimeInterval = 0
    /// A release that came before the press was on screen, held.
    var releaseHeld = false
    /// The opacity's progress, 0 at rest and 1 held, on the scale's curve.
    var ease = PressFeedback(from: 0, to: 0, start: 0)
    /// How much opacity a full press takes away: the node's own at the
    /// press, less its `-exact-press-opacity` share of it.
    var depth: Float = 0
    /// The views this press shows in its colours, itself included.
    var inked: [WeakNodeView] = []
    /// The keys a press (this node's or an ancestor's) replaces in this
    /// view's style, with the values the engine gave them; nil was absent.
    var rest: [String: BatchValue?] = [:]
    var overlay: NodeStyle = [:]
    /// The pressed node whose colour this view shows, while it shows one.
    var inkedBy = WeakNodeView()
    /// As a node that was never pressed holds it.
    var atRest: Bool {
        !down && at == 0 && !releaseHeld && ease.rests(at: 0) && depth == 0 && inked.isEmpty && rest.isEmpty
            && overlay.isEmpty && inkedBy.view == nil
    }
}

/// A view a press recoloured, held weakly: it may be destroyed or taken
/// for another node while held.
struct WeakNodeView { weak var view: NodeView? }

extension NodeView {
    /// The rows of the pressed paint (LLP 1061 D7).
    static let pressPaintRows = ["press_opacity", "press_background_color", "press_color"]
    /// Whether the author wrote this node's pressed look: a press scale or
    /// any of the paint (any value, 1 included). It replaces the platform's
    /// unsaid press (LLP 1115.000 D6).
    var authorsPress: Bool { style["press_scale"] != nil || Self.pressPaintRows.contains { style[$0] != nil } }

    /// AppKit's layer is optional, UIKit's not.
    private var paintLayer: CALayer? { layer }
    /// The node's own opacity, as the engine presents it.
    private var ownOpacity: Float {
        #if os(macOS)
        Float(alphaValue)
        #else
        Float(alpha)
        #endif
    }

    /// Show the pressed paint (`down`) or ease it back; `instantly` drops it.
    func paintPress(_ down: Bool, instantly: Bool = false) {
        if down, pressPaint.releaseHeld { pressPaint.releaseHeld = false; return }
        guard down != pressPaint.down else { return }
        guard !down || Self.pressPaintRows.contains(where: { style[$0] != nil }) else { return }
        let now = CACurrentMediaTime()
        let still = instantly || ExactEnv.agentFreezes
        if !down, !still, now - pressPaint.at < PressFeedback.unseen {
            pressPaint.releaseHeld = true
            let at = pressPaint.at
            DispatchQueue.main.asyncAfter(deadline: .now() + PressFeedback.duration) { [weak self] in
                guard let self, pressPaint.releaseHeld, pressPaint.at == at else { return }
                pressPaint.releaseHeld = false
                paintPress(false)
            }
            return
        }
        pressPaint.releaseHeld = false
        pressPaint.down = down
        if down { pressPaint.at = now }
        fadePressOpacity(down, at: now, still: still)
        inkPress(down, still: still)
    }

    private func fadePressOpacity(_ down: Bool, at now: CFTimeInterval, still: Bool) {
        let key = "pressOpacity"
        guard let layer = paintLayer else { return }
        if down {
            let share = Float(min(1, max(0, style["press_opacity"]?.number ?? 1)))
            pressPaint.depth = ownOpacity * (1 - share)
        }
        let depth = pressPaint.depth
        guard depth != 0 else { layer.removeAnimation(forKey: key); return }
        let shown = pressPaint.ease.factor(at: now)
        pressPaint.ease.aim(down ? 1 : 0, at: now)
        let to = -depth * Float(down ? 1 : 0)
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.isAdditive = true
        if still {
            pressPaint.ease = PressFeedback(from: down ? 1 : 0, to: down ? 1 : 0, start: now)
            guard down else { layer.removeAnimation(forKey: key); return }
            // Held: what the dim's hold is (PressFeedback.swift `dimPress`).
            fade.fromValue = to
            fade.duration = .greatestFiniteMagnitude
        } else {
            fade.fromValue = -depth * Float(shown)
            fade.duration = PressFeedback.duration
            fade.timingFunction = PressFeedback.timing
        }
        fade.toValue = to
        fade.fillMode = .both
        fade.isRemovedOnCompletion = !down
        layer.add(fade, forKey: key)
    }

    /// The colours on (`down`) or off: this view's background, and the text
    /// colour of every view below that takes this one's.
    private func inkPress(_ down: Bool, still: Bool) {
        let changed: Bool
        if down {
            // Itself first, so a colour the engine gives it mid-press finds it.
            pressPaint.inked = [WeakNodeView(view: self)]
            let own = Self.pressOverlay(style)
            guard !own.isEmpty else { return }
            standInForPress(own)
            if let ink = own["text_color"] { inkBelow(self, ink: ink, inherited: pressPaint.rest["text_color"] ?? nil) }
            changed = true
        } else {
            changed = !pressPaint.inked.isEmpty
            for held in pressPaint.inked { held.view?.endPressStandIn() }
            pressPaint.inked = []
        }
        guard changed, !still, let layer = paintLayer else { return }
        let fade = CATransition()
        fade.type = .fade
        fade.duration = PressFeedback.duration
        fade.timingFunction = PressFeedback.timing
        layer.add(fade, forKey: kCATransition)
    }

    /// The views under `view` whose text colour is the pressed node's: none
    /// written of their own, and either one this press already colours or
    /// the same value as the pressed node's (a control's own colour is not
    /// one it inherits). A view with a colour of its own keeps its subtree's
    /// too.
    private func inkBelow(_ view: PlatformView, ink: BatchValue, inherited: BatchValue?) {
        for child in view.subviews {
            guard let node = child as? NodeView else { inkBelow(child, ink: ink, inherited: inherited); continue }
            guard !node.wrote("text_color"), node.pressPaint.inkedBy.view === self || node.style["text_color"] == inherited else { continue }
            pressPaint.inked.append(WeakNodeView(view: node))
            node.pressPaint.inkedBy = WeakNodeView(view: self)
            node.standInForPress(["text_color": ink])
            inkBelow(node, ink: ink, inherited: inherited)
        }
    }

    /// Show `values` in place of this view's own, keeping the engine's.
    private func standInForPress(_ values: NodeStyle) {
        var s = style
        for (key, value) in values {
            if pressPaint.rest[key] == nil { pressPaint.rest[key] = .some(s[key]) }
            s[key] = value
            pressPaint.overlay[key] = value
        }
        restyleForPress(s)
    }

    /// The engine's values again.
    func endPressStandIn() {
        guard !pressPaint.overlay.isEmpty else { return }
        var s = style
        for (key, value) in pressPaint.rest {
            if let value { s[key] = value } else { s.removeValue(forKey: key) }
        }
        pressPaint.rest = [:]
        pressPaint.overlay = [:]
        pressPaint.inkedBy = WeakNodeView()
        restyleForPress(s)
    }

    /// The views below `view` coloured by `pressing`'s press, back to their own.
    private func endInk(below view: PlatformView, by pressing: NodeView) {
        for child in view.subviews {
            if let node = child as? NodeView, node.pressPaint.inkedBy.view === pressing { node.endPressStandIn() }
            endInk(below: child, by: pressing)
        }
    }

    /// The opacity's share again while held: the node's own opacity, or its
    /// pressed share (`share`, the row's value; none is no dim), changed. The
    /// fade goes on from what shows over the rest of its time; a dim no longer
    /// written goes at once.
    func refreshPressOpacity(_ share: BatchValue?? = nil) {
        guard pressPaint.down, let layer = paintLayer else { return }
        let key = "pressOpacity"
        let fraction = Float(min(1, max(0, (share ?? style["press_opacity"])?.number ?? 1)))
        let depth = ownOpacity * (1 - fraction)
        guard depth != pressPaint.depth else { return }
        let now = CACurrentMediaTime()
        let from = -pressPaint.depth * Float(pressPaint.ease.factor(at: now))
        pressPaint.depth = depth
        let left = ExactEnv.agentFreezes ? 0 : max(0, pressPaint.ease.start + PressFeedback.duration - now)
        // None any more goes at once: the platform's press may answer next.
        guard depth != 0 else { layer.removeAnimation(forKey: key); return }
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.isAdditive = true
        fade.fromValue = left > 0 ? from : -depth
        fade.toValue = -depth
        fade.duration = left > 0 ? left : .greatestFiniteMagnitude
        fade.timingFunction = PressFeedback.timing
        fade.fillMode = .both
        fade.isRemovedOnCompletion = false
        layer.add(fade, forKey: key)
    }

    /// The keys a node's own press shows, from its style's press rows.
    private static func pressOverlay(_ s: NodeStyle) -> NodeStyle {
        var own: NodeStyle = [:]
        if let fill = s["press_background_color"] { own["background_color"] = fill }
        if let ink = s["press_color"] { own["text_color"] = ink }
        return own
    }

    /// An engine style for this view while a press paints it: the engine's
    /// new values are kept for the release, and what the press shows is
    /// read again from it. The pressed node's own rows may have changed (a
    /// new colour goes to its subtree again); a descendant that now writes
    /// its own colour keeps it.
    func pressPainted(_ s: NodeStyle) -> NodeStyle {
        guard pressPaint.down || !pressPaint.overlay.isEmpty else { return s }
        var s = s
        var overlay = pressPaint.overlay
        if pressPaint.down {
            overlay = Self.pressOverlay(s)
            refreshPressOpacity(s["press_opacity"])
            if overlay["text_color"] != pressPaint.overlay["text_color"] {
                // The views it coloured keep taking its colour; new ones
                // holding its own take it too; the rest go back.
                let before = pressPaint.inked.compactMap(\.view).filter { $0 !== self }
                pressPaint.inked = [WeakNodeView(view: self)]
                if let ink = overlay["text_color"] { inkBelow(self, ink: ink, inherited: s["text_color"]) }
                let kept = Set(pressPaint.inked.compactMap { $0.view.map(ObjectIdentifier.init) })
                for view in before where !kept.contains(ObjectIdentifier(view)) { view.endPressStandIn() }
            }
        } else if Self.writes(s, "text_color"), overlay["text_color"] != nil {
            // Its own colour now, and what is below it takes that one.
            overlay["text_color"] = nil
            if let pressing = pressPaint.inkedBy.view { endInk(below: self, by: pressing) }
        }
        for key in pressPaint.overlay.keys where overlay[key] == nil { pressPaint.rest[key] = nil }
        for (key, value) in overlay {
            pressPaint.rest[key] = .some(s[key])
            s[key] = value
        }
        pressPaint.overlay = overlay
        return s
    }

    /// Whether style `s` says its author wrote row `key`, as `wrote` reads a view's own.
    private static func writes(_ s: NodeStyle, _ key: String) -> Bool {
        guard case .array(let rows)? = s["written"] else { return false }
        return rows.contains(.string(key))
    }

    private func restyleForPress(_ s: NodeStyle) {
        applyStyle(s)
        guard isParagraph else { return }
        invalidateText()
        #if os(macOS)
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }

    /// A view taken for another node keeps nothing of a press, and the
    /// views its press recoloured go back to their own.
    func dropPressPaint() {
        for held in pressPaint.inked where held.view !== self { held.view?.endPressStandIn() }
        paintLayer?.removeAnimation(forKey: "pressOpacity")
        pressPaint = PressPaint()
    }
}
