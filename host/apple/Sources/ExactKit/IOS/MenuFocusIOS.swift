// UIKit's menus (iOS 26 and later) take typing for type-to-select: while one
// shows, its key input (`_UITypeSelectKeyInput`) becomes first responder in
// place of a first responder that is not text. With no hardware keyboard
// attached that brings the software keyboard up over the menu's lower rows.
// Measured on the iOS 27 simulator with a bare UIKit app: a context menu shows
// no keyboard, and the same menu with a plain `UIView` as first responder
// shows it. Exact always has such a responder: `ExactView` holds the focus for
// key commands, and a pressed node takes it (NodeViewIOS `touchesEnded`). So
// a native menu sets that focus aside as it shows, without dispatching
// `blur`, and puts it back, without `focus`, when it has ended. A text
// field's focus stays: its keyboard is up already, and typing is its own.
// A bar item's menu (NavigationBarIOS) has no hook early enough: it takes
// the input from the focus it saw as it began, and setting it aside in its
// rows' provider is too late (measured). QUEUE has it.
#if os(iOS) || os(tvOS)
import UIKit

final class MenuFocus {
    private weak var presenter: Presenter?
    private weak var held: UIResponder?
    /// While set, a node gaining or losing the focus dispatches nothing.
    private(set) var quiet = false
    init(presenter: Presenter) { self.presenter = presenter }

    /// A menu is about to show.
    func setAside() {
        // The session's own: a node holding the focus itself (not its field
        // or text area), else the view.
        guard held == nil, let presenter else { return }
        let node = presenter.focusedNode.flatMap { $0.isFirstResponder ? $0 : nil }
        guard let responder: UIView = node ?? presenter.session?.view.flatMap({ $0.isFirstResponder ? $0 : nil }),
              responder.window != nil, presenter.focusedNode == nil || node != nil else { return }
        quiet = true
        defer { quiet = false }
        if responder.resignFirstResponder() { held = responder }
    }
    /// The menu has ended: the focus goes back, unless something else has
    /// taken it since (a chosen row's press may have moved it).
    func restore() {
        guard let responder = held else { return }
        held = nil
        guard let view = responder as? UIView, view.window != nil, responder.canBecomeFirstResponder else { return }
        if let now = FirstResponder.current, !String(describing: type(of: now)).hasPrefix("_UITypeSelect") { return }
        quiet = true
        defer { quiet = false }
        _ = responder.becomeFirstResponder()
    }
}

#endif

#if os(iOS)
/// An invoker's pull-down (`MenuHost` overlays): the focus set aside while its
/// menu shows.
final class MenuButton: UIButton {
    weak var focus: MenuFocus?
    override func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willDisplayMenuFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        super.contextMenuInteraction(interaction, willDisplayMenuFor: configuration, animator: animator)
        focus?.setAside()
    }
    override func contextMenuInteraction(_ interaction: UIContextMenuInteraction, willEndFor configuration: UIContextMenuConfiguration, animator: UIContextMenuInteractionAnimating?) {
        super.contextMenuInteraction(interaction, willEndFor: configuration, animator: animator)
        guard let animator else { focus?.restore(); return }
        animator.addCompletion { [weak self] in self?.focus?.restore() }
    }
}
#endif
