// `input type="radio"` on AppKit (x2apps survey #2): an `NSButton` of the
// radio type, which assistive technology reads as AXRadioButton with its
// checked value, whose click never moves its own state — the group is
// Exact's (`Radios.swift`), so AppKit neither checks it nor groups it.
#if os(macOS)
import AppKit

/// A radio's cell: a click leaves its state alone and only sends the
/// action, so the host sees the state from before the click and decides.
final class RadioCell: NSButtonCell {
    override var nextState: Int { state.rawValue }
}

/// A radio button that takes no focus of its own: its node holds it, so
/// the node's `key` handlers and arrows hear the keys (`NodeView.keyDown`).
final class RadioButtonMac: NSButton {
    override class var cellClass: AnyClass? { get { RadioCell.self } set {} }
    override var acceptsFirstResponder: Bool { false }

    convenience init() {
        self.init(frame: .zero)
        setButtonType(.radio)
        title = ""
        imagePosition = .imageOnly
    }
}

extension ControlHost {
    func radioOn(_ control: NSControl) -> Bool { isOn(control) }

    func showRadio(_ id: UInt32, _ on: Bool) {
        guard let control = controls[id] else { return }
        setOn(control, on)
    }

    /// The focus moves to a radio, as Chrome focuses one a click or an
    /// arrow checks; a node that takes no focus leaves it where it is.
    func focusRadio(_ node: NodeView, keyboard: Bool = false) {
        guard let window = node.window, node.acceptsFirstResponder, window.firstResponder !== node else { return }
        window.makeFirstResponder(node)
    }

    @objc func radioClicked(_ sender: NSControl) {
        guard let node = presenter.views[UInt32(sender.tag)] else { return }
        focusRadio(node)
        checkRadio(node.id)
    }
}
#endif
