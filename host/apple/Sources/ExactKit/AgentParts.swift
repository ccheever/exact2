// `tap <node>/<part>` (@ref LLP 1075.003.000.001 §3.5): a part a hatch drew
// is reached only as real platform input at its place, never by calling it.
// The host's side is the aim and the evidence; the driver delivers between
// them, a touch or a pointer event at the aimed point, as it does for a node.
//
//   {"op":"tap","id":V,"part":"seal","aim":true}
//       → {"aimed": {"at": [dx, dy], "token": T}}   the part's middle, from its node's corner
//       or the refusal by name: no such live part, out of its window, covered.
//   {"op":"tap","id":V,"part":"seal","landed":T}
//       → {"landed": "part" | "elsewhere", …}   against the token: the same part, the same
//         bound view, the same incarnation; a part replaced or a view swapped between the
//         aim and the delivery is refused as stale.
//
// The aim changes nothing. Nothing is activated by name, so LLP 1012's
// sentence stands: every path is a touch at a place.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Where the last delivered pointer event landed: the view the platform's own
/// hit test gave it, recorded by the delivery (AgentMac.swift's click; the
/// simulator's real-touch landing), read by a part's `landed`.
enum PartLanding {
    nonisolated(unsafe) static weak var view: PlatformView?
    nonisolated(unsafe) static var delivered = false
}

extension Agent {
    private func hit(_ point: CGPoint, in window: FocusWindow) -> PlatformView? {
        #if os(macOS)
        return window.contentView?.superview?.hitTest(point) ?? window.contentView?.hitTest(point)
        #else
        return window.hitTest(point, with: nil)
        #endif
    }

    func partTap(_ req: [String: Any]) -> [String: Any] {
        let name = req["part"] as? String ?? ""
        guard let id = (req["id"] as? NSNumber)?.uint32Value, let node = presenter.views[id] else { return ["error": "part \(name): no view \(req["id"] ?? "?")"] }
        guard let (_, view) = presenter.elements.regions.part(name, of: id) else { return ["error": "part \(name): view \(id) has no live part of that id"] }
        guard let window = view.window, node.window === window else { return ["error": "part \(name): it is not in the window"] }
        let middle = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
        let aimed = hit(view.convert(middle, to: nil), in: window)
        let clear = aimed.map { $0 === view || $0.isDescendant(of: view) } ?? false
        let token = "\(id)/\(name)/\(ObjectIdentifier(view).hashValue)/\(session.natives.hatchIncarnation)"
        let covered = aimed.map { "covered by \(String(describing: Swift.type(of: $0)))" } ?? "nothing is hit at its middle"
        if req["aim"] as? Bool == true {
            guard clear else { return ["error": "part \(name): \(covered)"] }
            PartLanding.view = nil
            PartLanding.delivered = false
            let at = view.convert(middle, to: node)
            return ["aimed": ["at": [Agent.r2(at.x), Agent.r2(at.y)], "token": token] as [String: Any]]
        }
        guard let given = req["landed"] as? String else { return ["error": "tap with a part aims (\"aim\": true) or reads the landing (\"landed\": token)"] }
        guard given == token else { return ["error": "part \(name): it changed between the aim and the delivery (a stale token)"] }
        guard PartLanding.delivered else { return ["error": "part \(name): no pointer event was delivered since the aim"] }
        // The evidence is the delivery's own hit view, by identity, not a second look at the part.
        let landing = PartLanding.view
        let onPart = landing.map { $0 === view || $0.isDescendant(of: view) } ?? false
        var out: [String: Any] = ["tapped": Int(id), "part": name, "delivery": "platform", "landed": onPart ? "part" : "elsewhere"]
        if !onPart, let landing {
            var reached: [String: Any] = ["class": String(describing: Swift.type(of: landing))]
            var at: PlatformView? = landing
            while let v = at, !(v is NodeView) { at = v.superview }
            if let owner = at as? NodeView { reached["node"] = Int(owner.id) }
            out["hit"] = reached
        }
        return out
    }
}
