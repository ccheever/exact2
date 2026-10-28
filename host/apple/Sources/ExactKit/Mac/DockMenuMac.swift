// @ref LLP 1133.025 D5 — the Dock menu: each `-exact-menu` button whose ids
// include `dock` is also an item of the app's Dock menu, which AppKit asks
// the app delegate for each time it opens (`applicationDockMenu(_:)`). The
// item is the button as a menu bar item presents it: its label, `disabled`
// and `aria-checked`; choosing it presses the button. AppKit adds its own
// items below.
#if os(macOS)
import AppKit

extension ExactSession {
    /// This session's Dock menu items in declaration order, or `nil` for none.
    public var dockMenu: NSMenu? {
        let rows = presenter.dockButtons()
        guard !rows.isEmpty else { return nil }
        let menu = NSMenu()
        menu.autoenablesItems = false
        for node in rows {
            let item = AccessibleMenuItem(title: presenter.menus.title(of: node), action: #selector(DockMenuTarget.choose(_:)), keyEquivalent: "")
            item.owner = node
            item.target = DockMenuTarget.shared
            item.representedObject = DockMenuTarget.Choice(presenter, node)
            item.isEnabled = !node.disabled && !node.authoredInert
            item.state = node.props["accessibilityChecked"] == "true" ? .on : .off
            item.image = presenter.menus.image(of: node)
            menu.addItem(item)
        }
        return menu
    }

    /// What the agent's `state` says of the Dock menu AppKit would show.
    public var observedDockMenu: [[String: Any]] {
        presenter.dockButtons().map { ["id": Int($0.id), "title": presenter.menus.title(of: $0), "disabled": $0.disabled] }
    }
}

extension Presenter {
    /// The mounted buttons with `dock` among their `-exact-menu` ids, not
    /// hidden by the page.
    func dockButtons() -> [NodeView] {
        chrome.ids("menuPlacement").sorted().compactMap { views[$0] }.filter { node in
            node.isButton && node.props["menuPlacement"]?.split(whereSeparator: \.isWhitespace).contains("dock") == true
                && !menus.hidden(node)
        }
    }
}

/// The Dock menu's target: an item chosen presses its button, once AppKit
/// has closed the menu, if it is still the same live button.
final class DockMenuTarget: NSObject {
    static let shared = DockMenuTarget()
    final class Choice: NSObject {
        weak var presenter: Presenter?
        weak var node: NodeView?
        init(_ presenter: Presenter, _ node: NodeView) { self.presenter = presenter; self.node = node }
    }
    @objc func choose(_ sender: NSMenuItem) {
        guard let choice = sender.representedObject as? Choice else { return }
        DispatchQueue.main.async {
            guard let presenter = choice.presenter, let node = choice.node, presenter.views[node.id] === node,
                  !node.disabled, !node.authoredInert else { return }
            presenter.press(node.id, fromNativeMenu: true, outsideModal: true)
        }
    }
}
#endif
