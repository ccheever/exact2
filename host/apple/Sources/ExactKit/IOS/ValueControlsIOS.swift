// @ref LLP 1069.001 D5 — the controls that carry a value, projected onto
// UIKit as the checkbox is: `select` is iOS's pop-up button (a `UIButton`
// whose menu chooses, `changesSelectionAsPrimaryAction`), its options read
// from the kernel. Contract owns the value: the control moves at once,
// reports HTML's `input` then `change`, and shows the committed value after
// the action (D4).
#if os(iOS)
import UIKit

extension ControlHost {
    func makeValueControl(_ kind: String, _ id: UInt32) -> UIControl {
        var config = UIButton.Configuration.plain()
        config.indicator = .popup
        config.contentInsets = .zero
        let button = UIButton(configuration: config)
        button.showsMenuAsPrimaryAction = true
        button.changesSelectionAsPrimaryAction = true
        return button
    }

    func configureValue(_ control: UIControl, _ owner: NodeView, accent: UIColor?) {
        guard let button = control as? UIButton else { return }
        button.tintColor = accent
        let menu = presenter.selectOptions?(owner.id) ?? SelectMenu()
        guard menus[owner.id] != menu else { return }
        menus[owner.id] = menu
        let id = owner.id
        button.menu = UIMenu(children: menu.options.enumerated().map { i, option in
            UIAction(title: option.label, attributes: option.disabled ? .disabled : [], state: i == menu.chosen ? .on : .off) { [weak self] _ in
                self?.chose(id, option.value)
            }
        })
    }

    /// HTML sizes a select to its widest option, whichever is shown (D3).
    func naturalSize(_ control: UIControl, _ owner: NodeView) -> CGSize {
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

    /// A choice from the menu: `input` then `change`, then the committed
    /// value shown again, which an action that refused leaves unchanged.
    func chose(_ id: UInt32, _ value: String) {
        guard presenter.views[id] != nil else { return }
        presenter.controlValue(id, value, input: true, change: true)
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
        guard controls[node.id] is UIButton else { return nil }
        return ["tapped": Int(node.id), "delivery": "unsupported", "native": "control",
                "reason": "the iOS carrier opens no menu (UIKit presents one under a finger); `type <id> <value>` chooses"]
    }

    /// The agent's `type <id> <value>` (D9): the choice a menu would make.
    func type(_ node: NodeView, _ value: String) -> [String: Any]? {
        guard let control = controls[node.id], control is UIButton else { return nil }
        guard control.isEnabled, !node.inert else { return ["error": "control #\(node.id) is disabled or inert"] }
        if let refusal = (presenter.selectOptions?(node.id) ?? SelectMenu()).refusal(value, id: node.id) { return ["error": refusal] }
        chose(node.id, value)
        return ["typed": Int(node.id), "value": menus[node.id]?.chosenValue ?? "", "delivery": "host-activation", "native": "control"]
    }

    func valueObservation(_ control: UIControl) -> [String: Any]? {
        guard let button = control as? UIButton else { return nil }
        let menu = menus[UInt32(button.tag)]
        return ["view": "UIButton(pop-up)", "value": menu?.chosenValue as Any, "title": button.currentTitle as Any,
                "options": menu?.options.map(\.label) ?? []]
    }
}
#endif
