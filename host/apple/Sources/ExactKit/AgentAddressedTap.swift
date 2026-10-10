// A tap addressed by id presses what it names (LLP 1012 §1 `tap`; LLP
// 1035.003 D2). The agent aims a plain `tap <target>` at the middle of the
// target's box, and a finger there reaches the deepest node with a press of
// its own: for a composite row — a post that opens its thread, holding a link
// card, a quoted post and Like and Repost — the middle is often one of those,
// so `tap post-0` pressed the card, or liked the post (found by the Bluesky
// clone, on real people's posts). A drive that names the row means the row:
//
// - A target with a press of its own is pressed itself: at its middle when a
//   finger there reaches it, else at the point of its box nearest the middle
//   where one does; when no point does, the tap is refused, naming what its
//   middle would press.
// - A target without one keeps the touch's rule (the press bubbles to the
//   nearest ancestor with a handler), but a control inside it — a node with
//   its own press, an SVG element, an inline run or link, a native control or
//   a canvas — is never pressed in its name: the tap is refused, naming both.
//
// Only the plain form aims: `tap <target> at <x> <y>`, a point, and every
// other form keep the point's semantics, as do VoiceOver's activation and a
// native button's own action. The platform files say what a press at a point
// reaches (`pressReach`); the rule and the search are here, once.
import Foundation
import CoreGraphics

/// What a press at one point reaches, as the platform's touch or click
/// would resolve it.
enum PressReach {
    /// The node whose own press it is.
    case node(NodeView)
    /// A target with no view of its own, drawn by this node: an SVG element,
    /// an inline run, a Markdown link. Its id when it has one.
    case part(NodeView, String, UInt32?)
    /// A native control inside this node that takes the touch itself (a
    /// switch, a text field's editor).
    case control(NodeView)
    /// A canvas that takes the touch as pointer input.
    case canvas(NodeView)
    /// A disabled node on the way up: nothing is pressed.
    case blocked(NodeView)
    case nothing

    var node: NodeView? {
        switch self {
        case .node(let n), .part(let n, _, _), .control(let n), .canvas(let n), .blocked(let n): return n
        case .nothing: return nil
        }
    }

    /// The press is `v`'s own: `v` takes it, or `v` (disabled) stops it.
    func isOwn(_ v: NodeView) -> Bool {
        switch self {
        case .node(let n), .control(let n), .canvas(let n), .blocked(let n): return n === v
        case .part, .nothing: return false
        }
    }

    /// The id a driver names for it: the part's own when it has one.
    var pressing: Int? {
        if case .part(_, _, let id?) = self { return Int(id) }
        return node.map { Int($0.id) }
    }

    var described: String {
        switch self {
        case .node(let n): return "#\(n.id)"
        case .part(_, let what, _): return what
        case .control(let n): return "the native control of #\(n.id)"
        case .canvas(let n): return "the canvas #\(n.id)"
        case .blocked(let n): return "disabled #\(n.id)"
        case .nothing: return "nothing"
        }
    }
}

extension Agent {
    /// The points of `box` a finger might land on, nearest `mid` first: the
    /// middles of a grid of cells of about 12 points (at most 32 a side),
    /// ties in reading order.
    static func aimPoints(in box: CGRect, from mid: CGPoint) -> [CGPoint] {
        guard !box.isNull, box.width > 0, box.height > 0 else { return [] }
        let cols = min(32, max(3, Int((box.width / 12).rounded(.up)))), rows = min(32, max(3, Int((box.height / 12).rounded(.up))))
        var points: [(CGPoint, CGFloat, Int)] = []
        for r in 0..<rows {
            for c in 0..<cols {
                let p = CGPoint(x: box.minX + (CGFloat(c) + 0.5) * box.width / CGFloat(cols), y: box.minY + (CGFloat(r) + 0.5) * box.height / CGFloat(rows))
                points.append((p, hypot(p.x - mid.x, p.y - mid.y), r * cols + c))
            }
        }
        return points.sorted { $0.1 != $1.1 ? $0.1 < $1.1 : $0.2 < $1.2 }.map(\.0)
    }

