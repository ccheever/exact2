// @ref LLP 1069.001 D5 — the controls that carry a value, projected onto
// AppKit as the checkbox is: `select` is an `NSPopUpButton`, its items read
// from the kernel. Contract owns the value: the control moves at once,
// reports HTML's `input` then `change`, and shows the bound value when it
// changes, keeping the person's choice until then (D4, amended 2026-10-04).
#if os(macOS)
import AppKit

extension ControlHost {
    func makeValueControl(_ kind: String) -> NSControl {
        if kind == "range" { return makeRange() }
        if ControlKinds.dates.contains(kind) { return makeDate(kind) }
        let popup = NSPopUpButton(frame: .zero, pullsDown: false)
        // Each item's own `disabled`, never AppKit's validation.
        popup.autoenablesItems = false
        return popup
    }

    func configureValue(_ control: NSControl, _ owner: NodeView, accent: NSColor?) {
        if let slider = control as? NSSlider { configureRange(slider, owner); return }
        if let picker = control as? NSDatePicker { configureDate(picker, owner, accent: accent); return }
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
        // A slider has no natural width; Chrome's range is 129 wide.
        if control is NSSlider { return CGSize(width: 129, height: ceil(natural.height)) }
        guard control is NSPopUpButton, let cell = control.cell else { return natural }
        return CGSize(width: ceil(max(natural.width, cell.cellSize.width)), height: natural.height)
    }

    @objc func valueChanged(_ sender: NSControl) {
        if let slider = sender as? NSSlider { rangeChanged(slider); return }
        if let picker = sender as? NSDatePicker { dateChanged(picker); return }
        guard let popup = sender as? NSPopUpButton, let value = popup.selectedItem?.representedObject as? String else { return }
        chose(UInt32(sender.tag), value)
    }

    /// A choice from the menu: `input` then `change`, then the committed
    /// value shown again, which an action that refused leaves unchanged.
    func chose(_ id: UInt32, _ value: String) {
        guard presenter.views[id] != nil else { return }
        let before = presenter.selectOptions?(id)
        presenter.controlValue(id, value, input: true, change: true)
        // The bound value is shown when it changes, as the web build's select:
        // an action that wrote none leaves the person's choice showing (LLP
        // 1069.001 D4, amended 2026-10-04).
        guard presenter.selectOptions?(id) != before else { return }
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
        if let slider = controls[node.id] as? NSSlider {
            guard slider.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
            return typeRange(slider, node, value)
        }
        if let picker = controls[node.id] as? NSDatePicker {
            guard picker.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
            return typeDate(picker, node, value)
        }
        if kinds[node.id] == "radio" { return typeRadio(node, value) } // x2apps survey #2
        // A checkbox (or `switch`) takes `true` or `false`, and is clicked when that differs, as on the web.
        if kinds[node.id] == "checkbox" || kinds[node.id] == "switch", let control = controls[node.id] {
            guard value == "true" || value == "false" else { return ["error": "checkbox \(node.id) takes true or false, not \"\(value)\""] }
            if isOn(control) != (value == "true"), activate(node) != true { return ["error": "control #\(node.id) is disabled, inert or not shown"] }
            return ["typed": Int(node.id), "checked": isOn(control), "delivery": "host-activation", "native": "control"]
        }
        guard let control = controls[node.id], let popup = control as? NSPopUpButton else { return nil }
        guard control.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
        let choice = (presenter.selectOptions?(node.id) ?? SelectMenu()).choose(value, id: node.id)
        guard let chosen = choice.value else { return ["error": choice.refusal ?? "select \(node.id) refused \"\(value)\""] }
        popup.menu?.cancelTracking()
        if let index = popup.menu?.items.firstIndex(where: { $0.representedObject as? String == chosen }) { popup.selectItem(at: index) }
        chose(node.id, chosen)
        return ["typed": Int(node.id), "value": popup.selectedItem?.representedObject as? String ?? "", "delivery": "host-activation", "native": "control"]
    }

    func valueObservation(_ control: NSControl) -> [String: Any]? {
        if let slider = control as? NSSlider {
            return ["view": "NSSlider", "value": slider.doubleValue, "min": slider.minValue, "max": slider.maxValue]
        }
        if let picker = control as? NSDatePicker {
            let empty = (picker as? DateField)?.empty == true
            return ["view": "NSDatePicker", "value": empty ? "" : DateValue.format(kinds[UInt32(picker.tag)] ?? "date", picker.dateValue)]
        }
        guard let popup = control as? NSPopUpButton else { return nil }
        let menu = menus[UInt32(popup.tag)]
        return ["view": "NSPopUpButton", "value": menu?.chosenValue as Any, "title": popup.titleOfSelectedItem as Any,
                "options": menu?.options.map(\.label) ?? []]
    }
}
#endif
