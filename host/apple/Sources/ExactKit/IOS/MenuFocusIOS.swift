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
// rows' provider is too late (measured). So a navigation bar holding a menu
// item sets the focus aside at the touch that may open it (`BarTouch`). Its
// menu says nothing when it ends, and the focus cannot come back while it
// shows: the menu's input takes it again and the keyboard rises (measured).
// So the view's focus comes back at the next touch on the page, once the
// menu has gone; a node's is blurred, as a touch on a page's chrome blurs
// an element on the web.
// `GCKeyboard` cannot tell when to skip this: the simulator reports a
// keyboard while UIKit shows its software one.
#if os(iOS) || os(tvOS)
import UIKit

final class MenuFocus {
    private weak var presenter: Presenter?
    private weak var held: UIResponder? { didSet { heldIncarnation = (held as? NodeView)?.incarnation } }
    /// The held node's incarnation: a view the node pool has since lent to
    /// another node is not given the focus it set aside.
    private var heldIncarnation: UInt64?
    /// While set, a node gaining or losing the focus dispatches nothing.
    private(set) var quiet = false
    /// The focus a bar's touch set aside waits for the next touch on the view.
    private(set) var untilNextTouch = false
    init(presenter: Presenter) { self.presenter = presenter }

    /// A menu is about to show.
    func setAside(untilTouch: Bool = false) {
        // The session's own: a node holding the focus itself (not its field
        // or text area), else the view.
        guard let presenter else { return }
        let node = presenter.focusedNode.flatMap { $0.isFirstResponder ? $0 : nil }
        guard let responder: UIView = node ?? presenter.session?.view.flatMap({ $0.isFirstResponder ? $0 : nil }),
              responder.window != nil, presenter.focusedNode == nil || node != nil else {
            // Nothing to set aside now (a bar touched again, say): what is
            // held stays held, and a menu with an end returns it then.
            if !untilTouch { untilNextTouch = false }
            return
        }
        if untilTouch, node != nil {
            // A touch on the bar leaves a node as a touch on the page's chrome
            // leaves an element on the web: blurred. The view, which hears key
            // commands, takes the focus at the next touch on the page (not
            // now: the menu's input would take it, raising the keyboard).
            guard responder.resignFirstResponder() else { return }
            releaseView()
            held = presenter.session?.view
            untilNextTouch = held != nil
            return
        }
        quiet = true
        defer { quiet = false }
        guard responder.resignFirstResponder() else { return }
        releaseView()
        held = responder
        untilNextTouch = untilTouch
    }
    /// A node that resigns hands the focus to the view, its nearest ancestor
    /// that takes it (UIKit's fallback, measured): the view lets it go too,
    /// or the menu's input takes it from the view.
    private func releaseView() {
        if let view = presenter?.session?.view, view.isFirstResponder { _ = view.resignFirstResponder() }
    }
    /// Navigation bars' touches set the focus aside while their items hold a
    /// menu, and the next touch on the page brings it back (`BarTouch`): one
    /// recognizer per bar, and one on the viewport, which a presentation
    /// carries with the page.
    func watch(_ bar: UINavigationBar) {
        if !(bar.gestureRecognizers ?? []).contains(where: { $0 is BarTouch }) { bar.addGestureRecognizer(BarTouch(focus: self, bar: true)) }
        if let view = presenter?.viewport, !(view.gestureRecognizers ?? []).contains(where: { $0 is BarTouch }) {
            view.addGestureRecognizer(BarTouch(focus: self, bar: false))
        }
    }
    /// A touch on the page, with a bar's focus still aside.
    func touched() { if untilNextTouch { restore() } }
    /// The menu has ended: the focus goes back, unless something else has
    /// taken it since (a chosen row's press may have moved it).
    func restore() {
        untilNextTouch = false
        guard let responder = held else { return }
        let incarnation = heldIncarnation
        held = nil
        guard let view = responder as? UIView, view.window != nil, responder.canBecomeFirstResponder else { return }
        if let node = view as? NodeView, node.incarnation != incarnation || presenter?.views[node.id] !== node { return }
        if let now = FirstResponder.current, !String(describing: type(of: now)).hasPrefix("_UITypeSelect") { return }
        quiet = true
        defer { quiet = false }
        _ = responder.becomeFirstResponder()
    }
}

/// On a navigation bar whose items hold a menu, a touch sets the focus aside
/// as it begins, before a bar item's menu decides on its input; on the
/// viewport, a touch brings it back. It recognizes nothing and holds
/// no touch from either.
final class BarTouch: UIGestureRecognizer, UIGestureRecognizerDelegate {
    private weak var focus: MenuFocus?
    private let bar: Bool
    init(focus: MenuFocus, bar: Bool) {
        self.focus = focus; self.bar = bar
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false; delaysTouchesBegan = false; delaysTouchesEnded = false
        delegate = self
    }
    private var holdsMenu: Bool {
        guard let item = (view as? UINavigationBar)?.topItem else { return false }
        return ((item.leftBarButtonItems ?? []) + (item.rightBarButtonItems ?? [])).contains { $0.menu != nil }
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        if !bar {
            // Not a touch on a bar, which this view holds too.
            let onBar = sequence(first: touches.first?.view, next: { $0?.superview }).contains { $0 is UINavigationBar }
            if !onBar { focus?.touched() }
        } else if holdsMenu { focus?.setAside(untilTouch: true) }
        state = .failed
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
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
