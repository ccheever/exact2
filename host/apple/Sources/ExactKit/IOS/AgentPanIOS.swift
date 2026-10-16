#if os(iOS) || os(tvOS)
import UIKit

/// @ref LLP 1057 §10.6 — the iOS carrier's one held contact (LLP 1035.003 D1).
/// UIKit synthesizes no touch (LLP 1008 §9), so the contact is `recognized`.
/// On a `pan` node its deltas take the presenter's pan path, as
/// `ContactLayoutPan`'s do, and `up` releases at the engine's tracker over the
/// synthesized samples, spaced `ms / steps` apart on the contact's own clock,
/// never the wall time between requests (LLP 1057.001 §3). Anywhere else it is
/// a finger held where it went down: its `pointerdown`, `pointermove`s and
/// `pointerup` as a real touch's, and on `up` the press a finger lifted there
/// makes, still on its target; the driver seeks the clock through a hold.
struct AgentPan {
    /// The `pan` node; nil for a finger (`pressing`), which presses on its lift.
    weak var node: NodeView?
    var pressing = false
    var pointer: HeldNode?
    /// The nodes its `pointerdown` went to, which hear its `pointerup`.
    var path: [HeldNode] = []
    /// A pressing contact's target: what the lift presses where the finger is.
    var target: HeldNode?
    /// The contact in the viewport's space, and its synthetic time in seconds.
    var at: CGPoint
    var t: Double
    var began = false
    /// The modifiers held through it, a chord prefix (`KeyCodes.held`).
    var held = ""
}

