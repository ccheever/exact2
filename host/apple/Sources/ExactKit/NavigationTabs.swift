// @ref LLP 1075.003 §3.7 — a navigation root's tabs, as the web finds them
// (`host/web/navigation.js` `panelsOf`): the tabpanels its own tablist's
// tabs name with `aria-controls`, in tab order. Each panel holds one tab's
// stack — its route rows — and every tab's stack stays mounted. A tablist or
// a panel inside a route is that route's, never the root's. Shared by the AppKit
// presenter (hidden and inert, LLP 1038 D6's rule) and the UIKit one (a
// tab bar controller, one navigation controller a tab).
import Foundation

/// A navigation root's tabs as the kernel finds them (`exact_root_tabs`;
/// `Kernel::root_tabs`, LLP 1115.001 D11): the rule every host with a tab
/// bar reads, over the kernel's own tree in tree order. The Mac's sidebar
/// reads it (LLP 1115.002 D3); UIKit's tab bar still reads `NavigationTabs`
/// below, over its views.
struct RootTabs: Equatable {
    let tablist: UInt32, tabs: [UInt32], panels: [UInt32]
    /// The app left the bar's placement unsaid, by the web's rule for its
    /// own sidebar (LLP 1116.004 D1.1; `host/web/navigation.js` `unsaidBar`):
    /// read from the kernel's layout rows, which do not cross to the views.
    let unsaid: Bool

    init(tablist: UInt32, tabs: [UInt32], panels: [UInt32], unsaid: Bool) {
        (self.tablist, self.tabs, self.panels, self.unsaid) = (tablist, tabs, panels, unsaid)
    }

    init?(json: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: json) as? [String: Any],
              let tablist = o["tablist"] as? Int, let tabs = o["tabs"] as? [Int], let panels = o["panels"] as? [Int] else { return nil }
        self.init(tablist: UInt32(tablist), tabs: tabs.map { UInt32($0) }, panels: panels.map { UInt32($0) }, unsaid: o["unsaid"] as? Bool ?? false)
    }
}

/// A stack that shows its root route as a list beside its detail, as the
/// runner finds it (`exact_stack_split`; `Runner::stack_split`, LLP 1115.002
/// D4): the rule every host with a split reads, over the kernel's own tree.
/// The Mac's split view reads it (SplitMac.swift).
struct StackSplit: Equatable {
    /// The list whose row pushes; nil where only `navigationSplit` asked.
    let list: UInt32?
    /// The list column's width as `navigationSplit` names it.
    let width: Double?
    /// Each route above the root that is a row's detail, by the route's id,
    /// with that row's press.
    let rows: [UInt32: UInt32]

    init(list: UInt32?, width: Double?, rows: [UInt32: UInt32]) { (self.list, self.width, self.rows) = (list, width, rows) }

    init?(json: Data) {
        guard let o = try? JSONSerialization.jsonObject(with: json) as? [String: Any] else { return nil }
        let pairs = (o["rows"] as? [[Int]] ?? []).filter { $0.count == 2 }.map { (UInt32($0[0]), UInt32($0[1])) }
        self.init(list: (o["list"] as? Int).map { UInt32($0) }, width: (o["width"] as? NSNumber)?.doubleValue,
                  rows: Dictionary(pairs, uniquingKeysWith: { first, _ in first }))
    }
}

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
                          $0.props["id"] == name && $0.props["accessibilityRole"] == "tabpanel" && owner(of: $0) === root
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
