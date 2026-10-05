// @ref LLP 1069.001 D5 — `input type="checkbox"` is AppKit's checkbox
// (`NSButton`), and `switch` an `NSSwitch`, projected into the node's box as
// a tablist's segmented control is. Contract owns the value: the control
// flips at once, reports, and shows the committed `checked` after the action
// (D4). `appearance: none` draws nothing native.
#if os(macOS)
import AppKit

final class ControlHost: NSObject {
    unowned let presenter: Presenter
    var controls: [UInt32: NSControl] = [:]
    /// Which control each node shows (`ControlKinds`), to remake it when that changes.
    var kinds: [UInt32: String] = [:]
    /// The size last reported per control, so each is published once.
    private var reported: [UInt32: CGSize] = [:]
    /// A select's menu as last built, so a batch that leaves it alone does not rebuild it.
    var menus: [UInt32: SelectMenu] = [:]
    /// A range's last reported value while it moves, so each is sent once.
    var lastRange: [UInt32: String] = [:]
    /// A range's bound value last written into it: written again only when
    /// it changes (LLP 1069.001 D4, amended 2026-10-04).
    var appliedRange: [UInt32: String] = [:]
    /// A radio's group from the kernel (`exact_radio_group`; x2apps survey #2).
    var radioGroup: ((UInt32) -> RadioGroup)?

    init(_ presenter: Presenter) { self.presenter = presenter }

    func isOn(_ control: NSControl) -> Bool {
        ((control as? NSSwitch)?.state ?? (control as? NSButton)?.state) == .on
    }

    func setOn(_ control: NSControl, _ on: Bool) {
        let state: NSControl.StateValue = on ? .on : .off
        if let s = control as? NSSwitch, s.state != state { s.state = state }
        if let b = control as? NSButton, b.state != state { b.state = state }
    }

    /// The control the node shows, made (or remade, when its kind changes).
    private func control(for node: NodeView) -> NSControl {
        let kind = ControlKinds.kind(node.props)
        if let existing = controls[node.id], kinds[node.id] == kind { return existing }
        controls.removeValue(forKey: node.id)?.removeFromSuperview()
        menus.removeValue(forKey: node.id)
        appliedRange.removeValue(forKey: node.id)
        let made: NSControl
        switch kind {
        case "switch": made = NSSwitch()
        case "button": made = makeNativeButton(node) // LLP 1069.011
        case "checkbox":
            let box = NSButton(checkboxWithTitle: "", target: nil, action: nil)
            box.imagePosition = .imageOnly
            made = box
        case "radio": made = RadioButtonMac() // x2apps survey #2
        default: made = makeValueControl(kind)
        }
        made.tag = Int(node.id)
        made.target = self
        made.action = kind == "button" ? #selector(nativePressed(_:)) : kind == "radio" ? #selector(radioClicked(_:))
            : kind == "switch" || kind == "checkbox" ? #selector(changed(_:)) : #selector(valueChanged(_:))
        controls[node.id] = made
        kinds[node.id] = kind
        return made
    }

