// Real touches under the agent (LLP 1080.000): the touch runner taps the
// device's screen through XCTest, and the app reports two things the driver
// cannot see from outside. `aim` resolves a target to a point in the scene's
// coordinate space, refusing by name, and changes nothing (D4). The dispatch
// log records each touch the window dispatched, after UIKit dispatched it
// (D5): window dispatch only — never a recognizer's outcome, a cancellation
// or a handled press, which `state` and `tree` answer.
#if os(iOS) || os(tvOS)
import UIKit

/// The app's window: an ordinary `UIWindow` that, under the agent only,
/// logs the touches it dispatched.
public final class ExactWindow: UIWindow {
    public override func sendEvent(_ event: UIEvent) {
        super.sendEvent(event)
        if ExactEnv.agentMode, event.type == .touches { TouchLog.record(event, in: self) }
    }
}

/// The last 256 touch dispatches, in order. A touch is a small integer from
/// the first time its `UITouch` is seen until it ends.
enum TouchLog {
    nonisolated(unsafe) static var entries: [[String: Any]] = []
    nonisolated(unsafe) static var seq = 0
    nonisolated(unsafe) static var dropped = 0
    nonisolated(unsafe) static var ids: [ObjectIdentifier: Int] = [:]
    nonisolated(unsafe) static var nextTouch = 1
    static let capacity = 256

    static func record(_ event: UIEvent, in window: UIWindow) {
        for touch in event.allTouches ?? [] {
            let phase: String
            switch touch.phase {
            case .began: phase = "began"
            case .moved: phase = "moved"
            case .ended: phase = "ended"
            default: continue // stationary, and cancelled, which reaches views and not this window
            }
            let key = ObjectIdentifier(touch)
            let id = ids[key] ?? { defer { nextTouch += 1 }; ids[key] = nextTouch; return nextTouch }()
            if phase == "ended" { ids[key] = nil }
            let p = touch.location(in: window)
            seq += 1
            var entry: [String: Any] = ["seq": seq, "touch": id, "phase": phase, "t": touch.timestamp,
                                        "at": [Agent.r2(p.x), Agent.r2(p.y)], "type": touch.type == .direct ? "direct" : "\(touch.type.rawValue)"]
            for (key, value) in landing(touch.view) { entry[key] = value }
            entries.append(entry)
            if entries.count > capacity { entries.removeFirst(); dropped += 1 }
        }
    }

    /// Where a touch's view sits: its session and that session's generation,
    /// the Exact node it is in, or the UIKit class it hit.
    static func landing(_ view: UIView?) -> [String: Any] {
        var out: [String: Any] = ["view": view.map { String(describing: type(of: $0)) } ?? NSNull()]
        var at = view
        while let v = at, !(v is NodeView) { at = v.superview }
        out["node"] = (at as? NodeView).map { Int($0.id) } ?? NSNull()
        // In a grouped list's cell, which row and which part of it: the
        // node alone is the list's for every row (LLP 1084).
        if let projected = GroupedListsLink.part?(view) { out["projected"] = projected }
        if let v = view {
            for (label, session) in Agent.routes where v.isDescendant(of: session.presenter.viewport) || session.view.map({ v.isDescendant(of: $0) }) == true {
                out["session"] = label
                out["generation"] = session.generation
                break
            }
        }
        return out
    }

    /// `{"op":"tap","log":S}`: the entries after `S`, at most 32.
    static func read(after s: Int) -> [String: Any] {
        let oldest = entries.first.map { $0["seq"] as! Int } ?? seq + 1
        let after = entries.filter { ($0["seq"] as! Int) > s }
        // `lost`: entries after `s` fell out of the ring before this read.
        return ["log": Array(after.prefix(32)), "next": seq, "dropped": dropped,
                "lost": s + 1 < oldest, "truncated": after.count > 32]
    }
}

