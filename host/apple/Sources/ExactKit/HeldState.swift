// A held state (LLP 1122 D8, LLP 1121 stage 8 E2): the build channel's `hold`
// and the agent's `holdState` hold one state on one node as the host's own,
// with no handler told. The hold is kept apart from the node's real state and
// shown beside it: `active` is the press of the node the press resolves to
// (`showPress`, so its scale and press paint ease as a real press's, at once
// on the agent's clock), and a real press already down on it is taken over
// (its release or its leaving shows nothing, PressFeedback.swift); `hover`
// (the Mac; iOS has none) and `focus-visible` that node's look (`looked`,
// PressPaint.swift). One at a time; a disabled node, or one under a disabled
// node, refuses one. Real input on the node (a press or a key on it or inside
// it, the pointer coming over it or leaving it for a hover, the focus coming
// or going for a focus), its disabling and its going let it go, each said why
// (`dropped`); letting go leaves what the real state shows.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

@MainActor
enum HeldState {
    static weak var view: NodeView?
    static var state: String?
    /// The node the hold named (a press resolves to the node at or above it).
    static var asked: UInt32?
    static var dropped: ((String) -> Void)?
    /// Whether the pointer is over the held node (a hover's) and the focus
    /// in it (a focus's), as they were last seen: real input flips them.
    static var over = false, focused = false
    /// The pointer's hover path as it was last seen (the Mac), innermost first.
    static var pointerPath: [WeakNodeView] = []
    /// While a dev loop's new generation replaces the tree: the hold was let
    /// go unsaid before it (`swap`), real input is none, and the build
    /// channel's hold frame is taken again after under the same plan
    /// (`frame`, `ExactApp.rehold`); under another the server replays it,
    /// its site corresponded.
    static var swapping = false
    /// The build channel's hold frame held now, and the plan it was held under.
    static var frame: (frame: [String: Any], plan: String?)?
    /// The revision of the hold frame being applied: the same frame taken
    /// again (resent, or `rehold`'s) replaces nothing.
    static var applying: Int?
    /// The hold frame revisions let go of here (Inspect.swift `holdCommand`).
    static var ended: [Int] = []
    /// The build channel's reports of a hold, as they go (the tests).
    static var heard: (([String: Any]) -> Void)?
    nonisolated static let states: Set<String> = ["active", "hover", "focus-visible"]
    static func end(_ revision: Int?) {
        guard let revision else { return }
        ended.append(revision)
        if ended.count > 64 { ended.removeFirst() }
    }

    /// The hold let go, said why (nil: unsaid). The node shows its real
    /// state again: a press really down and inside it keeps its paint.
    static func letGo(_ reason: String?) {
        guard let state else { return }
        let held = view, told = dropped
        view = nil
        self.state = nil
        asked = nil
        dropped = nil
        if let held {
            if state == "active" { if !(held.pressed && !held.pressPaint.outside) { held.showPress(false) } } else { held.relook() }
        }
        if let reason { told?(reason) }
    }

    /// A dev loop's new generation is about to replace the tree: the hold
    /// let go unsaid, its frame kept for `rehold`.
    static func swap() {
        swapping = true
        letGo(nil)
    }

    /// Real input at `hit` (a press's hit, a key's focus): on the held node
    /// or inside it (through authored parents), it lets the hold go.
    static func input(at hit: PlatformView?) {
        guard !swapping, let held = view, let hit, within(hit, held) else { return }
        letGo("input")
    }

    /// The pointer's hover path moved (the Mac): a hover held lets go when
    /// the pointer comes over its node or leaves it, not as it moves within.
    static func hovered(_ path: [NodeView]) {
        pointerPath = path.map { WeakNodeView(view: $0) }
        guard !swapping, state == "hover", let held = view else { return }
        if path.contains(where: { $0 === held }) != over { letGo("input") }
    }

    /// The focus moved to `focus` (nil: none): a focus held lets go when the
    /// focus comes into its node or leaves it.
    static func focusMoved(to focus: PlatformView?) {
        guard !swapping, state == "focus-visible", let held = view else { return }
        if (focus.map { within($0, held) } ?? false) != focused { letGo("input") }
    }

    /// A node's props changed: a held node now disabled, or under a disabled
    /// one, lets its hold go.
    static func propsChanged() {
        if !swapping, let held = view, held.disabledAt { letGo("disabled") }
    }

    static func within(_ view: PlatformView, _ outer: NodeView) -> Bool {
        if let presenter = outer.presenter { return presenter.logicallyWithin(view, outer) }
        return view.isDescendant(of: outer)
    }

    /// Hold `state` on node `id` of `presenter` (or let any go: nil): `active`
    /// on the node its press resolves to through the logical tree (a flat
    /// leaf or an inline run, which have no view, included), `hover` and
    /// `focus-visible` on the node's own view. The state held, or why not:
    /// `gone`, `disabled`, or unsupported here (iOS's hover, or a state no
    /// host knows).
    static func hold(_ state: String?, on id: UInt32, in presenter: Presenter, dropped: ((String) -> Void)?) -> (held: String?, reason: String?, unsupported: Bool) {
        letGo("replaced")
        guard let state else { return (nil, nil, false) }
        guard states.contains(state) else { return (nil, nil, true) }
        #if os(iOS) || os(tvOS)
        if state == "hover" { return (nil, nil, true) }
        #endif
        var at: UInt32? = id, found: NodeView?
        while let n = at {
            if let view = presenter.views[n] {
                if view.disabled { return (nil, "disabled", false) }
                if state != "active" || view.handlers.contains("press") { found = view; break }
            } else if state != "active" { return (nil, "gone", false) }
            at = presenter.logicalParent(of: n)
        }
        guard let node = found, node.window != nil else { return (nil, "gone", false) }
        if node.disabledAt { return (nil, "disabled", false) }
        view = node
        self.state = state
        asked = id
        self.dropped = dropped
        over = pointerPath.contains { $0.view === node }
        focused = presenter.focusedNode.map { within($0, node) } ?? false
        if state == "active" { node.showPress(true) } else { node.relook() }
        return (state, nil, false)
    }

