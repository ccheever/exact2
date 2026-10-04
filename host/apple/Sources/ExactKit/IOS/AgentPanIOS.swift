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
            panContact = AgentPan(node: node, at: at, t: t)
            return ["contact": Int(node.id), "phase": "down", "at": point(at), "delivery": "recognized"]
        }
        guard var contact = panContact else { return nil }
        guard let node = contact.node, presenter.views[node.id] === node, node.handlers.contains("pan"), SwipeInput.allows(node) else {
            // The node went or stopped panning: the contact is over, at rest.
            panContact = nil
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
            if contact.began { presenter.panRelease(node.id, 0, 0) }
            return ["phase": "cancel", "at": point(contact.at), "delivery": "recognized"]
        default:
            return nil
        }
    }
}
#endif
