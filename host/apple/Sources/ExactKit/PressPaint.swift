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
//
// A node's `:active` (LLP 1121 D8) lowers to these rows, its opacity to
// `press_opacity_to`: an absolute target the press eases to from what shows
// (a hover look's included), not a share. Its `:hover` and `:focus-visible`
// looks stand in under the press while the pointer hovers it (the Mac) or its
// focus is visible, at once, their opacity an additive layer opacity
// (`looked`).
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
    /// The engine's style before the hover and focus looks (`looked`).
    var lookBase: NodeStyle?
    /// Whether the pointer hovers it, for a `:hover` look (the Mac).
    var hovered = false
    /// Whether its focus is visible, for a `:focus-visible` look.
    var focusShown = false
    /// The opacity the looks that hold show, if one names it.
    var lookOpacity: Float?
    /// As a node that was never pressed (nor looked) holds it.
    var atRest: Bool {
        !down && at == 0 && !releaseHeld && ease.rests(at: 0) && depth == 0 && inked.isEmpty && rest.isEmpty
            && overlay.isEmpty && inkedBy.view == nil && lookBase == nil && !hovered && !focusShown && lookOpacity == nil
    }
}

/// A view a press recoloured, held weakly: it may be destroyed or taken
/// for another node while held.
struct WeakNodeView { weak var view: NodeView? }

extension NodeView {
    /// Whether the text colour it shows is its own: written (a moving one
    /// included, `style.rs`), or its own press's (LLP 1061 D7), not one an
    /// ancestor's press lends it. A platform field takes no other (LLP 1115 D4).
    var showsOwnTextColor: Bool {
        wrote("text_color") || (pressPaint.down && style["press_color"] != nil)
    }
    /// The rows of the pressed paint (LLP 1061 D7).
    static let pressPaintRows = ["press_opacity", "press_opacity_to", "press_background_color", "press_color"]
    /// Whether the author wrote this node's pressed look: a press scale or
    /// any of the paint (any value, 1 included). It replaces the platform's
    /// unsaid press (LLP 1115.000 D6).
    var authorsPress: Bool { style["press_scale"] != nil || Self.pressPaintRows.contains { style[$0] != nil } }

    /// State look `state` (`hover`, `focus_visible`) style `s` carries (LLP 1121 D8).
    static func look(_ state: String, in s: NodeStyle) -> NodeStyle? {
        guard case .object(let looks)? = s["looks"], case .object(let look)? = looks[state] else { return nil }
        return look
    }

    /// How much opacity a full press takes away, by style `s`, from what
    /// shows (a look's, else the node's own): to `:active`'s absolute target,
    /// or the pressed paint's share of it, as the web and Linux take it.
    private func pressDepth(_ s: NodeStyle) -> Float {
        let shown = pressPaint.lookOpacity ?? ownOpacity
        if let to = s["press_opacity_to"]?.number {
            return shown - Float(min(1, max(0, to)))
        }
        return shown * (1 - Float(min(1, max(0, s["press_opacity"]?.number ?? 1))))
    }

