// @ref LLP 1069.001 D5 — `input type="checkbox"` is projected onto UIKit as
// a tablist is onto a segmented control: `switch` is a `UISwitch`; a plain
// checkbox is drawn as Safari iOS draws it, a rounded square filled with
// `accent-color` and a checkmark (UIKit has no checkbox). Contract owns the
// value: the control flips at once, reports, and shows the committed
// `checked` after the action (D4). `appearance: none` draws nothing native.
#if os(iOS)
import UIKit

/// Safari iOS's checkbox: a 16×16 rounded square, filled and checked when on.
final class ExactCheckbox: UIControl {
    var isOn = false { didSet { if isOn != oldValue { setNeedsDisplay(); updateAccessibility() } } }
    var accent: UIColor? { didSet { setNeedsDisplay() } }
    override var isEnabled: Bool { didSet { setNeedsDisplay(); updateAccessibility() } }
    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
        isAccessibilityElement = true
        addTarget(self, action: #selector(toggle), for: .touchUpInside)
        updateAccessibility()
    }
    required init?(coder: NSCoder) { nil }
    override var intrinsicContentSize: CGSize { CGSize(width: 16, height: 16) }
    @objc private func toggle() {
        isOn.toggle()
        sendActions(for: .valueChanged)
    }
    /// VoiceOver reads it as Safari's: a button with a checked state.
    private func updateAccessibility() {
        accessibilityTraits = isEnabled ? .button : [.button, .notEnabled]
        accessibilityValue = isOn ? "checked" : "unchecked"
    }
    override func draw(_ rect: CGRect) {
        let side = min(bounds.width, bounds.height)
        let box = CGRect(x: bounds.midX - side / 2, y: bounds.midY - side / 2, width: side, height: side)
        let fill = accent ?? tintColor ?? .systemBlue
        let alpha: CGFloat = isEnabled ? 1 : 0.4
        if isOn {
            fill.withAlphaComponent(alpha).setFill()
            UIBezierPath(roundedRect: box, cornerRadius: side * 0.25).fill()
            let check = UIBezierPath()
            check.move(to: CGPoint(x: box.minX + side * 0.25, y: box.minY + side * 0.52))
            check.addLine(to: CGPoint(x: box.minX + side * 0.43, y: box.minY + side * 0.7))
            check.addLine(to: CGPoint(x: box.minX + side * 0.76, y: box.minY + side * 0.32))
            check.lineWidth = max(1.5, side * 0.12)
            check.lineCapStyle = .round
            check.lineJoinStyle = .round
            UIColor.white.withAlphaComponent(alpha).setStroke()
            check.stroke()
        } else {
            let ring = UIBezierPath(roundedRect: box.insetBy(dx: 0.5, dy: 0.5), cornerRadius: side * 0.25)
            UIColor.systemBackground.withAlphaComponent(alpha).setFill()
            ring.fill()
            ring.lineWidth = 1
            UIColor.systemGray.withAlphaComponent(alpha).setStroke()
            ring.stroke()
        }
    }
}

final class ControlHost: NSObject {
    unowned let presenter: Presenter
    var controls: [UInt32: UIControl] = [:]
    /// Which control each node shows (`ControlKinds`), to remake it when that changes.
    private var kinds: [UInt32: String] = [:]
    /// The size last reported per control, so each is published once.
    private var reported: [UInt32: CGSize] = [:]
    /// A select's menu as last built, so a batch that leaves it alone does not rebuild it.
    var menus: [UInt32: SelectMenu] = [:]
    /// A range's last reported value while it moves, so each is sent once.
    var lastRange: [UInt32: String] = [:]

    init(_ presenter: Presenter) { self.presenter = presenter }

