// @ref LLP 1069.001 D5 — `input type="checkbox"` is AppKit's checkbox
// (`NSButton`), and `switch` an `NSSwitch`, projected into the node's box as
// a tablist's segmented control is. Contract owns the value: the control
// flips at once, reports, and shows the committed `checked` after the action
// (D4). `appearance: none` draws nothing native.
#if os(macOS)
import AppKit

final class ControlHost: NSObject {
    unowned let presenter: Presenter
    private var controls: [UInt32: NSControl] = [:]
    /// The size last reported per control, so each is published once.
    private var reported: [UInt32: CGSize] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter }

    private func isSwitch(_ node: NodeView) -> Bool { node.props["accessibilityRole"] == "switch" }

    private func isOn(_ control: NSControl) -> Bool {
        ((control as? NSSwitch)?.state ?? (control as? NSButton)?.state) == .on
    }

    private func setOn(_ control: NSControl, _ on: Bool) {
        let state: NSControl.StateValue = on ? .on : .off
        if let s = control as? NSSwitch, s.state != state { s.state = state }
        if let b = control as? NSButton, b.state != state { b.state = state }
    }

    /// The control the node shows, made (or remade, when `switch` changes).
    private func control(for node: NodeView) -> NSControl {
        let wantSwitch = isSwitch(node)
        if let existing = controls[node.id], (existing is NSSwitch) == wantSwitch { return existing }
        controls.removeValue(forKey: node.id)?.removeFromSuperview()
        let made: NSControl
        if wantSwitch {
            made = NSSwitch()
        } else {
            let box = NSButton(checkboxWithTitle: "", target: nil, action: nil)
            box.imagePosition = .imageOnly
            made = box
        }
        made.tag = Int(node.id)
        made.target = self
        made.action = #selector(changed(_:))
        controls[node.id] = made
        return made
    }

    func sync() {
        let owners = presenter.carrying("type:checkbox").filter { $0.kind == "control" }
        let live = Set(owners.map(\.id))
        for id in Array(controls.keys) where !live.contains(id) {
            controls.removeValue(forKey: id)?.removeFromSuperview()
            reported.removeValue(forKey: id)
        }
        var sizes: [(UInt32, CGSize?)] = []
        for owner in owners {
            let control = control(for: owner)
            if owner.style["appearance"]?.string == "none" {
                // The author's box is the look; the node keeps its role.
                control.removeFromSuperview()
                continue
            }
            if control.superview !== owner { owner.addSubview(control) }
            if let on = owner.props["checked"].map({ $0 == "true" }) { setOn(control, on) }
            let accent = owner.channels("accent_color").map { TextEngine.color($0) }
            // NSSwitch takes the system accent; AppKit gives it no tint.
            (control as? NSButton)?.contentTintColor = accent
            control.isEnabled = !owner.disabled
            control.setAccessibilityLabel(owner.props["accessibilityLabel"])
            control.setAccessibilityIdentifier(owner.props["testId"])
            let natural = control.intrinsicContentSize
            let box = owner.contentBox()
            control.frame = CGRect(x: box.midX - natural.width / 2, y: box.midY - natural.height / 2,
                                   width: natural.width, height: natural.height)
            if reported[owner.id] != natural {
                reported[owner.id] = natural
                sizes.append((owner.id, natural))
            }
        }
        // Published outside the batch being applied, as images' are.
        if !sizes.isEmpty { DispatchQueue.main.async { [weak self] in self?.presenter.onIntrinsic?(sizes) } }
    }

    @objc private func changed(_ sender: NSControl) {
        let id = UInt32(sender.tag)
        guard presenter.views[id] != nil else { return }
        presenter.checked(id, isOn(sender))
        // The committed state is authoritative: an action that refused the
        // toggle snaps the control back (D4).
        if let committed = presenter.views[id]?.props["checked"].map({ $0 == "true" }) { setOn(sender, committed) }
    }

    /// The agent's `tap` (LLP 1069.001 D9): the control's own activation.
    func activate(_ node: NodeView) -> Bool? {
        guard let control = controls[node.id] else { return nil }
        guard control.window != nil, control.isEnabled, !node.inert, !control.isHiddenOrHasHiddenAncestor else { return false }
        if let b = control as? NSButton { b.performClick(nil) } else {
            setOn(control, !isOn(control))
            changed(control)
        }
        return true
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard let control = controls[node.id] else { return nil }
        return ["view": control is NSSwitch ? "NSSwitch" : "NSButton(checkbox)", "on": isOn(control),
                "size": [Agent.r2(control.frame.width), Agent.r2(control.frame.height)]]
    }

    func reset() {
        for control in controls.values { control.removeFromSuperview() }
        controls.removeAll()
        reported.removeAll()
    }
}
#endif