/// A node as a contact found it: its id and incarnation, so a view the pool
/// reuses for another node (`NodePool`) is never taken for it.
struct HeldNode {
    weak var node: NodeView?
    let id: UInt32, incarnation: UInt64
    init(_ node: NodeView) { self.node = node; id = node.id; incarnation = node.incarnation }
    /// Still that node of `presenter`'s tree.
    func live(in presenter: Presenter) -> NodeView? {
        guard let node, presenter.views[id] === node, node.id == id, node.incarnation == incarnation else { return nil }
        return node
    }
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
            // Through a sheet's or a lifted popover's authored parents, as `MountRelay`
            current = (v as? NodeView)?.presenter?.listenerParent(of: v) ?? v.superview
        }
        return nil
    }

    /// The nodes a finger on `view` sends its `pointerdown` and `pointerup`
    /// to, as `PointerRecognizer` resolves a real touch's: from the innermost
    /// enabled node hearing the pointer up, each that hears either.
    static func fingerPath(from view: UIView) -> [HeldNode] {
        #if os(iOS)
        return (pointerNode(from: view).map(PointerRecognizer.bubbling(from:)) ?? []).map(HeldNode.init)
        #else
        return [] // tvOS touches no node (PointerIOS.swift)
        #endif
    }

    /// A request's `modifiers` (`Shift+Meta`) as a chord prefix (`Shift+Meta+`).
    static func heldPrefix(_ req: [String: Any]) -> String {
        (req["modifiers"] as? String).map { $0.isEmpty || $0.hasSuffix("+") ? $0 : $0 + "+" } ?? ""
    }

    /// A viewport point (`box`'s space) in the window.
    func windowPoint(_ at: CGPoint) -> CGPoint {
        let vp = presenter.viewport
        return vp.convert(CGPoint(x: at.x + vp.contentOffset.x, y: at.y + vp.contentOffset.y), to: nil)
    }

    /// A finger's `pointerdown` or `pointerup` at `window` (a window point)
    /// to `path`, innermost first, as `PointerRecognizer` sends a real
    /// touch's: the down to each node still the tree's, enabled and hearing
    /// it, the up to each still the tree's (one its down disabled hears its
    /// release too), until a handler stops it; `held`, the modifiers.
    func fingerPointer(_ kind: PointerKind, _ path: [HeldNode], at window: CGPoint, held: String = "") {
        let name = kind == .down ? "pointerdown" : "pointerup"
        let nodes = path.compactMap { $0.live(in: presenter) }.filter { $0.handlers.contains(name) }
        guard !nodes.isEmpty else { return }
        presenter.propagating {
            for n in nodes where presenter.views[n.id] === n && (kind == .up || !n.disabled) {
                presenter.pointer(n.id, kind, fingerSample(kind, n, at: window, held: held))
                if presenter.propagationStopped { break }
            }
        }
    }

    private func fingerSample(_ kind: PointerKind, _ node: NodeView, at window: CGPoint, held: String) -> PointerSample {
        let local = node.convert(window, from: nil), box = node.contentBox(), client = presenter.client(window)
        return PointerSample(x: Double(local.x - box.minX), y: Double(local.y - box.minY),
            buttons: kind == .up ? 0 : 1, pressure: kind == .up ? 0 : 0.5, type: "touch", id: 2,
            clientX: Double(client.x), clientY: Double(client.y), held: held)
    }

    private func panPointer(_ contact: AgentPan, _ kind: PointerKind) {
        let window = windowPoint(contact.at)
        if kind != .move { fingerPointer(kind, contact.path, at: window, held: contact.held); return }
        guard let node = contact.pointer?.live(in: presenter), !node.disabled, node.handlers.contains("pointermove") else { return }
        presenter.pointer(node.id, .move, fingerSample(.move, node, at: window, held: contact.held))
    }

    /// A hold's time on the platform's clock: none under the agent's
    /// (`virtual`: the driver seeks the runner by it), else the main run
    /// loop turns for it, as AppKit's carrier does, so UIKit's own motion
    /// runs on. No UIKit recognizer sees the recognized contact either way.
    func holdTime(_ ms: Double, _ req: [String: Any]) {
        if ms > 0, ms.isFinite, req["virtual"] as? Bool != true { RunLoop.main.run(until: Date(timeIntervalSinceNow: ms / 1000)) }
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
            guard let v = view(req), let win = v.window else { return nil }
            let b = box(v)
            let at = CGPoint(x: (req["x"] as? Double) ?? b.midX, y: (req["y"] as? Double) ?? b.midY)
            guard at.x.isFinite, at.y.isFinite else { return ["error": "tap down: the point must be finite"] }
            let t = CACurrentMediaTime()
            guard let node = Agent.panNode(from: v) else {
                // A drag's contact (`pan`) is a pan's or none: a finger held elsewhere moves nothing.
                if req["pan"] != nil { return nil }
                // A finger held still: it lands only where the target is seen, as a tap's does.
                let p = windowPoint(at), seen = win.hitTest(p, with: nil)
                if let why = offscreen(v, box: CGRect(origin: at, size: .zero), hit: seen) ?? seen.flatMap({ obscured(v, at: p, hit: $0) }) {
                    return ["error": "tap #\(v.id) down: \(why)"]
                }
                let touched = seen ?? v
                let contact = AgentPan(node: nil, pressing: true, pointer: Agent.pointerNode(from: touched).map(HeldNode.init), path: Agent.fingerPath(from: touched),
                                       target: HeldNode(v), at: at, t: t, held: Agent.heldPrefix(req))
                panContact = contact
                panPointer(contact, .down)
                return ["contact": Int(v.id), "phase": "down", "at": point(at), "delivery": "recognized"]
            }
            presenter.onPanSample?(true, Double(at.x), Double(at.y), t)
            let contact = AgentPan(node: node, pointer: Agent.pointerNode(from: v).map(HeldNode.init), path: Agent.fingerPath(from: v), at: at, t: t, held: Agent.heldPrefix(req))
            panContact = contact
            panPointer(contact, .down)
            return ["contact": Int(node.id), "phase": "down", "at": point(at), "delivery": "recognized"]
        }
        guard var contact = panContact else { return nil }
        if contact.pressing { return fingerPhase(phase, req, &contact) }
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
            holdTime(ms, req)
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

    /// A finger held where it went down, off any `pan` node: a move is its
    /// `pointermove`s (nothing scrolls: `tap <scroller> drag` does), a hold
    /// moves its own time, and `up` is its `pointerup`, then the press a
    /// finger lifted there makes if it is still on its target (a lift beside
    /// it presses nothing, as a touch up outside a control does).
    private func fingerPhase(_ phase: String, _ req: [String: Any], _ contact: inout AgentPan) -> [String: Any]? {
        let point = { (p: CGPoint) in [Agent.r2(p.x), Agent.r2(p.y)] }
        let ms = max(0, (req["ms"] as? Double) ?? 0)
        switch phase {
        case "move":
            let to = CGPoint(x: (req["x"] as? Double) ?? Double(contact.at.x) + ((req["dx"] as? Double) ?? 0),
                             y: (req["y"] as? Double) ?? Double(contact.at.y) + ((req["dy"] as? Double) ?? 0))
            guard to.x.isFinite, to.y.isFinite, ms.isFinite else { return ["error": "tap move: the point and duration must be finite"] }
            let steps = max(1, Int((ms / 16).rounded())), from = contact.at
            for i in 1...steps {
                let f = CGFloat(i) / CGFloat(steps)
                contact.at = i == steps ? to : CGPoint(x: from.x + (to.x - from.x) * f, y: from.y + (to.y - from.y) * f)
                contact.t += ms / 1000 / Double(steps)
                panPointer(contact, .move)
            }
            panContact = contact
            return ["phase": "move", "at": point(to), "delivery": "recognized"]
        case "hold":
            contact.t += ms.isFinite ? ms / 1000 : 0
            panContact = contact
            holdTime(ms, req)
            return ["phase": "hold", "at": point(contact.at), "delivery": "recognized"]
        case "up", "cancel":
            panContact = nil
            panPointer(contact, .up)
            var reply: [String: Any] = ["phase": phase, "at": point(contact.at), "delivery": "recognized"]
            guard phase == "up", let target = contact.target?.live(in: presenter), target.window != nil else { return reply }
            let b = box(target)
            guard b.contains(contact.at) else { return reply }
            // The plain tap's press at that point, its pointer events already sent.
            var lift: [String: Any] = ["id": Int(target.id), "at": [Double(contact.at.x - b.minX), Double(contact.at.y - b.minY)], "lift": true]
            if !contact.held.isEmpty { lift["modifiers"] = contact.held }
            let lifted = tap(lift)
            if let error = lifted["error"] { reply["pressError"] = error } else { reply["pressed"] = lifted["pressed"] ?? NSNull() }
            return reply
        default:
            return nil
        }
    }
}
#endif
