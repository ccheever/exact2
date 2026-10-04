// @ref LLP 1069.001 D5 — the controls that carry a value, projected onto
// UIKit as the checkbox is: `select` is iOS's pop-up button (a `UIButton`
// whose menu chooses, `changesSelectionAsPrimaryAction`), its options read
// from the kernel. Contract owns the value: the control moves at once,
// reports HTML's `input` then `change`, and shows the bound value when it
// changes, keeping the person's choice until then (D4, amended 2026-10-04).
#if os(iOS) || os(tvOS)
import UIKit

extension ControlHost {
    func makeValueControl(_ kind: String, _ id: UInt32) -> UIControl {
        #if os(tvOS)
        // tvOS has no UISlider or UIDatePicker; the pop-up button stands in.
        #else
        if kind == "range" { return makeRange() }
        if ControlKinds.dates.contains(kind) { return makeDate(kind) }
        #endif
        var config = UIButton.Configuration.plain()
        config.indicator = .popup
        config.contentInsets = .zero
        let button = UIButton(configuration: config)
        button.showsMenuAsPrimaryAction = true
        button.changesSelectionAsPrimaryAction = true
        return button
    }

    func configureValue(_ control: UIControl, _ owner: NodeView, accent: UIColor?) {
        #if !os(tvOS)
        if let slider = control as? UISlider { configureRange(slider, owner, accent: accent); return }
        if let picker = control as? UIDatePicker { configureDate(picker, owner, accent: accent); return }
        #endif
        guard let button = control as? UIButton else { return }
        assign(button, \.tintColor, accent)
        let menu = presenter.selectOptions?(owner.id) ?? SelectMenu()
        guard menus[owner.id] != menu else { return }
        menus[owner.id] = menu
        picked.removeValue(forKey: owner.id)
        install(button, menu, id: owner.id, showing: menu.chosen)
    }

    /// The menu, with the option at `showing` the one shown.
    private func install(_ button: UIButton, _ menu: SelectMenu, id: UInt32, showing: Int?) {
        button.menu = UIMenu(children: menu.options.enumerated().map { i, option in
            UIAction(title: option.label, attributes: option.disabled ? .disabled : [], state: i == showing ? .on : .off) { [weak self] _ in
                self?.chose(id, option.value)
            }
        })
    }

    /// HTML sizes a select to its widest option, whichever is shown (D3).
    func naturalSize(_ control: UIControl, _ owner: NodeView) -> CGSize {
        #if !os(tvOS)
        // A slider has no natural width; Chrome's range is 129 wide.
        if control is UISlider { return CGSize(width: 129, height: ceil(control.intrinsicContentSize.height)) }
        // A compact date picker sizes by Auto Layout, not before it lays out.
        if control is UIDatePicker {
            let s = control.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
            return CGSize(width: ceil(s.width), height: ceil(s.height))
        }
        #endif
        if control is NativeButtonIOS {
            let s = control.intrinsicContentSize
            return CGSize(width: ceil(s.width), height: ceil(s.height))
        }
        guard let button = control as? UIButton, let config = button.configuration else { return control.intrinsicContentSize }
        let probe = UIButton(configuration: config)
        var size = CGSize.zero
        for option in menus[owner.id]?.options ?? [] {
            probe.configuration?.title = option.label
            let s = probe.intrinsicContentSize
            size = CGSize(width: max(size.width, s.width), height: max(size.height, s.height))
        }
        return size == .zero ? button.intrinsicContentSize : CGSize(width: ceil(size.width), height: ceil(size.height))
    }

    /// A choice from the menu: `input` then `change`. The bound value is
    /// shown when it changes, as the web build's select: an action that
    /// wrote none leaves the person's choice showing (LLP 1069.001 D4,
    /// amended 2026-10-04).
    func chose(_ id: UInt32, _ value: String) {
        guard presenter.views[id] != nil else { return }
        let before = presenter.selectOptions?(id)
        presenter.controlValue(id, value, input: true, change: true)
        if let before, presenter.selectOptions?(id) == before, let button = controls[id] as? UIButton {
            picked[id] = value
            install(button, before, id: id, showing: before.options.firstIndex { $0.value == value })
            return
        }
        picked.removeValue(forKey: id)
        menus.removeValue(forKey: id)
        if let owner = presenter.views[id], let control = controls[id] {
            configureValue(control, owner, accent: owner.channels("accent_color").map { TextEngine.color($0) })
        }
    }

