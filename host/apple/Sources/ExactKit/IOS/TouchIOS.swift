// Real touches under the agent (LLP 1080.000): the touch runner taps the
// device's screen through XCTest, and the app reports two things the driver
// cannot see from outside. `aim` resolves a target to a point in the scene's
// coordinate space, refusing by name, and changes nothing (D4). The dispatch
// log records each touch the window dispatched, after UIKit dispatched it
// (D5): window dispatch only — never a recognizer's outcome, a cancellation
// or a handled press, which `state` and `tree` answer — and beside it which
// touches down have reached the view they landed on (`reached`).
#if os(iOS) || os(tvOS)
import UIKit

/// The app's window: an ordinary `UIWindow` that, under the agent only,
/// logs the touches it dispatched.
public final class ExactWindow: UIWindow {
    public override func sendEvent(_ event: UIEvent) {
        // Inspect mode takes every touch and press (InspectIOS.swift), and
        // knows the touches down when it begins. (It reached this window,
        // so the agent's touch log still has it.)
        #if EXACT_INSPECT && os(iOS)
        InspectHost.see(event)
        if Inspect.on || InspectHost.owning, InspectHost.take(event, in: self) {
            if ExactEnv.agentMode, event.type == .touches { TouchLog.record(event, in: self) }
            return
        }
        #endif
        // A contact going down, a native control's included, ends the live
        // press's focus handoff of the session it lands in before anything
        // hears it (PressHandoffIOS); one landing in none, every session's.
        for touch in event.allTouches ?? [] where event.type == .touches && touch.phase == .began {
            let sessions = ExactApp.shared.sessions
            let owners = touch.view.map { view in sessions.filter { $0.presenter.mounts(view) } } ?? []
            for session in owners.isEmpty ? sessions : owners { session.presenter.supersedePress() }
        }
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
    /// The touches down whose view's `touchesBegan` has had them, for the
    /// views that say so (a node, a native button: `reports`). UIKit hands a
    /// touch to its view after the window dispatched it, in a scroll view
    /// only once `delaysContentTouches` lets it go (about 150 ms), and a
    /// read inside a held touch waits for that (`touches.mjs`).
    nonisolated(unsafe) static var reached: Set<Int> = []
    static let capacity = 256

    /// Outside a scroll view a view's `touchesBegan` runs inside the
    /// window's `sendEvent`, before `record` sees the touch: either names it.
    static func reach(_ touches: Set<UITouch>) {
        for touch in touches { reached.insert(number(touch)) }
    }

    private static func number(_ touch: UITouch) -> Int {
        let key = ObjectIdentifier(touch)
        if let id = ids[key] { return id }
        defer { nextTouch += 1 }
        ids[key] = nextTouch
        return nextTouch
    }

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
            let id = number(touch)
            if phase == "ended" { ids[key] = nil; reached.remove(id) }
            let p = touch.location(in: window)
            seq += 1
            var entry: [String: Any] = ["seq": seq, "touch": id, "phase": phase, "t": touch.timestamp,
                                        "at": [Agent.r2(p.x), Agent.r2(p.y)], "type": touch.type == .direct ? "direct" : "\(touch.type.rawValue)"]
            // A touch a system recognizer takes (iOS 26's tab bar selects with
            // one on the window) has no view: where the window's hit test put it.
            for (key, value) in landing(touch.view ?? window.hitTest(p, with: nil)) { entry[key] = value }
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
        // Whether the view says when the touch reaches it (`reach`).
        out["reports"] = view is NodeView || view is NativeButtonIOS
        // In a grouped list's cell, which row and which part of it: the
        // node alone is the list's for every row (LLP 1084).
        if let projected = GroupedListsLink.part?(view) { out["projected"] = projected }
        if let v = view {
            for (label, session) in Agent.routes where Agent.shows(session, v) {
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
        return ["log": Array(after.prefix(32)), "next": seq, "dropped": dropped, "reached": reached.sorted(),
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
        if nav.tabController != nil, !nav.tablistAppearanceNone, let list = nav.adoptedTablist.flatMap({ presenter.views[$0] }), v === list || v.isDescendant(of: list) {
            return "is in the authored tablist \(shown) UIKit's tab bar"
        }
        for controller in nav.allNavigations where nav.stacks[ObjectIdentifier(controller)]?.showsBar == true {
            for case let route as RouteController in controller.viewControllers {
                guard let shape = HeaderShape.bar(of: route.node) else { continue }
                let replaced = shape.authored.isEmpty ? [shape.header] : shape.placed
                if replaced.contains(where: { v === $0 || v.isDescendant(of: $0) }) {
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
        settleForPress()
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
        // A row of a menu UIKit shows (native presentation): the finger
        // aims at UIKit's row, never at the hidden node.
        if let (pop, row) = presenter.menus.nativeRow(v) {
            guard let space = scene?.coordinateSpace else { return ["error": "tap #\(v.id): no scene"] }
            let box: CGRect
            switch shownRow(row, of: pop, in: win) {
            case .failure(let refusal): return ["error": "tap #\(v.id): \(refusal.why)"]
            case .success(let shown): box = shown
            }
            let p = CGPoint(x: box.midX, y: box.midY), vp = presenter.viewport, q = vp.convert(p, from: nil)
            let label = presenter.menus.given[pop.id]?.first { $0.row == row.id }?.label ?? ""
            return aimed(v, at: p, local: CGPoint(x: q.x - vp.contentOffset.x, y: q.y - vp.contentOffset.y), in: win, space: space,
                         hit: nil, native: ["kind": "menuItem", "label": label, "row": Int(row.id)])
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
                at["at"] = pair
            }
        }
        // A plain tap's aim: the target's own press (`AgentAddressedTap.swift`).
        let press = (req["aim"] as? [String: Any])?["press"] as? Bool == true
        // A row a grouped list draws, or its toggle's or detail button's
        // control (LLP 1084 D5): the finger aims at UIKit's cell or
        // accessory, never the hidden authored node beneath it.
        var target: UIView = v, port: UIScrollView?, region: CGRect?
        switch presenter.groupedLists?.shown(v) {
        case .refused(let why): return ["error": "tap #\(v.id): \(why)"]
        case .view(let drawn, let list): target = drawn; port = list
        case nil: break
        }
        // A tab a segmented control draws: its segment, the control taking
        // the touch (r5 weather). A header button UIKit's bar draws under
        // `--chrome platform`: its bar item (r5 weather, mood journal).
        if target === v, let (control, segment) = presenter.segments.drawn(v) { target = control; region = segment }
        var barred = false
        if target === v, let item = barItem(v) {
            guard case .success(let (bar, frame)) = item else { return ["error": "tap #\(v.id): its bar item is not placed in the navigation bar; `clock settle` first waits out a push or sheet still moving"] }
            target = bar; region = frame; barred = true
        }
        // A tab UIKit's tab bar shows: its item there.
        if target === v, let item = tabItem(v) {
            switch item {
            case .failure(let refusal): return ["error": "tap #\(v.id): \(refusal.why)"]
            case .success(let control): target = control; region = control.bounds; barred = true
            }
        }
        let drawnBox = target === v ? nil : box(target, region: region)
        // A point from the top left of what is drawn: UIKit's cell for a row a list draws.
        if let pair = at["at"] as? [Double] {
            let whole = drawnBox ?? box(v)
            guard pair[0] >= 0, pair[1] >= 0, pair[0] < whole.width, pair[1] < whole.height else { return ["error": "tap #\(v.id) at: (\(pair[0]), \(pair[1])) is outside its \(Agent.r2(whole.width))×\(Agent.r2(whole.height)) box"] }
        }
        let drawnAt = { (b: CGRect) -> CGPoint in
            if let pair = at["at"] as? [Double] { return CGPoint(x: b.minX + pair[0], y: b.minY + pair[1]) }
            return CGPoint(x: at["x"] as? Double ?? b.midX, y: at["y"] as? Double ?? b.midY)
        }
        guard var local = drawnBox.map(drawnAt) ?? tapPoint(at, node: v) else {
            return ["error": "tap #\(req["id"] ?? v.id): no visible text fragment; scroll it into view first"]
        }
        let vp = presenter.viewport
        var p = vp.convert(CGPoint(x: local.x + vp.contentOffset.x, y: local.y + vp.contentOffset.y), to: nil)
        awaitAppOverlay(at: p, in: win)
        var seen = win.hitTest(p, with: nil)
        if !presenter.reach(of: v).contains(local) || seen == nil {
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
                return ["error": "tap #\(v.id): \(Agent.covers(seen!))"]
            }
        }
        // Stage 3's boundary, over the target, every node enclosing it (an
        // invoker around a label) and every node enclosing what the finger
        // would hit.
        for node in (barred ? [] : Agent.enclosing(v)) + Agent.enclosing(seen) {
            if let why = substituted(node) {
                return ["error": "unsupported: tap #\(v.id)\(node === v ? "" : " (through node #\(node.id))") \(why); native presentation under a real touch lands at LLP 1080.000 stage 3"]
            }
        }
        guard let space = scene?.coordinateSpace else { return ["error": "tap #\(v.id): no scene"] }
        return aimed(v, at: p, local: local, in: win, space: space, hit: seen, avoided: avoided)
    }

    /// The aim's reply: the window point `p` in the scene's space (`local`
    /// in the viewport's), and where the dispatch log must see it land: the
    /// Exact node `seen` is in, or no node for UIKit's own row (`native`).
    private func aimed(_ v: NodeView, at p: CGPoint, local: CGPoint, in win: UIWindow, space: UICoordinateSpace, hit seen: UIView?,
                       avoided: PressReach? = nil, native: [String: Any]? = nil) -> [String: Any] {
        let vp = presenter.viewport, s = win.convert(p, to: space), fixed = win.convert(p, to: win.screen.fixedCoordinateSpace)
        let origin = vp.convert(CGPoint(x: vp.contentOffset.x, y: vp.contentOffset.y), to: space)
        var aim: [String: Any] = [
            "seq": TouchLog.seq, "session": session.label, "generation": session.generation,
            "orientation": Agent.orientation(win.windowScene),
            "screen": ["w": Agent.r2(space.bounds.width), "h": Agent.r2(space.bounds.height), "x": Agent.r2(origin.x), "y": Agent.r2(origin.y)],
            "point": [Agent.r2(s.x), Agent.r2(s.y)], "node": Int(v.id),
            "fixed": [Agent.r2(fixed.x), Agent.r2(fixed.y)], // the screen's portrait space, where XCTest's synthesizer presses (touches.swift)
            "at": [Agent.r2(local.x), Agent.r2(local.y)], // the viewport point, as `tap` replies it
            "window": [Agent.r2(p.x), Agent.r2(p.y)], // the window point, as the dispatch log records touches
            "viewport": [Agent.r2(vp.bounds.width), Agent.r2(vp.bounds.height)], // where a drag must end
            // The Exact node the window's hit test finds there: where the dispatch log must see the touch land.
            "hit": TouchLog.landing(seen)["node"] ?? NSNull(),
            // In a grouped list, the row and part it must land on too.
            "projected": TouchLog.landing(seen)["projected"] ?? NSNull(),
            // Its middle reaches a control inside it: aimed beside it.
            "avoided": avoided.map { ["pressing": $0.pressing ?? NSNull(), "what": $0.described] as [String: Any] } ?? NSNull(),
        ]
        if let native { aim["native"] = native }
        return ["aim": aim]
    }

    /// Where UIKit shows `row` of the menu it presents for `pop`, or why a
    /// finger cannot pick it. UIKit puts no identifier on the element it
    /// shows for an action, so the row is found by its popover and place:
    /// `pop` must be the menu UIKit was last given to present, or one of its
    /// submenus (`MenuHost.presentedMenu`), and its rows (`MenuHost.given`) must
    /// read as exactly one run of the rows shown (in a menu that scrolls,
    /// the rows it shows as one slice of them); the row is the one at its
    /// place there. A row of another popover, however alike its rows read,
    /// is refused, and so are two popovers of the presented menu that read
    /// the same, a menu shown twice, and a row scrolled out of its menu.
    func shownRow(_ row: NodeView, of pop: NodeView, in window: UIWindow) -> Result<CGRect, AimRefusal> {
        let menus = presenter.menus, name = Agent.popoverName(pop)
        guard let given = menus.given[pop.id], menus.presentedMenu[pop.id] == pop.incarnation,
              let place = given.firstIndex(where: { $0.row == row.id && $0.incarnation == row.incarnation }) else {
            let shown = menus.presentedMenu.keys.sorted().first.flatMap { presenter.views[$0] }.map { " (UIKit was last given \(Agent.popoverName($0))'s)" } ?? ""
            return .failure(AimRefusal(why: "it is a row of the menu of \(name), which is not the menu UIKit shows\(shown): tap its invoker first"))
        }
        let labels = given.map(\.label)
        if let twin = menus.presentedMenu.keys.sorted().first(where: { $0 != pop.id && menus.given[$0]?.map(\.label) == labels }) {
            return .failure(AimRefusal(why: "\(name) and popover #\(twin) are both in the menu UIKit shows and read the same; give their rows different titles"))
        }
        // Where the menu shows: a run of all its rows, or, in a list that
        // scrolls, the rows the list shows as one slice of them (`first`:
        // the place of the slice's first row).
        var found: [(rows: [(label: String, box: CGRect)], first: Int)] = []
        for group in Agent.menuGroups(in: window) {
            if group.scrolls, !group.rows.isEmpty, group.rows.count <= labels.count {
                for k in 0...(labels.count - group.rows.count) where zip(group.rows, labels[k...]).allSatisfy({ $0.label == $1 }) { found.append((group.rows, k)) }
                continue
            }
            for i in group.rows.indices where i + labels.count <= group.rows.count && zip(group.rows[i...], labels).allSatisfy({ $0.label == $1 }) {
                found.append((Array(group.rows[i..<i + labels.count]), 0))
            }
        }
        guard found.count == 1 else {
            return .failure(AimRefusal(why: found.isEmpty
                ? "the menu of \(name) does not show its rows: tap its invoker first, then `clock settle`"
                : "\(found.count) places UIKit shows read as the menu of \(name) (another menu, or a scrolled slice whose rows repeat); close the others first, or drive it without --touch platform"))
        }
        let (rows, first) = found[0]
        guard place >= first, place < first + rows.count else {
            return .failure(AimRefusal(why: "its row is scrolled out of the menu of \(name), and the agent's finger picks only rows the menu shows: drive it without --touch platform, where `tap` opens the menu painted"))
        }
        return .success(rows[place - first].box)
    }

    static func popoverName(_ pop: NodeView) -> String { "popover #\(pop.id)\(pop.props["id"].map { " (\($0))" } ?? "")" }

    /// Whether UIKit shows the menu it was last given (`shownRow`'s match),
    /// whatever presents it: a pull-down, a popover.
    func menuShown(in window: UIWindow) -> Bool {
        let groups = Agent.menuGroups(in: window).map { ($0.scrolls, $0.rows.map(\.label)) }
        return presenter.menus.presentedMenu.keys.contains { id in
            guard let labels = presenter.menus.given[id]?.map(\.label), !labels.isEmpty else { return false }
            return groups.contains { scrolls, group in
                // All of its rows; in a list that scrolls, a slice of them.
                let (whole, part) = scrolls ? (labels, group) : (group, labels)
                return !part.isEmpty && whole.indices.contains { $0 + part.count <= whole.count && Array(whole[$0..<$0 + part.count]) == part }
            }
        }
    }

    /// The labelled accessibility elements the window shows outside every
    /// session, as VoiceOver finds them (none inside another), grouped by
    /// the scroll view that holds them (a menu's list) and in reading order
    /// within each: what a menu UIKit shows reads as. `scrolls`: the list
    /// holds more than it shows, and rows off the screen are not among these.
    static func menuGroups(in window: UIWindow) -> [(scrolls: Bool, rows: [(label: String, box: CGRect)])] {
        var groups: [ObjectIdentifier: (list: UIView, rows: [(label: String, box: CGRect)])] = [:], order: [ObjectIdentifier] = []
        func visit(_ v: UIView, in list: UIView) {
            guard !v.isHidden, v.alpha > 0.01, !(v is NodeView), !routes.contains(where: { shows($0.1, v) }) else { return }
            if v.isAccessibilityElement {
                let box = v.convert(v.bounds, to: window), middle = CGPoint(x: box.midX, y: box.midY)
                // In a list, only a row it shows: not one scrolled under its edge.
                let shown = (list as? UIScrollView).map { $0.convert($0.bounds, to: window) } ?? window.bounds
                guard let label = v.accessibilityLabel, !label.isEmpty, box.width > 0, box.height > 0,
                      window.bounds.contains(middle), shown.contains(middle) else { return }
                let key = ObjectIdentifier(list)
                if groups[key] == nil { order.append(key); groups[key] = (list, []) }
                groups[key]?.rows.append((label, box))
                return
            }
            for sub in v.subviews { visit(sub, in: v is UIScrollView ? v : list) }
        }
        for sub in window.subviews where !routes.contains(where: { $0.1.view.map { $0.isDescendant(of: sub) } == true }) { visit(sub, in: sub) }
        return order.compactMap { groups[$0] }.map { group in
            let scrolls = (group.list as? UIScrollView).map { $0.contentSize.height > $0.bounds.height + 1 } ?? false
            return (scrolls, group.rows.sorted { ($0.box.minY, $0.box.minX) < ($1.box.minY, $1.box.minX) })
        }
    }
}

/// A row given to a menu UIKit shows: its node, and the label UIKit's row
/// reads (its title, then its subtitle after a comma).
struct GivenRow {
    let row: UInt32, incarnation: UInt64, label: String
    init(_ node: NodeView, title: String, subtitle: String?) {
        row = node.id; incarnation = node.incarnation
        label = subtitle.map { $0.isEmpty ? title : "\(title), \($0)" } ?? title
    }
}

/// Why a finger cannot aim at what UIKit shows for a node (a menu's row,
/// a tab's item).
struct AimRefusal: Error { let why: String }
#endif
