// The Siri Remote on tvOS (branch proof of concept). UIKit's focus engine
// moves the focus among exact2's nodes — what a keyboard can focus, and any
// enabled press target — and reports each move as focus and blur. Select
// presses the focused node. Menu presses the active route's Back control
// (LLP 1075 D1) while a route can pop; otherwise its recognizer is removed,
// so Menu reaches tvOS and leaves the app, as tvOS requires at an app's root.
#if os(tvOS)
import UIKit

extension NodeView {
    override var canBecomeFocused: Bool {
        canBecomeFirstResponder || (!disabled && !inert && handlers.contains("press"))
    }

    override func didUpdateFocus(in context: UIFocusUpdateContext, with coordinator: UIFocusAnimationCoordinator) {
        super.didUpdateFocus(in: context, with: coordinator)
        if context.nextFocusedItem === self {
            showFocusRing(true)
            if handlers.contains("focus") { presenter?.focus(id) }
        } else if context.previouslyFocusedItem === self {
            showFocusRing(false)
            if handlers.contains("blur") { presenter?.blur(id) }
        }
    }

    /// Whether `presses` hold Select for this node; a Select going down presses it.
    func remoteSelect(_ presses: Set<UIPress>, down: Bool) -> Bool {
        guard presses.contains(where: { $0.type == .select }), !disabled, handlers.contains("press") else { return false }
        if down { presenter?.press(id) }
        return true
    }
}

/// The Menu recognizer, present only while Menu goes back.
final class MenuKey: NSObject {
    unowned let presenter: Presenter
    private var tap: UITapGestureRecognizer?

    init(presenter: Presenter) { self.presenter = presenter }

    func sync() {
        let wanted = presenter.navigation.menuGoesBack
        if wanted, tap == nil, let view = presenter.session?.view {
            let recognizer = UITapGestureRecognizer(target: self, action: #selector(menu))
            recognizer.allowedPressTypes = [NSNumber(value: UIPress.PressType.menu.rawValue)]
            view.addGestureRecognizer(recognizer)
            tap = recognizer
        } else if !wanted, let recognizer = tap {
            recognizer.view?.removeGestureRecognizer(recognizer)
            tap = nil
        }
    }

    @objc private func menu() { presenter.navigation.menuBack() }
}
#endif