    /// The control the node shows, made (or remade, when its kind changes).
    private func control(for node: NodeView) -> UIControl {
        let kind = ControlKinds.kind(node.props)
        if let existing = controls[node.id], kinds[node.id] == kind { return existing }
        controls.removeValue(forKey: node.id)?.removeFromSuperview()
        menus.removeValue(forKey: node.id)
        let made: UIControl
        switch kind {
        case "switch": made = UISwitch()
        case "checkbox": made = ExactCheckbox(frame: .zero)
        default: made = makeValueControl(kind, node.id)
        }
        made.tag = Int(node.id)
        if kind == "switch" || kind == "checkbox" { made.addTarget(self, action: #selector(changed(_:)), for: .valueChanged) }
        controls[node.id] = made
        kinds[node.id] = kind
        return made
    }

    func sync() {
        let owners = ControlKinds.indexed.flatMap { presenter.carrying($0) }.filter { $0.kind == "control" }
        let live = Set(owners.map(\.id))
        for id in Array(controls.keys) where !live.contains(id) {
            controls.removeValue(forKey: id)?.removeFromSuperview()
            reported.removeValue(forKey: id)
            kinds.removeValue(forKey: id)
            menus.removeValue(forKey: id)
        }
        var sizes: [(UInt32, CGSize?)] = []
        for owner in owners {
            let native = owner.style["appearance"]?.string != "none"
            let control = control(for: owner)
            if !native {
                // The author's box is the look; the node keeps its role.
                control.removeFromSuperview()
                continue
            }
            if control.superview !== owner { owner.addSubview(control) }
            let on = owner.props["checked"].map { $0 == "true" }
            let accent = owner.channels("accent_color").map { TextEngine.color($0) }
            if let s = control as? UISwitch {
                if let on, s.isOn != on { s.setOn(on, animated: s.window != nil) }
                s.onTintColor = accent
            } else if let c = control as? ExactCheckbox {
                if let on { c.isOn = on }
                c.accent = accent
            } else {
                configureValue(control, owner, accent: accent)
            }
            control.isEnabled = !owner.disabled
            control.accessibilityLabel = owner.props["accessibilityLabel"]
            control.accessibilityIdentifier = owner.props["testId"]
            let natural = naturalSize(control, owner)
            let box = owner.contentBox()
            // A slider's track spans its box, as the web's does; the others
            // keep their own size, centred.
            let width = control is UISlider ? box.width : natural.width
            control.frame = CGRect(x: box.midX - width / 2, y: box.midY - natural.height / 2,
                                   width: width, height: natural.height)
            if reported[owner.id] != natural {
                reported[owner.id] = natural
                sizes.append((owner.id, natural))
            }
        }
        // Published outside the batch being applied, as images' are.
        if !sizes.isEmpty { DispatchQueue.main.async { [weak self] in self?.presenter.onIntrinsic?(sizes) } }
    }

    @objc private func changed(_ sender: UIControl) {
        let id = UInt32(sender.tag)
        guard presenter.views[id] != nil else { return }
        let on = (sender as? UISwitch)?.isOn ?? (sender as? ExactCheckbox)?.isOn ?? false
        presenter.checked(id, on)
        // The committed state is authoritative: an action that refused the
        // toggle snaps the control back (D4).
        if let committed = presenter.views[id]?.props["checked"].map({ $0 == "true" }) {
            if let s = sender as? UISwitch, s.isOn != committed { s.setOn(committed, animated: true) }
            if let c = sender as? ExactCheckbox, c.isOn != committed { c.isOn = committed }
        }
    }

    /// The agent's `tap` (LLP 1069.001 D9): the control's own activation.
    func activate(_ node: NodeView) -> Bool? {
        guard let control = controls[node.id] else { return nil }
        guard control.window != nil, control.isEnabled, !node.inert else { return false }
        if let s = control as? UISwitch { s.setOn(!s.isOn, animated: false); s.sendActions(for: .valueChanged) }
        else if control is ExactCheckbox { control.sendActions(for: .touchUpInside) }
        else { return openValue(control) }
        return true
    }

    func observation(_ node: NodeView) -> [String: Any]? {
        guard let control = controls[node.id] else { return nil }
        if let value = valueObservation(control) {
            return value.merging(["size": [Agent.r2(control.bounds.width), Agent.r2(control.bounds.height)]]) { a, _ in a }
        }
        let on = (control as? UISwitch)?.isOn ?? (control as? ExactCheckbox)?.isOn ?? false
        return ["view": control is UISwitch ? "UISwitch" : "checkbox", "on": on,
                "size": [Agent.r2(control.bounds.width), Agent.r2(control.bounds.height)]]
    }

    func reset() {
        for control in controls.values { control.removeFromSuperview() }
        controls.removeAll()
        reported.removeAll()
        kinds.removeAll()
        menus.removeAll()
    }
}
#endif
