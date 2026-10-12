#if os(iOS) || os(tvOS)
import UIKit

/// @ref LLP 1057 §10.6 — the iOS carrier's one held contact (LLP 1035.003 D1),
/// on a `pan` node only. UIKit synthesizes no touch (LLP 1008 §9), so the
/// contact is `recognized`: its deltas take the presenter's pan path, as
/// `ContactLayoutPan`'s do, and `up` releases at the engine's tracker over the
/// synthesized samples, spaced `ms / steps` apart on the contact's own clock,
/// never the wall time between requests (LLP 1057.001 §3). Every other contact stays `unsupported`.
struct AgentPan {
    weak var node: NodeView?
    weak var pointer: NodeView?
    /// The contact in the viewport's space, and its synthetic time in seconds.
    var at: CGPoint
    var t: Double
    var began = false
}

extension Agent {
    /// The `pan` node a contact on `view` would reach: from it up, stopping at
    /// rule 3's boundary (a field or a text area; a press node between is
    /// passed, as a pan takes a drag that starts on a button), as
    /// `MouseLayoutPan.down` walks. Where another
    /// drag could take the contact first — a descendant's (rule 3) or a drag
    /// binding on the pan node (rule 4) — only UIKit's arbitration can say,
    /// so there is no recognized pan to deliver (nil).
    static func panNode(from view: UIView) -> NodeView? {
        var at: UIView? = view
        while let current = at {
            if let node = current as? NodeView {
                if node.handlers.contains("pan") {
                    let bound = node.reorderPan != nil || node.transformRecognizer != nil || node.heightRecognizer != nil
                    return SwipeInput.allows(node) && !bound ? node : nil
                }
                if node.field != nil || node.textArea != nil { return nil }
                if !node.dragRecognizers.isEmpty || node.handlers.contains("swiperight") { return nil }
            }
            at = current.superview
        }
        return nil
    }

    /// The pointer observer is independent of the pan recognizer, as for a
    /// real touch. Its nearest enabled owner may be a child of the pan node.
    private static func pointerNode(from view: UIView) -> NodeView? {
        var current: UIView? = view
        while let v = current {
            if let n = v as? NodeView, !n.disabled,
               !n.handlers.isDisjoint(with: ["pointerdown", "pointermove", "pointerup"]) { return n }
            current = v.superview
        }
        return nil
    }

    private func panPointer(_ contact: AgentPan, _ kind: PointerKind) {
        guard let node = contact.pointer, presenter.views[node.id] === node, !node.disabled else { return }
        let name = kind == .down ? "pointerdown" : kind == .up ? "pointerup" : "pointermove"
        guard node.handlers.contains(name) else { return }
        let local = node.convert(contact.at, from: presenter.viewport), box = node.contentBox()
        let client = presenter.client(presenter.viewport.convert(contact.at, to: nil))
        presenter.pointer(node.id, kind, PointerSample(x: Double(local.x - box.minX), y: Double(local.y - box.minY),
            buttons: kind == .up ? 0 : 1, pressure: kind == .up ? 0 : 0.5, type: "touch", id: 2,
            clientX: Double(client.x), clientY: Double(client.y)))
    }

