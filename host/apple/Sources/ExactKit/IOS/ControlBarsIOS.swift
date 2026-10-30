// Content under a fixed control (LLP 1131): a scroller whose neighbours
// between it and its route's or page's edge are control bars (a segmented
// switcher, a composer, a row of buttons) is drawn through them, inset by
// them, and they draw over its content with UIKit's scroll edge effect, as a
// hand-built iOS 26 screen's list runs under a `safeAreaBar`. Layout does not
// change: the bands are its scroll view's (`PageEdgesIOS.swift`).
#if os(iOS) || os(tvOS)
import UIKit

/// One pass's answers (`Presenter.reachPageEdges`), so sibling scrollers share
/// their parent's children and each box is judged once.
final class ControlBars {
    unowned let presenter: Presenter
    init(_ presenter: Presenter) { self.presenter = presenter }
    private var bars: [ObjectIdentifier: Bool] = [:]
    /// A parent's shown in-flow children (nil: one drawn as a layer, `FlatLeaves`, never a bar), where each
    /// is, and whether every one before and after each is a bar.
    private struct Kids { var all: [NodeView?] = [], at: [ObjectIdentifier: Int] = [:], barsBefore: [Bool] = [], barsAfter: [Bool] = [] }
    private var kids: [ObjectIdentifier: Kids] = [:]

    private func children(of p: NodeView) -> Kids {
        if let known = kids[ObjectIdentifier(p)] { return known }
        var k = Kids()
        let shown = { (v: NodeView) in !v.isHidden && NavigationHost.inFlow(v) }
        if let ids = presenter.flats.children(of: p.id) {
            k.all = ids.compactMap { id in presenter.flats.leaves[id] != nil ? .some(nil) : presenter.views[id].flatMap { shown($0) ? .some($0) : nil } }
        } else { k.all = p.container.subviews.compactMap { $0 as? NodeView }.filter(shown) }
        for (i, v) in k.all.enumerated() { if let v { k.at[ObjectIdentifier(v)] = i } }
        let bar = k.all.map { $0.map(isBar) ?? false }
        var before = true, after = true
        k.barsBefore = bar.map { b in defer { before = before && b }; return before }
        k.barsAfter = bar.reversed().map { b in defer { after = after && b }; return after }.reversed()
        kids[ObjectIdentifier(p)] = k
        return k
    }

    static func laid(_ node: NodeView) -> CGRect {
        CGRect(x: node.center.x - node.bounds.width / 2, y: node.center.y - node.bounds.height / 2,
               width: node.bounds.width, height: node.bounds.height)
    }
    static func onlyRide(_ node: NodeView) -> Bool {
        let a = node.transform, b = CGAffineTransform(translationX: 0, y: node.controlRide)
        return CATransform3DIsAffine(node.layer.transform) && zip([a.a, a.b, a.c, a.d, a.tx, a.ty], [b.a, b.b, b.c, b.d, b.tx, b.ty]).allSatisfy { abs($0 - $1) < 0.00001 }
    }

    /// D1: a shown, in-flow box with only its bar ride as a transform, with no
    /// background of the author's, holding at least one control and nothing
    /// else but wrappers and captions. A control is taken whole; an image, a
    /// picture, a native view, a scroller or a painted wrapper outside one
    /// makes it no bar.
    func isBar(_ box: NodeView) -> Bool {
        if let known = bars[ObjectIdentifier(box)] { return known }
        let placed = !box.isHidden && NavigationHost.inFlow(box) && Self.onlyRide(box)
            && ["static", "relative"].contains(box.layoutPosition) && !box.wrote("background_color")
        let answer = placed && contents(box).map { $0 > 0 } == true
        bars[ObjectIdentifier(box)] = answer
        return answer
    }

    /// What a box inside a bar holds: how many controls, or nil when it holds
    /// anything else (a bar's box is checked for its own background first,
    /// `isBar`; a control may have one). Each box is judged once a pass.
    private var held: [ObjectIdentifier: Int?] = [:]
    private func contents(_ n: NodeView) -> Int? {
        if let known = held[ObjectIdentifier(n)] { return known }
        func painted(_ style: NodeStyle) -> Bool {
            if case .array(let rows)? = style["written"] { return rows.contains(.string("background_color")) }
            return false
        }
        // A child drawn as a layer (`FlatLeaves`) is a wrapper: one the author painted is no bar's.
        func leaf(_ id: UInt32) -> Int? {
            if let l = presenter.flats.leaves[id] {
                if painted(l.style) { return nil }
                return (presenter.flats.children(of: id) ?? []).reduce(0) { sum, id in sum.flatMap { s in leaf(id).map { s + $0 } } }
            }
            return presenter.views[id].flatMap(contents) ?? 0
        }
        let answer: Int?
        if n.isHidden || n.style["display"]?.string == "none" { answer = 0 }
        else if isControl(n) { answer = 1 }
        // A wrapper is a plain box with no paint of its own: a text, an image, a picture, a canvas, a native
        // view or a scroller is not.
        else if n.wrote("background_color") || n.scrollView != nil { answer = nil }
        else if n.isParagraph { answer = 0 }
        else if n.kind != "view" { answer = nil }
        else if let ids = presenter.flats.children(of: n.id) { answer = ids.reduce(0) { sum, id in sum.flatMap { s in leaf(id).map { s + $0 } } } }
        else { answer = n.container.subviews.reduce(0) { sum, v in sum.flatMap { s in (v as? NodeView).map { contents($0).map { s + $0 } } ?? s } } }
        held[ObjectIdentifier(n)] = answer
        return answer
    }