    func sync() {
        let owners = ControlKinds.indexed.flatMap { presenter.carrying($0) }.filter { $0.kind == "control" }
        let live = Set(owners.map(\.id))
        // A leaving control keeps drawing until its exit ends (LLP 1069.011 D9).
        let leaving = Set(presenter.leaving.values.flatMap { $0.members.map(\.id) })
        for id in Array(controls.keys) where !live.contains(id) && !leaving.contains(id) {
            controls.removeValue(forKey: id)?.removeFromSuperview()
            reported.removeValue(forKey: id)
            kinds.removeValue(forKey: id)
            menus.removeValue(forKey: id)
            lastRange.removeValue(forKey: id)
            appliedRange.removeValue(forKey: id)
        }
        var sizes: [(UInt32, CGSize?)] = []
        for owner in owners {
            let control = control(for: owner)
            if owner.style["appearance"]?.string == "none" {
                // The author's box is the look; the node keeps its role.
                control.removeFromSuperview()
                continue
            }
            let mount = owner.controlMount
            if control.superview !== mount { mount.addSubview(control) }
            let accent = owner.channels("accent_color").map { TextEngine.color($0) }
            if let b = control as? NativeButtonMac {
                configureNative(b, owner, accent: accent)
            } else if control is NSSwitch || kinds[owner.id] == "checkbox" || kinds[owner.id] == "radio" {
                if let on = owner.props["checked"].map({ $0 == "true" }) { setOn(control, on) }
                // NSSwitch takes the system accent; AppKit gives it no tint.
                (control as? NSButton)?.contentTintColor = accent
            } else {
                configureValue(control, owner, accent: accent)
            }
            if !(control is NativeButtonMac) {
                control.isEnabled = !owner.disabled
                control.setAccessibilityLabel(owner.props["accessibilityLabel"])
                control.setAccessibilityIdentifier(owner.props["testId"])
            }
            let natural = naturalSize(control)
            let box = owner.contentBox()
            // A slider's track spans its box, as the web's does; a native
            // button fills it, its chrome inside (LLP 1069.011 D6); the
            // others keep their own size, centred.
            if control is NativeButtonMac {
                // The box is the button's alignment rect, as its natural size
                // is; its bezel's shadow and insets fall outside it.
                let frame = control.frame(forAlignmentRect: box)
                if control.frame != frame { control.frame = frame }
            } else {
                let width = control is NSSlider ? box.width : natural.width
                control.frame = CGRect(x: box.midX - width / 2, y: box.midY - natural.height / 2,
                                       width: width, height: natural.height)
            }
            if reported[owner.id] != natural {
                reported[owner.id] = natural
                sizes.append((owner.id, natural))
            }
        }
        // Published outside the batch being applied, as images' are.
        // A control destroyed before then reports nothing.
        if !sizes.isEmpty {
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                let live = sizes.filter { self.controls[$0.0] != nil && self.presenter.views[$0.0] != nil }
                if !live.isEmpty { self.presenter.onIntrinsic?(live) }
            }
        }
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
        // A native button takes the ordinary click path (LLP 1069.011 D10).
        guard let control = controls[node.id], !(control is NativeButtonMac) else { return nil }
        guard control.window != nil, control.isEnabled, !node.inert, !control.isHiddenOrHasHiddenAncestor else { return false }
        // A click at a slider's middle moves its knob there, as a click
        // does; AppKit's own tracking loop would wait for a mouse-up the
        // agent's synthesized click never hands it.
        // A date field takes the focus, as a click into it does.
        if let picker = control as? NSDatePicker { return picker.window?.makeFirstResponder(picker) ?? false }
        if let slider = control as? NSSlider {
            _ = typeRange(slider, node, String((slider.minValue + slider.maxValue) / 2))
            return true
        }
        if !["checkbox", "switch", "radio"].contains(kinds[node.id]) { return openValue(control) }
        if let b = control as? NSButton { b.performClick(nil) } else {
            setOn(control, !isOn(control))
            changed(control)
        }
        return true
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard let control = controls[node.id] else { return nil }
        if let b = control as? NativeButtonMac { return nativeObservation(b) }
        if let value = valueObservation(control) {
            return value.merging(["size": [Agent.r2(control.frame.width), Agent.r2(control.frame.height)]]) { a, _ in a }
        }
        return ["view": control is NSSwitch ? "NSSwitch" : control is RadioButtonMac ? "NSButton(radio)" : "NSButton(checkbox)", "on": isOn(control),
                "size": [Agent.r2(control.frame.width), Agent.r2(control.frame.height)]]
    }

    func reset() {
        for control in controls.values { control.removeFromSuperview() }
        controls.removeAll()
        reported.removeAll()
        kinds.removeAll()
        menus.removeAll()
        lastRange.removeAll()
        appliedRange.removeAll()
    }
}
#endif