extension Agent {
    /// Why a real touch on `v` would not test what a person's app does, or
    /// nil: under the agent the host substitutes some presentations, and
    /// their taps run through activation (LLP 1080.000 D7, stage 3).
    func substituted(_ v: NodeView) -> String? {
        if presenter.menus.ownsConfirmationNode(v) { return "is a confirmation's action (UIKit's alert)" }
        if presenter.swipeActions.ownsAction(v.id) { return "is a native swipe action" }
        if presenter.menus.agentPainted(v) { return "goes through the agent's painted popovers" }
        let nav = presenter.navigation
        // Under `--chrome platform` UIKit's bars draw these: a finger aims
        // at a node, and the node is not where the bar item is.
        let shown = ExactEnv.authoredChrome ? "the agent shows in place of" : "drawn by (--chrome platform; tap it without --touch platform)"
        if nav.tabController != nil, let list = nav.adoptedTablist.flatMap({ presenter.views[$0] }), v === list || v.isDescendant(of: list) {
            return "is in the authored tablist \(shown) UIKit's tab bar"
        }
        for controller in nav.allNavigations where nav.stacks[ObjectIdentifier(controller)]?.showsBar == true {
            for case let route as RouteController in controller.viewControllers {
                if let header = HeaderShape(route: route.node, back: nil)?.header, v === header || v.isDescendant(of: header) {
                    return "is in the authored header \(shown) UIKit's navigation bar"
                }
            }
        }
        return nil
    }

    /// `view` and the Exact nodes enclosing it, innermost first.
    static func enclosing(_ view: UIView?) -> [NodeView] {
        var out: [NodeView] = [], at = view
        while let v = at { if let node = v as? NodeView { out.append(node) }; at = v.superview }
        return out
    }

    /// The scene's interface orientation, by UIKit's name for it.
    static func orientation(_ scene: UIWindowScene?) -> String {
        #if os(tvOS)
        // tvOS has no interface orientation.
        return "unknown"
        #else
        switch scene?.effectiveGeometry.interfaceOrientation {
        case .portrait: return "portrait"
        case .portraitUpsideDown: return "portraitUpsideDown"
        case .landscapeLeft: return "landscapeLeft"
        case .landscapeRight: return "landscapeRight"
        default: return "unknown"
        }
        #endif
    }

    /// The private touch forms of `tap` (LLP 1080.000 D4/D5), or nil.
    func touchForm(_ req: [String: Any]) -> [String: Any]? {
        if let s = req["log"] as? Int { return TouchLog.read(after: s) }
        guard req["aim"] != nil else { return nil }
        return aim(req)
    }

