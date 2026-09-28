// @ref LLP 1069.001 D5 — the controls that carry a value, projected onto
// AppKit as the checkbox is: `select` is an `NSPopUpButton`, its items read
// from the kernel. Contract owns the value: the control moves at once,
// reports HTML's `input` then `change`, and shows the committed value after
// the action (D4).
#if os(macOS)
import AppKit

extension ControlHost {
    func makeValueControl(_ kind: String) -> NSControl {
        let popup = NSPopUpButton(frame: .zero, pullsDown: false)
        // Each item's own `disabled`, never AppKit's validation.
        popup.autoenablesItems = false
        return popup
    }

    func configureValue(_ control: NSControl, _ owner: NodeView, accent: NSColor?) {
        guard let popup = control as? NSPopUpButton else { return }
        let menu = presenter.selectOptions?(owner.id) ?? SelectMenu()
        guard menus[owner.id] != menu else { return }
        menus[owner.id] = menu
        // Items by hand: `addItems(withTitles:)` merges equal titles.
        popup.removeAllItems()
        for option in menu.options {
            let item = NSMenuItem(title: option.label, action: nil, keyEquivalent: "")
            item.representedObject = option.value
            item.isEnabled = !option.disabled
            popup.menu?.addItem(item)
        }
        popup.selectItem(at: menu.chosen ?? -1)
    }

    /// The control's size: a pop-up's is its widest item's, as HTML sizes a
    /// select (D3), which its cell measures and its intrinsic size may not.
    func naturalSize(_ control: NSControl) -> CGSize {
        let natural = control.intrinsicContentSize
        guard control is NSPopUpButton, let cell = control.cell else { return natural }
        return CGSize(width: ceil(max(natural.width, cell.cellSize.width)), height: natural.height)
    }

    @objc func valueChanged(_ sender: NSControl) {
        guard let popup = sender as? NSPopUpButton, let value = popup.selectedItem?.representedObject as? String else { return }
        chose(UInt32(sender.tag), value)
    }

    /// A choice from the menu: `input` then `change`, then the committed
    /// value shown again, which an action that refused leaves unchanged.
    func chose(_ id: UInt32, _ value: String) {
        guard presenter.views[id] != nil else { return }
        presenter.controlValue(id, value, input: true, change: true)
        menus.removeValue(forKey: id)
        if let owner = presenter.views[id], let control = controls[id] {
            configureValue(control, owner, accent: nil)
        }
    }

    /// The agent's `tap` on a select: its menu opens, as a click opens it.
    /// AppKit tracks an open menu modally; the agent's requests still run
    /// (common modes), and a `type` closes it with the choice.
    func openValue(_ control: NSControl) -> Bool {
        guard let popup = control as? NSPopUpButton else { return false }
        DispatchQueue.main.async { popup.performClick(nil) }
        return true
    }

    /// The agent's `type <id> <value>` (D9): the choice a menu would make.
    func type(_ node: NodeView, _ value: String) -> [String: Any]? {
        guard let control = controls[node.id], let popup = control as? NSPopUpButton else { return nil }
        guard control.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
        if let refusal = (presenter.selectOptions?(node.id) ?? SelectMenu()).refusal(value, id: node.id) { return ["error": refusal] }
        popup.menu?.cancelTracking()
        if let index = popup.menu?.items.firstIndex(where: { $0.representedObject as? String == value }) { popup.selectItem(at: index) }
        chose(node.id, value)
        return ["typed": Int(node.id), "value": menus[node.id]?.chosenValue ?? "", "delivery": "host-activation", "native": "control"]
    }

    func valueObservation(_ control: NSControl) -> [String: Any]? {
        guard let popup = control as? NSPopUpButton else { return nil }
        let menu = menus[UInt32(popup.tag)]
        return ["view": "NSPopUpButton", "value": menu?.chosenValue as Any, "title": popup.titleOfSelectedItem as Any,
                "options": menu?.options.map(\.label) ?? []]
    }
}
#endif
