// The Siri Remote on tvOS (branch proof of concept). UIKit's focus engine
// moves the focus among exact2's nodes — what a keyboard can focus, and any
// enabled press target — and reports each move as focus and blur. Select
// presses the focused node. Menu presses the active route's Back control
// (LLP 1075 D1) while a route can pop, else a shown button that declares
// `aria-keyshortcuts="Escape"` (the key macOS presses it with). With neither,
// its recognizer is removed, so Menu reaches tvOS and leaves the app, as
// tvOS requires at an app's root.
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
            repaintThrough()
            if handlers.contains("focus") { presenter?.focus(id) }
        } else if context.previouslyFocusedItem === self {
            showFocusRing(false)
            repaintThrough()
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
        let wanted = presenter.navigation.menuGoesBack || escapeControl != nil
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

    /// The shown, enabled press target that declares Escape as its key.
    private var escapeControl: NodeView? {
        presenter.carrying("accessibilityKeyShortcuts").first {
            $0.window != nil && !sequence(first: $0 as UIView, next: \.superview).contains(where: \.isHidden) && !$0.disabled && !$0.inert && $0.handlers.contains("press")
                && ($0.props["accessibilityKeyShortcuts"] ?? "").split(whereSeparator: \.isWhitespace).contains("Escape")
        }
    }

    @objc private func menu() {
        if presenter.navigation.menuGoesBack { presenter.navigation.menuBack() } else if let control = escapeControl { presenter.press(control.id) }
    }
}
#endif
