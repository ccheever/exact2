// @ref LLP 1075.003 §3.7 — a navigation root's tabs, as the web finds them
// (`host/web/navigation.js` `panelsOf`): the tabpanels its own tablist's
// tabs name with `aria-controls`, in tab order. Each panel holds one tab's
// stack — its route rows — and every tab's stack stays mounted. A tablist
// inside a route is that route's, never the root's. Shared by the AppKit
// presenter (hidden and inert, LLP 1038 D6's rule) and the UIKit one (a
// tab bar controller, one navigation controller a tab).
import Foundation

struct NavigationTabs {
    let tablist: NodeView
    /// The tabs that name a panel, and those panels, in tab order.
    let tabs: [NodeView]
    let panels: [NodeView]

    /// A panel's stack: its children that carry a `navigationKey`.
    static func routes(in node: NodeView) -> [NodeView] {
        node.container.subviews.compactMap { $0 as? NodeView }.filter { $0.props["navigationKey"] != nil }
    }

    /// The root's tabs, or nil when it has none (one stack).
    static func of(_ root: NodeView, _ presenter: Presenter) -> NavigationTabs? {
        for list in presenter.carrying("role:tablist") where list.isDescendant(of: root) && owner(of: list) === root {
            var tabs: [NodeView] = [], panels: [NodeView] = []
            for case let tab as NodeView in list.container.subviews where tab.props["accessibilityRole"] == "tab" {
                guard let name = tab.props["accessibilityControls"],
                      let panel = presenter.carrying("id").first(where: {
                          $0.props["id"] == name && $0.props["accessibilityRole"] == "tabpanel" && $0.isDescendant(of: root)
                      }) else { continue }
                tabs.append(tab)
                panels.append(panel)
            }
            if !panels.isEmpty { return NavigationTabs(tablist: list, tabs: tabs, panels: panels) }
        }
        return nil
    }

    /// The nearest ancestor of a node that is a route or a navigation root.
    private static func owner(of node: NodeView) -> NodeView? {
        var at = node.superview
        while let view = at {
            if let n = view as? NodeView, n.props["navigationKey"] != nil { return n }
            at = view.superview
        }
        return nil
    }

    /// Which stack holds the route the root names, and each stack's routes.
    func stacks(selecting key: String) -> (at: Int?, stacks: [[NodeView]]) {
        let stacks = panels.map(NavigationTabs.routes(in:))
        return (stacks.firstIndex { $0.contains { $0.props["navigationKey"] == key } }, stacks)
    }
}
