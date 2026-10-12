// `input type="radio"` on AppKit (x2apps survey #2): an `NSButton` of the
// radio type, which assistive technology reads as AXRadioButton with its
// checked value, whose click never moves its own state — the group is
// Exact's (`Radios.swift`), so AppKit neither checks it nor groups it.
#if os(macOS)
import AppKit

/// A radio's cell: a click leaves its state alone and only sends the
/// action, so the host sees the state from before the click and decides.
final class RadioCell: AriaButtonCellMac {
    override var nextState: Int { state.rawValue }
}

/// A radio button that takes no focus of its own: its node holds it, so
/// the node's `key` handlers and arrows hear the keys (`NodeView.keyDown`).
final class RadioButtonMac: NSButton {
    override class var cellClass: AnyClass? { get { RadioCell.self } set {} }
    override var acceptsFirstResponder: Bool { false }
    override func mouseDown(with event: NSEvent) { if !contextClicked(event) { super.mouseDown(with: event) } }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if attribute == .enabled, ariaUnavailableFromNode { return false }
        return super.accessibilityAttributeValue(attribute)
    }

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

    /// The focus moves to the radio an arrow checks, from the radio that
    /// had it; a node that takes no focus leaves it where it is.
    func focusRadio(_ node: NodeView, keyboard: Bool = false) {
        guard let window = node.window, node.acceptsFirstResponder, window.firstResponder !== node else { return }
        window.makeFirstResponder(node)
    }

    /// A click checks the radio and moves no focus, as a click on AppKit's
    /// radio button does (LLP 1115.000 D7.1): a field being edited keeps its
    /// editor, its edit committed before the radio's `change`.
    @objc func radioClicked(_ sender: NSControl) {
        guard let node = presenter.views[UInt32(sender.tag)] else { return }
        presenter.commitPendingEdit()
        checkRadio(node.id)
    }
}
#endif
