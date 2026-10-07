// `input type="radio"` on UIKit (x2apps survey #2), which has no radio
// control: drawn as Safari iOS draws one — a circle, filled with
// `accent-color` and a white inner dot when checked — the way the checkbox
// is (`ExactCheckbox`). A tap never moves its own state: the group is
// Exact's (`Radios.swift`).
#if os(iOS) || os(tvOS)
import UIKit

final class ExactRadio: ExactCheckbox {
    /// A tap moves nothing here: the host hears it (`radioTapped`), and
    /// checks it and unchecks the rest of its group.
    override func activated() {}

    /// UIKit has no public radio trait: VoiceOver reads it as Safari's ARIA
    /// radios read here (`setAccessibilityChecked`), a button that is
    /// selected while checked, with no checkbox value.
    override func updateAccessibility() {
        var traits: UIAccessibilityTraits = .button
        if isOn { traits.insert(.selected) }
        if !isEnabled { traits.insert(.notEnabled) }
        accessibilityTraits = traits
        accessibilityValue = nil
    }

    override func draw(_ rect: CGRect) {
        let side = min(bounds.width, bounds.height)
        let box = CGRect(x: bounds.midX - side / 2, y: bounds.midY - side / 2, width: side, height: side)
        let alpha: CGFloat = isEnabled ? 1 : 0.4
        if isOn {
            (accent ?? tintColor ?? .systemBlue).withAlphaComponent(alpha).setFill()
            UIBezierPath(ovalIn: box).fill()
            UIColor.white.withAlphaComponent(alpha).setFill()
            UIBezierPath(ovalIn: box.insetBy(dx: side * 0.3, dy: side * 0.3)).fill()
        } else {
            let ring = UIBezierPath(ovalIn: box.insetBy(dx: 0.5, dy: 0.5))
            #if os(tvOS)
            UIColor.clear.setFill()
            #else
            UIColor.systemBackground.withAlphaComponent(alpha).setFill()
            #endif
            ring.fill()
            ring.lineWidth = 1
            UIColor.systemGray.withAlphaComponent(alpha).setStroke()
            ring.stroke()
        }
    }
}

extension ControlHost {
    func radioOn(_ control: UIControl) -> Bool { (control as? ExactCheckbox)?.isOn ?? false }

    func showRadio(_ id: UInt32, _ on: Bool) { (controls[id] as? ExactRadio)?.isOn = on }

    /// The arrows move the focus with the check, as on the web; a tap does
    /// not focus a radio, as Safari iOS does not. The ring shows for a
    /// keyboard's move (`showFocusRing`).
    func focusRadio(_ node: NodeView, keyboard: Bool = false) {
        guard !node.isFirstResponder, node.canBecomeFirstResponder, node.becomeFirstResponder() else { return }
        if keyboard { node.showFocusRing(true) }
    }

    @objc func radioTapped(_ sender: UIControl) { checkRadio(UInt32(sender.tag)) }
}
#endif
