// The native menu arm (exact2 LLP 1021 D3), macOS: a popover whose rows
// are buttons presents as an NSMenu popped below its invoker, built from
// the rows' data (text → title, aria-checked → the checkmark, disabled →
// dimmed; a row without an action becomes the separator), and a
// selection dispatches the row's press by view id into the runner — the
// same journal entry a painted click makes. The invoker's own press has
// already gone to the runner when the menu is built (the pop is deferred
// one turn), so a menu that refreshes its rows on that press shows the
// refreshed rows. The popover subtree itself never paints here.

#if os(macOS)
import AppKit

final class MenuHost: NSObject {
    private weak var presenter: Presenter?
    private var invokers: [UInt32: String] = [:]
    private var popovers: [String: UInt32] = [:]

    init(presenter: Presenter) { self.presenter = presenter }

    /// After a batch: hide every popover, remember who invokes what.
    func sync() {
        guard let presenter else { return }
        // An agent run gets the painted subtree, not the platform's menu
        // (LLP 1021 D4): the popover stays visible and the rows are tapped
        // by view id, so no NSMenu tracking loop ever blocks a driver.
        guard !ExactEnv.agentMode else { return }
        popovers.removeAll()
        invokers.removeAll()
        for v in presenter.carrying("popover") {
            v.isHidden = true
            if let name = v.props["id"] { popovers[name] = v.id }
        }
        for v in presenter.carrying("popovertarget") {
            // A row that only hides its popover (a menu item closing
            // itself, the spec's way) is not an invoker.
            if let target = v.props["popovertarget"], popovers[target] != nil,
               v.props["popovertargetaction"] != "hide" {
                invokers[v.id] = target
            }
        }
    }

    /// Every press passes through here; an invoker's also drops its menu,
    /// one turn later so the press's own batch — the switcher's refresh —
    /// is in the items.
    func pressed(_ id: UInt32) {
        guard let target = invokers[id], let popId = popovers[target] else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let presenter = self.presenter,
                  let invoker = presenter.views[id], let pop = presenter.views[popId]
            else { return }
            self.menu(of: pop).popUp(
                positioning: nil,
                at: NSPoint(x: 0, y: invoker.bounds.height + 2),
                in: invoker
            )
        }
    }

    /// The menu grammar, extracted (LLP 1021 D3).
    func menu(of pop: NodeView) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        var boundary = false
        for case let row as NodeView in pop.container.subviews {
            guard row.pressable else {
                boundary = true
                continue
            }
            if boundary, !menu.items.isEmpty { menu.addItem(.separator()) }
            boundary = false
            let item = NSMenuItem(title: title(of: row), action: #selector(pick(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = NSNumber(value: row.id)
            item.state = row.props["accessibilityChecked"] == "true" ? .on : .off
            item.isEnabled = row.props["disabled"] != "true"
            menu.addItem(item)
        }
        return menu
    }

    @objc private func pick(_ sender: NSMenuItem) {
        if let id = (sender.representedObject as? NSNumber)?.uint32Value {
            // The source subtree is hidden because NSMenu presents it.
            presenter?.press(id, fromNativeMenu: true)
        }
    }

    private func title(of v: NodeView) -> String {
        if v.kind == "text" { return v.paragraphSpec().runs.map(\.text).joined() }
        return v.container.subviews
            .compactMap { ($0 as? NodeView).map(title(of:)) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}
#endif
