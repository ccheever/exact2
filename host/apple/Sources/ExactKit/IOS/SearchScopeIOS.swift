// A search's scope bar (LLP 1134): a filter tablist right after a header
// whose search field the bar projects is that search bar's scope buttons, as
// a hand-built iPhone screen with a large title, a search and a filter has
// it, riding the bar as the title collapses.
#if os(iOS) || os(tvOS)
import UIKit

extension NavigationHost {
    /// Whether the search bar draws `tablist` (`SegmentHost` steps aside).
    func scopes(tablist: NodeView) -> Bool { controllers.values.contains { $0.scoped === tablist } }

    /// The scope bar's segmented control and the segment drawn for `tab`.
    func scopeSegment(_ tab: NodeView) -> (control: UIControl, segment: CGRect)? {
        #if os(iOS)
        guard let c = controllers.values.first(where: { $0.search?.scopeTabs.contains(tab.id) == true }), c.navigationController?.topViewController === c,
              let search = c.search, let index = search.scopeTabs.firstIndex(of: tab.id), let control = search.scopeControl, control.window != nil else { return nil }
        let views = control.segmentViews
        guard views.count == search.scopeTabs.count else { return nil }
        return (control, views[index].convert(views[index].bounds, to: control))
        #else
        return nil
        #endif
    }
}
#endif

#if os(iOS)
import UIKit

extension NavigationHost {
    /// D1, D2: the tablist the search bar takes as its scope bar, hidden
    /// and covered whole as the header is; the one it took before shown
    /// again (its cover goes with the next `reportCovers`).
    func scope(_ c: RouteController, shape: HeaderShape?) {
        let next = shape.flatMap { scopeFilter(after: $0, in: c) }
        guard next !== c.scoped else { return }
        c.scoped?.isHidden = false
        c.scoped = next
        next?.isHidden = true
    }

    /// D1: the next child the author shows in flow after a header the bar
    /// replaces whole whose search it projects, when it is a filter: two to
    /// five text tabs, one selected, none disabled, naming no panel, and no
    /// look of the author's on it or them.
    private func scopeFilter(after shape: HeaderShape, in c: RouteController) -> NodeView? {
        guard UIDevice.current.userInterfaceIdiom == .phone, shape.authored.isEmpty, shape.search != nil else { return nil }
        let kids = c.node.container.subviews.compactMap { $0 as? NodeView }
        // What the host hides (this filter, a segmented control's tabs) is still the author's.
        let shown = { (v: NodeView) in v.style["display"]?.string != "none" && Self.inFlow(v) }
        guard let at = kids.firstIndex(where: { $0 === shape.header }),
              let list = kids[(at + 1)...].first(where: shown),
              list.props["accessibilityRole"] == "tablist", list.props["accessibilityOrientation"] != "vertical", !Self.writesLook(list) else { return nil }
        // One that takes no input stays the page's, where it shows so (SegmentHost's `available`).
        var up: UIView? = list
        while let v = up {
            if (v as? NodeView)?.props["inert"] == "true" { return nil }
            up = presenter.eventParent(of: v)
        }
        if list.cssVisibilityHidden || list.alpha < 1 { return nil }
        let members = list.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" }
        guard (2...5).contains(members.count), members.filter({ $0.props["accessibilitySelected"] == "true" }).count == 1,
              members.allSatisfy({ tab in
                  guard tab.isButton, tab.props["accessibilityRole"] == "tab", tab.handlers.contains("press"), !tab.handlers.contains("contextmenu"), !tab.disabled, !tab.ariaUnavailable,
                        tab.props["accessibilityControls"] == nil, case .title(let words)? = tab.segmentFace, tab.accessibleName == words else { return false }
                  return tab.alpha == 1 && !Self.writesLook(tab) && !tab.container.subviews.contains { ($0 as? NodeView).map(Self.writesLook) == true }
              }) else { return nil }
        return list
    }

    /// A colour, background, border, shadow, font or `appearance` its author
    /// wrote (opacity is the view's `alpha`, read where this is).
    private static func writesLook(_ n: NodeView) -> Bool {
        guard case .array(let rows)? = n.style["written"] else { return false }
        let looks = ["background", "text_color", "tint_color", "accent_color", "font", "border_width", "border_style", "border_color", "box_shadow", "appearance"]
        return rows.contains { row in row.string.map { key in looks.contains { key.hasPrefix($0) } } == true }
    }
}

