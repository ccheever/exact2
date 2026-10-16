// Where a real finger aims when UIKit draws a node in its place (LLP 1080.000
// D4, LLP 1116.003): a header button's item in the navigation bar under
// `--chrome platform`. The authored node stays hidden in the content, so
// its own box is not where a person would touch.
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// Under `--chrome platform`, the navigation bar a header button's item
    /// sits in and the item's box in it (r5 weather's Cities, r5 mood
    /// journal's sheet Save). Nil for any other node; a refusal for an item
    /// UIKit has not placed.
    func barItem(_ node: NodeView) -> Result<(bar: UIView, item: CGRect), BarItemMissing>? {
        #if os(iOS)
        guard !ExactEnv.authoredChrome else { return nil }
        let nav = presenter.navigation
        let testId = node.props["testId"]
        for c in nav.controllers.values {
            guard let controller = c.navigationController, controller.topViewController === c, nav.barShows(controller),
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

    /// Under `--chrome platform`, the control UIKit shows for the item
    /// standing for the tab `node` is (or is in): the one accessibility
    /// element with the item's identifier, as VoiceOver and XCUITest find it
    /// (a `UITabBarItem` has no public frame). Nil for any other node; a
    /// refusal when none, or more than one, shows.
    func tabItem(_ node: NodeView) -> Result<UIView, AimRefusal>? {
        #if os(iOS)
        let nav = presenter.navigation
        guard nav.tabBarShows, let tabs = nav.container.flatMap({ NavigationTabs.of($0, presenter) }),
              let index = tabs.tabs.firstIndex(where: { node === $0 || node.isDescendant(of: $0) }),
              let controller = nav.tabController, let items = controller.tabBar.items, index < items.count else { return nil }
        // By the item's identifier (its tab's test id, `syncItems`), which
        // UIKit gives the control it shows for it, in the bar or, on an
        // iPad, at the top or in the sidebar; by its title only without one.
        let item = items[index], id = item.accessibilityIdentifier ?? "", title = item.title ?? ""
        guard !id.isEmpty || !title.isEmpty else { return .failure(AimRefusal(why: "its tab has neither a test id nor a title UIKit's tab bar shows; give it a testId")) }
        var found: [UIView] = [], pending: [UIView] = controller.view.subviews
        while !pending.isEmpty {
            let v = pending.removeFirst()
            guard !v.isHidden, v.alpha > 0.01, !(v is NodeView) else { continue }
            if v.isAccessibilityElement {
                let box = v.convert(v.bounds, to: nil)
                if id.isEmpty ? v.accessibilityLabel == title : v.accessibilityIdentifier == id,
                   box.width > 0, box.height > 0, v.window?.bounds.contains(CGPoint(x: box.midX, y: box.midY)) == true { found.append(v) }
                continue
            }
            pending.append(contentsOf: v.subviews)
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

    /// Why a finger would not reach the segment or bar item UIKit draws in
    /// `node`'s place (a sheet over it, the keyboard), or nil: the tap that
    /// activates either is refused as a covered node's is.
    func drawnCovered(_ node: NodeView) -> String? {
        let target: UIView, region: CGRect
        if let (control, segment) = presenter.segments.drawn(node) { (target, region) = (control, segment) }
        else if case .success(let (bar, frame))? = barItem(node) { (target, region) = (bar, frame) }
        else { return nil }
        guard let win = target.window else { return nil }
        let p = target.convert(CGPoint(x: region.midX, y: region.midY), to: nil)
        awaitAppOverlay(at: p, in: win)
        guard let hit = win.hitTest(p, with: nil) else { return "its middle is off the window" }
        return obscured(target, at: p, hit: hit)
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