    /// The agent's `tap` on a select opens nothing here: UIKit presents a
    /// button's menu only under a finger (`performPrimaryAction` opens one the
    /// carrier can neither see nor close), so the reply says so and `type`
    /// chooses (LLP 1069.001 D9, as a held contact is refused in LLP 1035.003).
    func openValue(_ control: UIControl) -> Bool { control.window != nil }
    func unopened(_ node: NodeView) -> [String: Any]? {
        #if !os(tvOS)
        if controls[node.id] is UIDatePicker {
            return ["tapped": Int(node.id), "delivery": "unsupported", "native": "control",
                    "reason": "the iOS carrier opens no picker (UIKit presents one under a finger); `type <id> <value>` sets it"]
        }
        if controls[node.id] is UISlider {
            return ["tapped": Int(node.id), "delivery": "unsupported", "native": "control",
                    "reason": "the iOS carrier drags no thumb (UIKit moves one under a finger); `type <id> <value>` sets it"]
        }
        #endif
        guard controls[node.id] is UIButton, !(controls[node.id] is NativeButtonIOS) else { return nil }
        return ["tapped": Int(node.id), "delivery": "unsupported", "native": "control",
                "reason": "the iOS carrier opens no menu (UIKit presents one under a finger); `type <id> <value>` chooses"]
    }

    /// The agent's `type <id> <value>` (D9): the choice a menu would make.
    func type(_ node: NodeView, _ value: String) -> [String: Any]? {
        #if !os(tvOS)
        if let slider = controls[node.id] as? UISlider {
            guard slider.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
            return typeRange(slider, node, value)
        }
        if let picker = controls[node.id] as? UIDatePicker {
            guard picker.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
            return typeDate(picker, node, value)
        }
        #endif
        // A checkbox (or `switch`) takes `true` or `false`, and is toggled when that differs, as on the web.
        if kinds[node.id] == "checkbox" || kinds[node.id] == "switch", let control = controls[node.id], !(control is NativeButtonIOS) {
            guard value == "true" || value == "false" else { return ["error": "checkbox \(node.id) takes true or false, not \"\(value)\""] }
            if checked(control) != (value == "true"), activate(node) != true { return ["error": "control #\(node.id) is disabled, inert or not shown"] }
            return ["typed": Int(node.id), "checked": checked(control), "delivery": "host-activation", "native": "control"]
        }
        guard let control = controls[node.id], control is UIButton, !(control is NativeButtonIOS) else { return nil }
        guard control.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
        let choice = (presenter.selectOptions?(node.id) ?? SelectMenu()).choose(value, id: node.id)
        guard let chosen = choice.value else { return ["error": choice.refusal ?? "select \(node.id) refused \"\(value)\""] }
        chose(node.id, chosen)
        return ["typed": Int(node.id), "value": picked[node.id] ?? menus[node.id]?.chosenValue ?? "", "delivery": "host-activation", "native": "control"]
    }

    /// Whether a checkbox or a switch is on.
    func checked(_ control: UIControl) -> Bool {
        #if os(tvOS)
        return (control as? ExactCheckbox)?.isOn ?? false
        #else
        return (control as? UISwitch)?.isOn ?? (control as? ExactCheckbox)?.isOn ?? false
        #endif
    }

    func valueObservation(_ control: UIControl) -> [String: Any]? {
        #if !os(tvOS)
        if let slider = control as? UISlider {
            return ["view": "UISlider", "value": Double(slider.value), "min": Double(slider.minimumValue), "max": Double(slider.maximumValue)]
        }
        if let picker = control as? UIDatePicker {
            return ["view": "UIDatePicker(compact)", "value": DateValue.format(kinds[UInt32(picker.tag)] ?? "date", picker.date)]
        }
        #endif
        guard let button = control as? UIButton, !(button is NativeButtonIOS) else { return nil }
        let menu = menus[UInt32(button.tag)]
        return ["view": "UIButton(pop-up)", "value": menu?.chosenValue as Any, "title": button.currentTitle as Any,
                "options": menu?.options.map(\.label) ?? []]
    }
}
#endif