    /// Where a plain tap addressed to `v` lands, or the refusal: its middle
    /// `mid`, else a point of `area` (its box as seen, clipped to the
    /// viewport), in one space. `reach` says what a press at a point
    /// reaches, nil where a finger cannot land on `v` (covered, under the
    /// keyboard, outside the viewport). Returns the point and, when it is not
    /// the middle, what the middle would have pressed.
    static func addressedAim(_ v: NodeView, middle mid: CGPoint, area: CGRect, reach: (CGPoint) -> PressReach?) -> AddressedAim {
        guard let atMiddle = reach(mid) else { return .at(mid, nil) } // the caller's own refusals say why
        if v.takesPress {
            if atMiddle.isOwn(v) { return .at(mid, nil) }
            for p in aimPoints(in: area, from: mid) where reach(p)?.isOwn(v) == true { return .at(p, atMiddle) }
            return .refused(AddressedRefusal(addressed: v, pressing: atMiddle, middle: mid, own: true))
        }
        // Without a press of its own: what its middle reaches, unless that is
        // a control inside it (a part of it, or a node within it); a disabled
        // node presses nothing.
        if atMiddle.isOwn(v) { return .at(mid, nil) }
        if case .blocked = atMiddle { return .at(mid, nil) }
        if let n = atMiddle.node, n === v || n.isInside(v) {
            return .refused(AddressedRefusal(addressed: v, pressing: atMiddle, middle: mid, own: false))
        }
        return .at(mid, nil)
    }
}

/// Where a plain tap lands: a point and, when it is not the middle, what the
/// middle would have pressed; or the refusal.
enum AddressedAim {
    case at(CGPoint, PressReach?)
    case refused(AddressedRefusal)
}

/// A plain tap that would press something other than what it names.
struct AddressedRefusal {
    let addressed: NodeView
    let pressing: PressReach
    let middle: CGPoint
    /// The target has a press of its own, and no point of it reaches it.
    let own: Bool

    var reply: [String: Any] {
        let a = "#\(addressed.id)", d = pressing.described
        let how = "tap \(pressing.pressing.map { "#\($0)" } ?? d), or tap \(a) at <x> <y> (a point in its box) for whatever a finger there reaches"
        let error = own
            ? "tap \(a) would press \(d) inside it, at its middle; no point of \(a) a finger can reach presses \(a) itself; \(how)"
            : "tap \(a) would press \(d) inside it; \(a) has no press of its own, and a tap that names it never presses a control inside it; \(how)"
        var reply: [String: Any] = ["error": error, "addressed": Int(addressed.id), "middle": [Agent.r2(middle.x), Agent.r2(middle.y)]]
        if let id = pressing.pressing { reply["pressing"] = id }
        return reply
    }
}

extension NodeView {
    /// Strictly inside `ancestor` in the view tree.
    func isInside(_ ancestor: NodeView) -> Bool { self !== ancestor && isDescendant(of: ancestor) }
}

#if os(macOS)
import AppKit

extension NodeView {
    /// Whether a click is this node's own (`mouseDown` keeps it): `pressable`
    /// — a `press` handler, a link, a button invoking a command or a popover —
    /// or a surface control.
    var takesPress: Bool { pressable || isSurfaceControl }
}

extension Agent {
    /// What a click at `p` (in the window) reaches, as `mouseDown` and
    /// `mouseUp` resolve it, nil where it would not land on `v`: an SVG
    /// element, an inline run or link, a canvas, a native control, else the
    /// nearest node from the hit one up that takes a press — or a disabled
    /// one on the way, which stops it.
    func pressReach(_ v: NodeView, at p: CGPoint, in win: NSWindow) -> PressReach? {
        guard let hit = win.contentView?.hitTest(p), hit === v || hit.isDescendant(of: v) else { return nil }
        var at: NSView? = hit, node: NodeView?
        while let cur = at {
            if let n = cur as? NodeView { node = n; break }
            // A native button's control stands for its node (LLP 1069.011 D4);
            // any other enabled control takes the click itself.
            if let button = cur as? NativeButtonMac, let owner = button.owner { node = owner; break }
            if let control = cur as? NSControl, control.isEnabled {
                var up = control.superview
                while let view = up, !(view is NodeView) { up = view.superview }
                if let n = up as? NodeView { return .control(n) }
            }
            at = cur.superview
        }
        guard let node, !node.inert else { return .nothing }
        let local = node.local(p)
        if let element = presenter.svg.target(node.id, at: local) { return .part(node, "SVG element #\(element)", element) }
        if node.isParagraph, let run = node.inlineActivationTarget(at: local) { return .part(node, "inline run #\(run.id)", run.id) }
        if node.isParagraph, let href = node.inlineLink(at: local) { return .part(node, "the link \(href)", nil) }
        if session.canvases.wantsInput(node.id), !node.isSurfaceControl { return .canvas(node) }
        var up: NSView? = node
        while let cur = up {
            if let n = cur as? NodeView {
                if n.disabled { return .blocked(n) }
                if n.takesPress { return n.bounds.contains(n.local(p)) ? .node(n) : .nothing }
            }
            up = cur.superview
        }
        return .nothing
    }
}
#endif