    /// `tap <scroller> drag <dx> <dy>` without real touches (LLP 1116.003; r5
    /// weather, mood journal): what a finger's drag on a scroller ends with,
    /// its content moved with the finger — up 300 points for a drag of
    /// -300 — as far as its range allows, lifted with no fling. Only on a
    /// node that scrolls itself (a scroll container, a grouped list), where
    /// the scroll view's pan is what takes the drag; a finger there must
    /// land on it. A drag on anything else reaches whatever UIKit's
    /// arbitration gives it, which only a real touch tests: refused, naming
    /// `--touch platform`.
    func scrollDrag(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard let delta = req["scrollDrag"] as? [Double], delta.count == 2, delta.allSatisfy(\.isFinite) else { return ["error": "drag: dx and dy must be finite"] }
        if v.scrollDormant { v.needScroll() }
        let list = presenter.groupedLists?.scroller(for: v.id).flatMap { $0.superview === v ? $0 : nil }
        guard let sv = list ?? v.scroll, sv.isScrollEnabled else {
            return ["error": "drag #\(v.id): without --touch platform a drag scrolls a scroller it starts on (`tap <scroll or list> drag 0 -300`); any other drag is a real gesture, --touch platform's (LLP 1080.000 §11)"]
        }
        // Where the finger goes down: the request's point in the viewport, else the middle.
        let b = box(v), vp = presenter.viewport
        let start = (req["from"] as? [Double]).map { CGPoint(x: $0[0], y: $0[1]) } ?? CGPoint(x: b.midX, y: b.midY)
        let p = vp.convert(CGPoint(x: start.x + vp.contentOffset.x, y: start.y + vp.contentOffset.y), to: nil)
        if let why = offscreen(v, box: CGRect(origin: start, size: .zero), hit: win.hitTest(p, with: nil)) { return ["error": "drag #\(v.id): \(why)"] }
        if let hit = win.hitTest(p, with: nil), let why = obscured(v, at: p, hit: hit) { return ["error": "drag #\(v.id) at (\(Agent.r2(start.x)), \(Agent.r2(start.y))): \(why)"] }
        let scrollsX = (sv as? ScrollView)?.scrollsX ?? false, scrollsY = (sv as? ScrollView)?.scrollsY ?? true
        let i = sv.adjustedContentInset, o = sv.contentOffset
        let maxX = max(-i.left, sv.contentSize.width + i.right - sv.bounds.width), maxY = max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height)
        let to = CGPoint(x: scrollsX ? min(max(o.x - CGFloat(delta[0]), -i.left), maxX) : o.x,
                         y: scrollsY ? min(max(o.y - CGFloat(delta[1]), -i.top), maxY) : o.y)
        // A drag ends a smooth correction, as a wheel does (LLP 1070.000 §11).
        if let owner = sv.superview as? NodeView, presenter.collections.animating.contains(owner.id) { presenter.collections.animationEnded(owner.id, dragging: true) }
        sv.setContentOffset(to, animated: false)
        presenter.settlePump()
        return ["tapped": Int(v.id), "at": [Agent.r2(start.x), Agent.r2(start.y)], "scrolledBy": [Agent.r2(to.x - o.x), Agent.r2(to.y - o.y)],
                "delivery": "recognized", "native": "scroll"]
    }

    /// The phase's reply, or nil where no pan contact applies (`unsupported`).
    func recognizedPan(_ phase: String, _ req: [String: Any]) -> [String: Any]? {
        let point = { (p: CGPoint) in [Agent.r2(p.x), Agent.r2(p.y)] }
        if phase == "down" {
            guard panContact == nil else { return ["error": "a contact is already down; use `tap up` or `tap cancel` first"] }
            guard let v = view(req), v.window != nil, let node = Agent.panNode(from: v) else { return nil }
            let b = box(v)
            let at = CGPoint(x: (req["x"] as? Double) ?? b.midX, y: (req["y"] as? Double) ?? b.midY)
            guard at.x.isFinite, at.y.isFinite else { return ["error": "tap down: the point must be finite"] }
            let t = CACurrentMediaTime()
            presenter.onPanSample?(true, Double(at.x), Double(at.y), t)
            let contact = AgentPan(node: node, pointer: Agent.pointerNode(from: v), at: at, t: t)
            panContact = contact
            panPointer(contact, .down)
            return ["contact": Int(node.id), "phase": "down", "at": point(at), "delivery": "recognized"]
        }
        guard var contact = panContact else { return nil }
        guard let node = contact.node, presenter.views[node.id] === node, node.handlers.contains("pan"), SwipeInput.allows(node) else {
            // The node went or stopped panning: the contact is over, at rest.
            panContact = nil
            panPointer(contact, .up)
            if contact.began, let node = contact.node, presenter.views[node.id] === node { presenter.panRelease(node.id, 0, 0) }
            return ["phase": phase, "contact": false, "cancelled": true, "at": point(contact.at), "delivery": "recognized"]
        }
        let ms = max(0, (req["ms"] as? Double) ?? 0)
        switch phase {
        case "move":
            let to = CGPoint(x: (req["x"] as? Double) ?? Double(contact.at.x) + ((req["dx"] as? Double) ?? 0),
                             y: (req["y"] as? Double) ?? Double(contact.at.y) + ((req["dy"] as? Double) ?? 0))
            guard to.x.isFinite, to.y.isFinite, ms.isFinite else { return ["error": "tap move: the point and duration must be finite"] }
            // Steps as the web carrier's: one per 16 ms, at least one.
            let steps = max(1, Int((ms / 16).rounded()))
            let from = contact.at
            var last = from
            for i in 1...steps {
                let f = CGFloat(i) / CGFloat(steps)
                let p = i == steps ? to : CGPoint(x: from.x + (to.x - from.x) * f, y: from.y + (to.y - from.y) * f)
                contact.t += ms / 1000 / Double(steps)
                contact.at = p
                panPointer(contact, .move)
                presenter.onPanSample?(false, Double(p.x), Double(p.y), contact.t)
                let dx = p.x - last.x, dy = p.y - last.y
                last = p
                if dx != 0 || dy != 0 { contact.began = true; presenter.pan(node.id, Double(dx), Double(dy)) }
            }
            contact.at = to
            panContact = contact
            return ["phase": "move", "at": point(to), "delivery": "recognized"]
        case "hold":
            contact.t += ms.isFinite ? ms / 1000 : 0
            presenter.onPanSample?(false, Double(contact.at.x), Double(contact.at.y), contact.t)
            panContact = contact
            return ["phase": "hold", "at": point(contact.at), "delivery": "recognized"]
        case "up":
            panContact = nil
            panPointer(contact, .up)
            if contact.began {
                // The lift is one frame after the last move, where it lands
                // on the web and AppKit carriers too.
                contact.t += 1.0 / 60
                presenter.onPanSample?(false, Double(contact.at.x), Double(contact.at.y), contact.t)
                let (vx, vy) = presenter.panVelocity?(contact.t) ?? (0, 0)
                presenter.panRelease(node.id, vx, vy)
                return ["phase": "up", "at": point(contact.at), "delivery": "recognized", "velocity": [vx, vy]]
            }
            // A pan that never began is the node's press (rule 4).
            if node.handlers.contains("press") {
                presenter.press(node.id)
                return ["phase": "up", "at": point(contact.at), "delivery": "recognized", "pressed": Int(node.id)]
            }
            return ["phase": "up", "at": point(contact.at), "delivery": "recognized"]
        case "cancel":
            panContact = nil
            panPointer(contact, .up)
            if contact.began { presenter.panRelease(node.id, 0, 0) }
            return ["phase": "cancel", "at": point(contact.at), "delivery": "recognized"]
        default:
            return nil
        }
    }
}
#endif