extension HeaderSearch {
    /// D2–D4: the scope bar is `list`'s tabs, or none.
    func scope(_ list: NodeView?) {
        let bar = controller.searchBar
        let tabs = list.map { $0.container.subviews.compactMap { $0 as? NodeView }.filter { $0.style["display"]?.string != "none" } } ?? []
        guard !tabs.isEmpty else {
            guard !scopeTabs.isEmpty else { return }
            scopeList = nil
            scopeTabs = []
            bar.scopeButtonTitles = nil
            bar.showsScopeBar = false
            return
        }
        scopeList = list
        scopeTabs = tabs.map(\.id)
        let titles = tabs.map { tab in if case .title(let words)? = tab.segmentFace { return words }; return tab.accessibleName }
        if bar.scopeButtonTitles != titles { bar.scopeButtonTitles = titles; scopeSettled = nil }
        showScopes()
        let selected = tabs.firstIndex { $0.props["accessibilitySelected"] == "true" } ?? 0
        // A finger's choice not yet reported stays (D3); the settled index is
        // written first, so a delegate call setting it raises presses nothing.
        let pending = scopeControl?.isTracking == true || scopeSettled.map({ $0 != bar.selectedScopeButtonIndex }) == true
        if !pending, bar.selectedScopeButtonIndex != selected {
            scopeSettled = selected
            bar.selectedScopeButtonIndex = selected
        }
        if bar.selectedScopeButtonIndex == selected { scopeSettled = selected }
        scopeIDs = tabs.map { $0.props["testId"] }
        labelScopes()
    }

    /// UIKit resets these as it presents and dismisses the search.
    func showScopes() {
        guard !scopeTabs.isEmpty else { return }
        if !controller.searchBar.showsScopeBar { controller.searchBar.showsScopeBar = true }
        if controller.scopeBarActivation != .manual { controller.scopeBarActivation = .manual }
    }

    /// Its segments carry their tabs' test ids, on the segment views UIKit
    /// makes at layout, as `LabelledSegmentedControl` writes them: again
    /// as the route lays out (`RouteController.viewDidLayoutSubviews`).
    func labelScopes() {
        guard let control = scopeControl else { return }
        control.layoutIfNeeded()
        let views = control.segmentViews
        guard views.count == scopeIDs.count else { return }
        for (view, id) in zip(views, scopeIDs) where view.accessibilityIdentifier != id { view.accessibilityIdentifier = id }
    }

    /// The scope bar's segmented control: in the search bar where the field
    /// is stacked under the title, in the navigation bar where the field
    /// goes to the toolbar (an iPhone with no tab bar, iOS 26), while its
    /// route is the top one. It is UIKit's control whose segments are the
    /// scope titles: not Exact's own, nor in the item's title view.
    var scopeControl: UISegmentedControl? {
        guard let titles = controller.searchBar.scopeButtonTitles, !scopeTabs.isEmpty else { return nil }
        let title = item?.titleView
        func find(_ v: UIView) -> UISegmentedControl? {
            if v === title || v is LabelledSegmentedControl { return nil }
            if let s = v as? UISegmentedControl, s.numberOfSegments == titles.count, titles.indices.allSatisfy({ s.titleForSegment(at: $0) == titles[$0] }) { return s }
            for child in v.subviews { if let s = find(child) { return s } }
            return nil
        }
        if let s = find(controller.searchBar) { return s }
        guard let item, item.searchController === controller,
              let nav = host?.controllers.values.first(where: { $0.navigationItem === item })?.navigationController, nav.topViewController?.navigationItem === item else { return nil }
        return find(nav.navigationBar)
    }

    /// D3: a person's choice presses that segment's tab.
    func searchBar(_ bar: UISearchBar, selectedScopeButtonIndexDidChange index: Int) {
        guard index != scopeSettled, scopeTabs.indices.contains(index), let presenter = host?.presenter else { return }
        scopeSettled = index
        if let tab = presenter.views[scopeTabs[index]], !tab.disabled, !tab.ariaUnavailable { presenter.press(tab.id) }
        // The press's batch may not reach the bar (one that changes only list rows): its answer is read here.
        if let list = scopeList, presenter.views[list.id] === list { scope(list) }
    }
    func didPresentSearchController(_ search: UISearchController) { showScopes() }
}
#endif