    /// Where a finger aimed at `id` lands, in the scene's coordinate space,
    /// or why it may not: the refusals `tap` makes, computed with nothing
    /// pressed, focused or opened.
    func aim(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        if v.placedAncestor?.placementHidden == true { return ["error": "tap #\(v.id): placed child is hidden"] }
        // Said before where it sits: until a swipe reveals it, its node is
        // past the row's edge (splitter rough 12).
        if presenter.swipeActions.ownsAction(v.id) {
            return ["error": "unsupported: tap #\(v.id) is a native swipe action: a finger swipes its row (`tap <row> drag -<width> 0`; a full swipe performs the first trailing action), and without --touch platform `tap` performs it"]
        }
        let scene = win.windowScene
        guard UIApplication.shared.applicationState == .active, scene?.activationState == .foregroundActive, win.isKeyWindow else {
            return ["error": "tap #\(v.id): the app is not the foreground, key window"]
        }
        // The point: the request's, else the middle of the target — of a
        // visible shaped fragment for an inline id, as `tap` aims.
        var at = req
        if let offset = req["aim"] as? [String: Any] {
            // A point that is present must be one: never the middle in its place.
            for k in ["x", "y"] where offset[k] != nil {
                guard let n = offset[k] as? Double, n.isFinite else { return ["error": "tap #\(v.id): the aim's \(k) must be a finite number"] }
            }
            at["x"] = offset["x"]; at["y"] = offset["y"]
            // A point in the target (`tap <target> at <x> <y>`), from its top left.
            if let point = offset["at"] {
                guard let pair = point as? [Double], pair.count == 2, pair.allSatisfy(\.isFinite) else { return ["error": "tap #\(v.id): the aim's at must be two finite numbers"] }
                let whole = box(v)
                guard pair[0] >= 0, pair[1] >= 0, pair[0] < whole.width, pair[1] < whole.height else { return ["error": "tap #\(v.id) at: (\(pair[0]), \(pair[1])) is outside its \(Agent.r2(whole.width))×\(Agent.r2(whole.height)) box"] }
                at["at"] = pair
            }
        }
        // A plain tap's aim: the target's own press (`AgentAddressedTap.swift`).
        let press = (req["aim"] as? [String: Any])?["press"] as? Bool == true
        // A row a grouped list draws, or its toggle's or detail button's
        // control (LLP 1084 D5): the finger aims at UIKit's cell or
        // accessory, never the hidden authored node beneath it.
        var target: UIView = v, port: UIScrollView?
        switch presenter.groupedLists?.shown(v) {
        case .refused(let why): return ["error": "tap #\(v.id): \(why)"]
        case .view(let drawn, let list): target = drawn; port = list
        case nil: break
        }
        let drawnBox = target === v ? nil : box(target)
        guard var local = drawnBox.map({ CGPoint(x: at["x"] as? Double ?? $0.midX, y: at["y"] as? Double ?? $0.midY) }) ?? tapPoint(at, node: v) else {
            return ["error": "tap #\(req["id"] ?? v.id): no visible text fragment; scroll it into view first"]
        }
        let vp = presenter.viewport
        var p = vp.convert(CGPoint(x: local.x + vp.contentOffset.x, y: local.y + vp.contentOffset.y), to: nil)
        var seen = win.hitTest(p, with: nil)
        if !CGRect(origin: .zero, size: vp.bounds.size).contains(local) || seen == nil {
            return ["error": "tap #\(v.id): the point is outside the viewport; scroll it into view first"]
        }
        if let why = obscured(target, at: p, hit: seen!) { return ["error": "tap #\(v.id): \(why)"] }
        // Named, it presses what it names, as the activation's tap does.
        var avoided: PressReach?
        if press, target === v, at["at"] == nil, (req["id"] as? Int).map({ presenter.inlineText(UInt32($0)) == nil }) == true {
            switch addressedPoint(v, box: v.tapBox(box(v)), in: win) {
            case .refused(let refusal): return refusal.reply
            case .at(let q, let middle):
                if let middle {
                    avoided = middle
                    local = q
                    p = vp.convert(CGPoint(x: q.x + vp.contentOffset.x, y: q.y + vp.contentOffset.y), to: nil)
                    seen = win.hitTest(p, with: nil)
                }
            }
        }
        // What a list draws is hit itself, in the list's port: never an
        // ancestor beside a clipped cell, which the landing check would pass.
        if let port {
            guard port.convert(port.bounds, to: nil).contains(p) else {
                return ["error": "tap #\(v.id): the point is outside the list's port; scroll it into view first"]
            }
            guard seen === target || seen!.isDescendant(of: target) else {
                return ["error": "tap #\(v.id): \((seen as? NodeView).map { "node #\($0.id)" } ?? String(describing: Swift.type(of: seen!))) covers its middle"]
            }
        }
        // Stage 3's boundary, over the target, every node enclosing it (an
        // invoker around a label) and every node enclosing what the finger
        // would hit.
        for node in Agent.enclosing(v) + Agent.enclosing(seen) {
            if let why = substituted(node) {
                return ["error": "unsupported: tap #\(v.id)\(node === v ? "" : " (through node #\(node.id))") \(why); native presentation under a real touch lands at LLP 1080.000 stage 3"]
            }
        }
        guard let space = scene?.coordinateSpace else { return ["error": "tap #\(v.id): no scene"] }
        let s = win.convert(p, to: space)
        let origin = vp.convert(CGPoint(x: vp.contentOffset.x, y: vp.contentOffset.y), to: space)
        return ["aim": [
            "seq": TouchLog.seq, "session": session.label, "generation": session.generation,
            "orientation": Agent.orientation(scene),
            "screen": ["w": Agent.r2(space.bounds.width), "h": Agent.r2(space.bounds.height), "x": Agent.r2(origin.x), "y": Agent.r2(origin.y)],
            "point": [Agent.r2(s.x), Agent.r2(s.y)], "node": Int(v.id),
            "at": [Agent.r2(local.x), Agent.r2(local.y)], // the viewport point, as `tap` replies it
            "window": [Agent.r2(p.x), Agent.r2(p.y)], // the window point, as the dispatch log records touches
            "viewport": [Agent.r2(vp.bounds.width), Agent.r2(vp.bounds.height)], // where a drag must end
            // The Exact node the window's hit test finds there: where the dispatch log must see the touch land.
            "hit": TouchLog.landing(seen)["node"] ?? NSNull(),
            // In a grouped list, the row and part it must land on too.
            "projected": TouchLog.landing(seen)["projected"] ?? NSNull(),
            // Its middle reaches a control inside it: aimed beside it.
            "avoided": avoided.map { ["pressing": $0.pressing ?? NSNull(), "what": $0.described] as [String: Any] } ?? NSNull(),
        ] as [String: Any]]
    }
}
#endif