    /// What is held on node `id` of `presenter`, for `layout` and inspect
    /// (LLP 1122 D8), on the node the hold named or the one it resolved to:
    /// its state and the rows it shows by wire name with their CSS values, as
    /// the runner's `node` op says them: the press rows for `active`
    /// (`:active` lowers to them), else the look's rows.
    static func rows(for id: UInt32, in presenter: Presenter) -> [String: Any]? {
        guard let held = view, let state, held.id == id || asked == id, presenter.views[held.id] === held else { return nil }
        var rows: [[String: Any]] = []
        if state == "active" {
            let style = presenter.session.flatMap { s -> [String: Any]? in
                guard let data = s.agent("{\"op\":\"node\",\"id\":\(held.id)}").data(using: .utf8),
                      let node = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
                return node["style"] as? [String: Any]
            }
            for name in ["press_scale", "press_opacity", "press_opacity_to", "press_background_color", "press_color"] {
                if let style { if let row = style[name] as? [String: Any], let value = row["value"] { rows.append(["name": name, "value": value]) } }
                else if let value = held.style[name] { rows.append(["name": name, "value": css(value)]) } // (no runner: the batch's)
            }
        } else {
            // The runner's `node` op carries no looks: the batch's look rows, as the runner's CSS text writes them.
            let look = NodeView.look(state == "hover" ? "hover" : "focus_visible", in: held.pressPaint.lookBase ?? held.style) ?? [:]
            rows = look.keys.sorted().map { ["name": $0, "value": css(look[$0]!)] }
        }
        return ["state": state, "rows": rows]
    }

    /// A batch value as the runner's CSS text writes it (runner/src/agent/row.rs):
    /// a colour `#rrggbb` (`#rrggbbaa` translucent), a pair `light-dark(…, …)`.
    static func css(_ value: BatchValue) -> Any {
        func hex(_ c: [Double]) -> String {
            let b = c.map { String(format: "%02x", Int(min(255, max(0, $0.rounded())))) }
            return "#" + b[0] + b[1] + b[2] + (c[3] >= 255 ? "" : b[3])
        }
        if let c = value.numbers, c.count == 4 { return hex(c) }
        if let pair = value.array, pair.count == 2, let l = pair[0].numbers, let d = pair[1].numbers, l.count == 4, d.count == 4 {
            return "light-dark(\(hex(l)), \(hex(d)))"
        }
        return value.any
    }
}

extension NodeView {
    /// Hold `state` on this node (or let any go: nil), as `HeldState.hold`.
    func hold(_ state: String?, dropped: ((String) -> Void)?) -> (held: String?, reason: String?, unsupported: Bool) {
        guard let presenter else { HeldState.letGo("replaced"); return (nil, state == nil ? nil : "gone", false) }
        return HeldState.hold(state, on: id, in: presenter, dropped: dropped)
    }

    /// Whether this node's `state` is held.
    func holds(_ state: String) -> Bool { HeldState.view === self && HeldState.state == state }

    /// Whether this node or one above it (through authored parents) is disabled.
    var disabledAt: Bool {
        guard let presenter else { return disabled }
        return presenter.logicalAncestry(of: self).contains { ($0 as? NodeView)?.disabled == true }
    }

    /// The node went or was taken for another: its hold is let go.
    func holdGone() { if HeldState.view === self { HeldState.letGo(HeldState.swapping ? nil : "gone") } }
}

extension Presenter {
    /// The node above `id` in the logical tree: a paragraph's run's owner, a
    /// flat leaf's parent (iOS), else as the runner says it.
    func logicalParent(of id: UInt32) -> UInt32? {
        if let run = inlineText(id) { return run.parent }
        #if os(iOS) || os(tvOS)
        if let parent = flats.parent(of: id) { return parent }
        #endif
        if let view = views[id] { return logicalAncestry(of: view).dropFirst().first { $0 is NodeView }.map { ($0 as! NodeView).id } }
        guard let data = session?.agent("{\"op\":\"node\",\"id\":\(id)}").data(using: .utf8),
              let node = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        return (node["parent"] as? NSNumber)?.uint32Value
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
        if req["holdState"] is NSNull == false, state == nil || !HeldState.states.contains(state!) {
            return ["error": "holdState is active, hover, focus-visible or null", "malformed": true]
        }
        guard let id = req["id"] as? Int, let data = session.agent("{\"op\":\"node\",\"id\":\(id)}").data(using: .utf8),
              let node = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any], node["error"] == nil else { return ["error": "no view \(req["id"] ?? "")"] }
        let r = MainActor.assumeIsolated { HeldState.hold(state, on: UInt32(id), in: presenter, dropped: nil) }
        if r.unsupported { return ["tapped": id, "holdState": state ?? NSNull(), "delivery": "unsupported"] }
        if let reason = r.reason { return ["error": reason == "disabled" ? "view \(id) is disabled: it holds no state" : "view \(id): no press there, nor any around it"] }
        return ["tapped": id, "holdState": r.held ?? NSNull(), "delivery": "recognized"]
    }
}