    /// The looks again, as they hold now: after a change the flags don't
    /// show (the node's `disabled`).
    func relook() {
        if disabled, HeldState.view === self { HeldState.letGo("disabled") } // a disabled node holds no state
        guard let base = pressPaint.lookBase else { return }
        restyleForPress(pressPainted(looked(base)))
        refreshPressOpacity()
    }

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
        if down { pressPaint.depth = pressDepth(style) }
        // The clock moves whatever the depth, so a depth that comes later
        // takes the press's real progress.
        if still { pressPaint.ease = PressFeedback(from: down ? 1 : 0, to: down ? 1 : 0, start: now) } else { pressPaint.ease.aim(down ? 1 : 0, at: now) }
        installPressFade(pressPaint.depth, still: still)
    }

    /// The press's opacity fade for `depth` as the press's ease describes it
    /// now: the same curve and start, so what shows is `-depth × progress` at
    /// every frame; held at its end once settled (always, on the agent's clock).
    private func installPressFade(_ depth: Float, still: Bool) {
        guard let layer = paintLayer else { return }
        let key = "pressOpacity", e = pressPaint.ease
        let settled = still || e.settled(at: CACurrentMediaTime())
        guard depth != 0, !(settled && e.to == 0) else { layer.removeAnimation(forKey: key); return }
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.isAdditive = true
        fade.fillMode = .both
        if settled {
            // Held: what the dim's hold is (PressFeedback.swift `dimPress`).
            fade.fromValue = -depth * Float(e.to)
            fade.toValue = -depth * Float(e.to)
            fade.duration = .greatestFiniteMagnitude
            fade.isRemovedOnCompletion = false
        } else {
            fade.fromValue = -depth * Float(e.from)
            fade.toValue = -depth * Float(e.to)
            fade.beginTime = layer.convertTime(e.start, from: nil)
            fade.duration = PressFeedback.duration
            fade.timingFunction = PressFeedback.timing
            fade.isRemovedOnCompletion = e.to == 0
        }
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

    /// The press's opacity again when what it starts from or goes to moves
    /// (the node's own opacity, a look's, style `s`'s press rows): the fade on
    /// the same ease with the new depth; a press opacity no longer written goes
    /// at once.
    func refreshPressOpacity(_ s: NodeStyle? = nil) {
        applyLookOpacity()
        // Held, easing in or easing out: the fade is always `-depth ×
        // progress` on the press's own ease, so what shows moves with it, as
        // on the web and Linux (`shown + (to − shown) × progress`).
        guard pressPaint.down || pressPaint.ease.factor(at: CACurrentMediaTime()) > 0 else { return }
        let depth = pressDepth(s ?? style)
        guard depth != pressPaint.depth else { return }
        pressPaint.depth = depth
        installPressFade(depth, still: ExactEnv.agentFreezes)
    }

    /// The keys a node's own press shows, from its style's press rows.
    private static func pressOverlay(_ s: NodeStyle) -> NodeStyle {
        var own: NodeStyle = [:]
        if let fill = s["press_background_color"] { own["background_color"] = fill }
        if let ink = s["press_color"] { own["text_color"] = ink }
        return own
    }

    /// The style keys a state look stands in (its opacity is a layer's, `applyLookOpacity`).
    static let lookRows = ["background_color", "border_color_top", "border_color_right", "border_color_bottom", "border_color_left", "box_shadow"]

    /// An engine style as this view shows it: its `:hover` then its
    /// `:focus-visible` look over it while they hold (none on a disabled
    /// node), then the press's paint (`pressPainted`). The engine's is kept
    /// to show again when they change.
    func presented(_ s: NodeStyle) -> NodeStyle {
        pressPaint.lookBase = s["looks"] == nil ? nil : s
        let shown = pressPainted(looked(s))
        applyLookOpacity()
        return shown
    }

    private func looked(_ s: NodeStyle) -> NodeStyle {
        pressPaint.lookOpacity = nil
        guard !disabled else { return s }
        var s = s
        for (state, on) in [("hover", pressPaint.hovered), ("focus_visible", pressPaint.focusShown)] where on {
            guard let look = Self.look(state, in: s) else { continue }
            for key in Self.lookRows {
                // A row the look clears is null: the base's goes (no shadow).
                if case .null? = look[key] { s.removeValue(forKey: key) } else if let value = look[key] { s[key] = value }
            }
            if let o = look["opacity"]?.number { pressPaint.lookOpacity = Float(min(1, max(0, o))) }
        }
        return s
    }

    /// The looks' opacity in place of the node's own, at once: an additive
    /// layer opacity, as the press's is, so it composes with every engine write.
    private func applyLookOpacity() {
        guard let layer = paintLayer else { return }
        guard let to = pressPaint.lookOpacity else { layer.removeAnimation(forKey: "lookOpacity"); return }
        let hold = CABasicAnimation(keyPath: "opacity")
        hold.isAdditive = true
        hold.fromValue = to - ownOpacity
        hold.toValue = to - ownOpacity
        hold.duration = .greatestFiniteMagnitude
        hold.isRemovedOnCompletion = false
        hold.fillMode = .both
        layer.add(hold, forKey: "lookOpacity")
    }

    /// The pointer came over it or left (`hovered`), or its focus came to
    /// show or went (`focus`): its look shows at once, under the press's.
    func lookState(hovered: Bool? = nil, focus: Bool? = nil) {
        let was = (pressPaint.hovered, pressPaint.focusShown)
        if let hovered { pressPaint.hovered = hovered }
        if let focus { pressPaint.focusShown = focus }
        guard (pressPaint.hovered, pressPaint.focusShown) != was else { return }
        relook()
    }

    /// An engine style for this view while a press paints it: the engine's
    /// new values are kept for the release, and what the press shows is
    /// read again from it. The pressed node's own rows may have changed (a
    /// new colour goes to its subtree again); a descendant that now writes
    /// its own colour keeps it.
    func pressPainted(_ s: NodeStyle) -> NodeStyle {
        // A press easing out reads its depth from the new style too.
        if !pressPaint.down { refreshPressOpacity(s) }
        guard pressPaint.down || !pressPaint.overlay.isEmpty else { return s }
        var s = s
        var overlay = pressPaint.overlay
        if pressPaint.down {
            overlay = Self.pressOverlay(s)
            refreshPressOpacity(s)
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
        holdGone()
        for held in pressPaint.inked where held.view !== self { held.view?.endPressStandIn() }
        paintLayer?.removeAnimation(forKey: "pressOpacity")
        paintLayer?.removeAnimation(forKey: "lookOpacity")
        pressPaint = PressPaint()
    }
}
