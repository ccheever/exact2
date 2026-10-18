// A held state (LLP 1122 D8, LLP 1121 stage 8 E2): the build channel's `hold`
// and the agent's `holdState` hold one state on one node as the host's own,
// with no handler told. `active` is the press of the node the press resolves
// to (`pressed`, so its scale and press paint ease as a real press's, at once
// on the agent's clock); `hover` (the Mac; iOS has none) and `focus-visible`
// that node's look (`lookState`). One at a time; a disabled node refuses one;
// real input on the node (its press's release, the pointer over it for a
// hover, its focus for a focus), its disabling and its going let it go, each
// said why (`dropped`).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

@MainActor
enum HeldState {
    static weak var view: NodeView?
    static var state: String?
    static var dropped: ((String) -> Void)?
    /// While the hold itself moves the node's state: not real input.
    static var setting = false
    /// While a dev loop's new generation replaces the tree: a held node
    /// going is let go unsaid, and the build channel's hold frame taken
    /// again after under the same plan (`frame`, `ExactApp.rehold`); under
    /// another the server replays it, its site corresponded.
    static var swapping = false
    /// The build channel's hold frame held now, and the plan it was held under.
    static var frame: (frame: [String: Any], plan: String?)?
    /// The revision of the hold frame being applied: the same frame taken
    /// again (resent, or `rehold`'s) replaces nothing.
    static var applying: Int?
    /// The hold frame revisions let go of here (Inspect.swift `holdCommand`).
    static var ended: [Int] = []
    static func end(_ revision: Int?) {
        guard let revision else { return }
        ended.append(revision)
        if ended.count > 64 { ended.removeFirst() }
    }

    /// The hold let go, said why (nil: unsaid); the node's state as the
    /// reason leaves it (`input`: the real state goes on as it is).
    static func letGo(_ reason: String?) {
        guard let state else { return }
        let held = view, told = dropped
        view = nil
        self.state = nil
        dropped = nil
        if let held, reason != "input" {
            setting = true
            switch state {
            case "active": held.pressed = false
            case "hover": held.lookState(hovered: false)
            default: held.lookState(focus: false)
            }
            setting = false
        }
        if let reason { told?(reason) }
    }
}

extension NodeView {
    /// Hold `state` on this node (or let any go: nil). The state held and
    /// the node, or why not: `gone`, `disabled`, or unsupported here.
    func hold(_ state: String?, dropped: ((String) -> Void)?) -> (held: String?, reason: String?, unsupported: Bool) {
        HeldState.letGo("replaced")
        guard let state else { return (nil, nil, false) }
        #if os(iOS) || os(tvOS)
        if state == "hover" { return (nil, nil, true) }
        #endif
        // `active`: the node the press resolves to, as a real press's.
        var node: NodeView? = self
        if state == "active" {
            while let n = node, !n.handlers.contains("press") { node = n.superview?.enclosingNode }
        }
        guard let node, node.window != nil else { return (nil, "gone", false) }
        if node.disabled { return (nil, "disabled", false) }
        HeldState.view = node
        HeldState.state = state
        HeldState.dropped = dropped
        HeldState.setting = true
        switch state {
        case "active": node.pressed = true
        case "hover": node.lookState(hovered: true)
        default: node.lookState(focus: true)
        }
        HeldState.setting = false
        return (state, nil, false)
    }

    /// Whether this node's `state` is held.
    func holds(_ state: String) -> Bool { HeldState.view === self && HeldState.state == state }

    /// Real input changed this node's `state`: a hold of it is let go.
    func heldInput(_ state: String) {
        if holds(state), !HeldState.setting { HeldState.letGo("input") }
    }

    /// The node went or was taken for another: its hold is let go.
    func holdGone() { if HeldState.view === self { HeldState.letGo(HeldState.swapping ? nil : "gone") } }

    /// What is held on this node, for `layout` (LLP 1122 D8): its state and
    /// the rows it shows by their wire names, the press paint's for `active`
    /// (`:active` lowered to it), the look's otherwise.
    var heldRows: [String: Any]? {
        guard HeldState.view === self, let state = HeldState.state else { return nil }
        let rows: [(String, BatchValue)]
        if state == "active" {
            rows = ["press_scale", "press_opacity", "press_opacity_to", "press_background_color", "press_color"].compactMap { key in style[key].map { (key, $0) } }
        } else {
            rows = (Self.look(state == "hover" ? "hover" : "focus_visible", in: style) ?? [:]).sorted { $0.key < $1.key }.map { ($0.key, $0.value) }
        }
        return ["state": state, "rows": rows.map { ["name": $0.0, "value": $0.1.key] }]
    }
}

extension ExactApp {
    /// After a dev loop's new generation: the build channel's hold frame
    /// taken again, under the same plan (under another, the server's replay).
    func rehold() {
        #if EXACT_INSPECT && !os(tvOS)
        let plan = BuildChannel.planDigest(self)
        MainActor.assumeIsolated {
            guard HeldState.view == nil, let kept = HeldState.frame, kept.plan == plan else { return }
            holdCommand(kept.frame)
        }
        #endif
    }
}

extension Agent {
    /// The agent's `{"op":"tap","id":V,"holdState":…}` (LLP 1012, LLP 1122
    /// D8): the state held on the node as the real one, or let go (null).
    func holdTap(_ req: [String: Any]) -> [String: Any] {
        let state = req["holdState"] as? String
        if req["holdState"] is NSNull == false, state == nil || !["active", "hover", "focus-visible"].contains(state!) {
            return ["error": "holdState is active, hover, focus-visible or null", "malformed": true]
        }
        guard let id = req["id"] as? Int, let view = presenter.views[UInt32(id)] else { return ["error": "no view \(req["id"] ?? "")"] }
        let r = view.hold(state, dropped: nil)
        if r.unsupported { return ["tapped": id, "holdState": state ?? NSNull(), "delivery": "unsupported"] }
        if let reason = r.reason { return ["error": reason == "disabled" ? "view \(id) is disabled: it holds no state" : "view \(id): no press there, nor any around it"] }
        return ["tapped": id, "holdState": r.held ?? NSNull(), "delivery": "recognized"]
    }
}

private extension PlatformView {
    /// The nearest node view at or above this view.
    var enclosingNode: NodeView? {
        var view: PlatformView? = self
        while let v = view { if let n = v as? NodeView { return n }; view = v.superview }
        return nil
    }
}