    /// A button, a link, a field, a `select`, a switch or a checkbox, a tab,
    /// or a group UIKit draws as its segmented control (taken whole: its own
    /// tabs are hidden behind it).
    func isControl(_ n: NodeView) -> Bool {
        n.actsAsButton || n.props["href"] != nil || presenter.segments.projects(n.id)
            || ["button", "control", "input", "textarea", "radio", "switch", "range", "date", "time", "select"].contains(n.kind)
            || n.props["accessibilityRole"] == "tab"
    }

    /// The control bars between `s` and `c`'s edge on one side (D2): at each
    /// level from `s` up to `c`, the box's shown in-flow siblings on that
    /// side, every one a bar placed wholly beyond it, which spans its parent's
    /// width and fills what they leave; a box with none is at its parent's
    /// edge. No parent transforms, scrolls or hides, and none below `c` clips. Empty
    /// when nothing is between them; nil when anything else is.
    func bars(of s: NodeView, in c: NodeView, top: Bool) -> [NodeView]? {
        var x = s, found: [NodeView] = []
        while x !== c {
            var up = x.superview
            // Between nodes, a host view that clips (a clip box) or scrolls cuts the band off.
            while let v = up, !(v is NodeView) { if v is UIScrollView || v.clipsToBounds || v.layer.mask != nil { return nil }; up = v.superview }
            // The container clipping to its own box cuts nothing off: no band reaches past it (a route
            // written `overflow="hidden"` at `inset=0`, which looks the same on the web).
            guard let p = up as? NodeView, !p.isHidden, !p.clipsToBounds || p === c, p.clipPath == nil, p.layer.mask == nil, p.transform.isIdentity,
                  p.scrollView == nil || p === c else { return nil }
            let k = children(of: p)
            guard let i = k.at[ObjectIdentifier(x)] else { return nil }
            let room = available(p)
            if !(top ? k.barsBefore[i] : k.barsAfter[i]) { return nil }
            let side = (top ? k.all[..<i] : k.all[(i + 1)...]).compactMap { $0 }
            // The host is not told the flex direction: the frames say which side a bar is on.
            guard side.allSatisfy({ top ? Self.laid($0).maxY <= x.frame.minY + 0.5 : Self.laid($0).minY >= x.frame.maxY - 0.5 }) else { return nil }
            if side.isEmpty {
                // Nothing between them: open only where it reaches its parent's edge.
                if !reaches(x, room, top: top) { return nil }
            } else {
                // It spans its parent and takes the room its bars leave (the host's `flexGrows`: an item
                // of a column growing by 1 or more, with no `max-height`), as a hand-built screen's list
                // does; so does a box holding it beside its own neighbours (a chat's thread and its
                // floating button, over a composer).
                guard abs(x.frame.minX - room.minX) <= 0.5, abs(x.frame.maxX - room.maxX) <= 0.5, x.props["flexGrows"] == "true" else { return nil }
            }
            found += side
            // A box between the scroller and the container has no padding or border at that edge: a gap.
            if p !== c {
                let content = p.contentBox()
                if top ? content.minY > 0.5 : p.bounds.height - content.maxY > 0.5 { return nil }
            }
            x = p
        }
        return found
    }

    /// `p`'s content box inside the cover a route's containers give it, as
    /// well as the author's padding.
    private func available(_ p: NodeView) -> CGRect {
        var room = p.contentBox()
        if case .edges(let e)? = presenter.navigation.covers[p.id] { room = room.inset(by: UIEdgeInsets(top: e.top, left: e.left, bottom: e.bottom, right: e.right)) }
        return room
    }

    /// `x`, with its margin, at that edge of `room`.
    private func reaches(_ x: NodeView, _ room: CGRect, top: Bool) -> Bool {
        top ? x.frame.minY - x.number("margin_top") - room.minY <= 0.5 : room.maxY - x.frame.maxY - x.number("margin_bottom") <= 0.5
    }

    /// Lift the bars above their scrollers' content and give each UIKit's edge
    /// effect over its scroller (D3, D4); a bar no longer one, removed from
    /// the tree included, goes back.
    static func sync(_ wanted: [(bar: NodeView, scroller: NodeView, edge: UIRectEdge)], in presenter: Presenter) {
        guard let viewport = presenter.viewport as? Viewport else { return }
        var kept = Set<ObjectIdentifier>()
        for w in wanted {
            kept.insert(ObjectIdentifier(w.bar))
            viewport.controlBars[ObjectIdentifier(w.bar)] = Weak(w.bar)
            if w.bar.bandOver != w.scroller.paintRank { w.bar.bandOver = w.scroller.paintRank; if let p = w.bar.superview { PaintOrder.changed(p) } }
            #if os(iOS)
            if #available(iOS 26.0, *), let sv = w.scroller.scrollView {
                let i = (w.bar.edgeInteraction as? UIScrollEdgeElementContainerInteraction) ?? {
                    let made = UIScrollEdgeElementContainerInteraction()
                    w.bar.addInteraction(made); w.bar.edgeInteraction = made
                    return made
                }()
                if i.scrollView !== sv { i.scrollView = sv }
                if i.edge != w.edge { i.edge = w.edge }
            }
            #endif
        }
        for (key, held) in viewport.controlBars where !kept.contains(key) {
            viewport.controlBars[key] = nil
            guard let bar = held.value else { continue }
            bar.bandOver = nil
            if let p = bar.superview { PaintOrder.changed(p) }
            if let i = bar.edgeInteraction as? UIInteraction { bar.removeInteraction(i) }
            bar.edgeInteraction = nil
        }
    }

    /// A bar, held weakly: a view the pool keeps is cleaned when it is no bar.
    final class Weak { weak var value: NodeView?; init(_ value: NodeView) { self.value = value } }
}
#endif
