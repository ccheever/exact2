// Where a real finger aims when UIKit draws a node in its place (LLP 1080.000
// D4, LLP 1116.003): a header button's item in the navigation bar under
// `--chrome platform`. The authored node stays hidden in the content, so
// its own box is not where a person would touch.
#if os(iOS) || os(tvOS)
import UIKit

extension NavigationHost {
    /// The search field a shown route projects into UIKit's search bar.
    func drawnSearch(_ node: NodeView) -> UITextField? {
        #if os(iOS)
        guard !ExactEnv.authoredChrome else { return nil }
        for c in controllers.values {
            guard let nav = c.navigationController, nav.topViewController === c, barShows(nav),
                  c.viewIfLoaded?.window != nil, let search = c.search, search.field === node else { continue }
            return search.controller.searchBar.searchTextField
        }
        #endif
        return nil
    }

    /// Under `--chrome platform`, the navigation bar a header button's item
    /// sits in and the item's box in it: where a finger aims, and where its
    /// pull-down opens. Nil for any other node; a refusal for an item UIKit
    /// has not placed.
    func barItem(_ node: NodeView) -> Result<(bar: UIView, item: CGRect), BarItemMissing>? {
        #if os(iOS)
        guard !ExactEnv.authoredChrome else { return nil }
        let testId = node.props["testId"]
        for c in controllers.values {
            guard let controller = c.navigationController, controller.topViewController === c, barShows(controller),
                  c.viewIfLoaded?.window != nil else { continue }
            let items = (c.navigationItem.leftBarButtonItems ?? []) + (c.navigationItem.rightBarButtonItems ?? [])
            // A pull-down's item has no target: its test id names it.
            guard let item = items.first(where: { ($0.target as? BarPress)?.id == node.id })
                    ?? items.first(where: { $0.target == nil && testId != nil && $0.accessibilityIdentifier == testId }) else { continue }
            let bar = controller.navigationBar
            guard let frame = item.frame(in: bar), frame.width > 0, frame.height > 0 else { return .failure(BarItemMissing()) }
            return .success((bar, frame))
        }
        #endif
        return nil
    }
}

extension Agent {
    /// A compact control can occupy only one end of its authored box.
    func nativeControlPoint(_ node: NodeView, box b: CGRect, in window: UIWindow) -> AddressedAim? {
        guard let control = presenter.controls.controls[node.id], control.window === window else { return nil }
        let cb = box(control), point = CGPoint(x: cb.midX, y: cb.midY), middle = CGPoint(x: b.midX, y: b.midY)
        guard point != middle else { return nil }
        let vp = presenter.viewport
        let toWindow = { (q: CGPoint) in vp.convert(CGPoint(x: q.x + vp.pageOrigin.x, y: q.y + vp.pageOrigin.y), to: nil) }
        guard pressReach(node, at: toWindow(point), in: window)?.isOwn(node) == true else { return nil }
        return .at(point, pressReach(node, at: toWindow(middle), in: window) ?? .nothing)
    }

    /// Under `--chrome platform`, the navigation bar a header button's item
    /// sits in and the item's box in it (r5 weather's Cities, r5 mood
    /// journal's sheet Save). Nil for any other node; a refusal for an item
    /// UIKit has not placed.
    func barItem(_ node: NodeView) -> Result<(bar: UIView, item: CGRect), BarItemMissing>? {
        presenter.navigation.barItem(node)
    }

    /// The control UIKit shows for the item standing for the tab `node` is
    /// (or is in), as VoiceOver and XCUITest find it (a `UITabBarItem` has no
    /// public frame): in the tab bar its tablist is drawn as (`drawnItem`),
    /// or under `--chrome platform` in UIKit's tab bar, the one accessibility
    /// element with the item's identifier. Nil for any other node; a refusal
    /// when none, or more than one, shows, or the item is outside the window.
    func tabItem(_ node: NodeView) -> Result<UIView, AimRefusal>? {
        #if os(iOS)
        if let (bar, item) = presenter.segments.barItem(of: node) { return drawnItem(item, in: bar) }
        let nav = presenter.navigation
        guard nav.tabBarShows, let tabs = nav.container.flatMap({ NavigationTabs.of($0, presenter) }),
              let index = tabs.tabs.firstIndex(where: { node === $0 || node.isDescendant(of: $0) }),
              let controller = nav.tabController, let items = controller.tabBar.items, index < items.count else { return nil }
        // By the item's identifier (its tab's test id, `syncItems`), which
        // UIKit gives the control it shows for it, in the bar or, on an
        // iPad, at the top or in the sidebar; by its title only without one.
        let item = items[index], id = item.accessibilityIdentifier ?? "", title = item.title ?? ""
        guard !id.isEmpty || !title.isEmpty else { return .failure(AimRefusal(why: "its tab has neither a test id nor a title UIKit's tab bar shows; give it a testId")) }
        let found = Self.elements(under: controller.view.subviews).filter { v in
            let box = v.convert(v.bounds, to: nil)
            return (id.isEmpty ? v.accessibilityLabel == title : v.accessibilityIdentifier == id) && v.window?.bounds.contains(CGPoint(x: box.midX, y: box.midY)) == true
        }
        guard found.count == 1 else {
            return .failure(AimRefusal(why: found.isEmpty ? "UIKit's tab bar shows no item for its tab; `clock settle` first waits out a transition"
                : "UIKit shows \(found.count) tab items as \(id.isEmpty ? "\"\(title)\"" : id); give each tab its own testId"))
        }
        return .success(found[0])
        #else
        return nil
        #endif
    }

    #if os(iOS)
    /// The views under `roots` that an accessibility client reads as
    /// elements, shown and with a box; the walk does not enter nodes.
    private static func elements(under roots: [UIView]) -> [UIView] {
        var found: [UIView] = [], pending = roots
        while !pending.isEmpty {
            let v = pending.removeFirst()
            guard !v.isHidden, v.alpha > 0.01, !(v is NodeView) else { continue }
            if v.isAccessibilityElement {
                if v.bounds.width > 0, v.bounds.height > 0 { found.append(v) }
                continue
            }
            pending.append(contentsOf: v.subviews)
        }
        return found
    }

    /// The control the tab bar a tablist is drawn as (LLP 1059) shows for
    /// `item`. That bar centres its items in a pill narrower than its box, so
    /// the hidden tab's box is not where its item is. By the item's
    /// identifier, else by its name or title: iOS 27.1's tab button carries
    /// its item's identifier only once an accessibility client has asked. The
    /// bar draws each button twice for its glass, so copies at one box count
    /// once: the copy a finger at its middle reaches.
    private func drawnItem(_ item: UITabBarItem, in bar: UITabBar) -> Result<UIView, AimRefusal> {
        let id = item.accessibilityIdentifier ?? "", names = [item.accessibilityLabel, item.title].compactMap { $0 }.filter { !$0.isEmpty }
        let controls = Self.elements(under: [bar]).filter { $0 is UIControl }
        var found = id.isEmpty ? [] : controls.filter { $0.accessibilityIdentifier == id }
        let byName = found.isEmpty
        if byName { found = controls.filter { $0.accessibilityLabel.map(names.contains) == true } }
        var shown: [(box: CGRect, control: UIView)] = []
        for v in found {
            let box = v.convert(v.bounds, to: nil), middle = CGPoint(x: box.midX, y: box.midY)
            let reached = v.window?.hitTest(middle, with: nil).map { $0 === v || $0.isDescendant(of: v) } == true
            if let i = shown.firstIndex(where: { abs($0.box.midX - box.midX) < 1 && abs($0.box.midY - box.midY) < 1 }) {
                if reached { shown[i].control = v }
            } else { shown.append((box, v)) }
        }
        guard shown.count == 1 else {
            return .failure(AimRefusal(why: shown.isEmpty ? "the tab bar its tablist is drawn as shows no item for its tab; `clock settle` first waits out a transition"
                : byName ? "the tab bar its tablist is drawn as shows \(shown.count) items named \"\(names.first ?? "")\"; give each tab its own name"
                : "the tab bar its tablist is drawn as shows \(shown.count) items as \(id); give each tab its own testId"))
        }
        let (box, control) = shown[0]
        guard control.window?.bounds.contains(CGPoint(x: box.midX, y: box.midY)) == true else {
            return .failure(AimRefusal(why: "its item is outside the window; scroll it into view first"))
        }
        return .success(control)
    }
    #endif

    /// Why a finger would not reach the segment or bar item UIKit draws in
    /// `node`'s place (a sheet over it, the keyboard), or nil: the tap that
    /// activates either is refused as a covered node's is.
    func drawnCovered(_ node: NodeView) -> String? {
        let target: UIView, region: CGRect, inBar: Bool
        if let (control, segment) = presenter.segments.drawn(node) { (target, region, inBar) = (control, segment, false) }
        else if case .success(let (bar, frame))? = barItem(node) { (target, region, inBar) = (bar, frame, true) }
        else { return nil }
        guard let win = target.window else { return nil }
        let p = target.convert(CGPoint(x: region.midX, y: region.midY), to: nil)
        awaitAppOverlay(at: p, in: win)
        guard let hit = win.hitTest(p, with: nil) else { return "its middle is off the window" }
        return obscured(inBar ? barItemTarget(node, bar: target, hit: hit) : target, at: p, hit: hit)
    }

    /// What a hit at a bar item's middle must land in: the item's own button
    /// when the hit is in it, found by the identifier Exact gives the item
    /// (its test id) among the views this session shows, since UIKit can host
    /// a glass item's button outside the bar's views; otherwise the bar. An
    /// item without a test id has no identifier to find its button by, so
    /// only the bar counts for it.
    func barItemTarget(_ node: NodeView, bar: UIView, hit: UIView) -> UIView {
        guard let id = node.props["testId"] else { return bar }
        let chain = sequence(first: hit, next: { $0.superview })
        // Fall back to the bar in a nested session's view, which `Agent.shows` counts through this session's view
        if let nearest = chain.lazy.compactMap({ $0 as? ExactView }).first, nearest.session !== session { return bar }
        return chain.first { $0.accessibilityIdentifier == id && Agent.shows(session, $0) } ?? bar
    }

    /// Whether `view` is in what `session` shows: its viewport, its view, or
    /// a navigation controller or tab bar controller of its own, a presented
    /// sheet's included, whose bars are outside both.
    static func shows(_ session: ExactSession, _ view: UIView) -> Bool {
        if session.presenter.mounts(view) || session.view.map({ view.isDescendant(of: $0) }) == true { return true }
        // A dialog's sheet holds the session's own nodes, outside its view (LLP 1115.000 D11.5).
        if session.presenter.menus.sheets.shows(view) { return true }
        let nav = session.presenter.navigation
        if let tabs = nav.tabController, view.isDescendant(of: tabs.view) { return true }
        return nav.allNavigations.contains { $0.viewIfLoaded.map { view.isDescendant(of: $0) } == true }
    }
}

/// A header button's bar item that UIKit has not placed in its bar.
struct BarItemMissing: Error {}
#endif
