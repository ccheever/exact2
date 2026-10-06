// The Siri Remote on tvOS (LLP 1008 §9). UIKit's focus engine
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
    /// An explicit negative `tabindex` is no remote stop, as it is no Tab
    /// stop (LLP 1088 D7.3); the remote's order stays UIKit's geometry.
    override var canBecomeFocused: Bool {
        if let index = explicitTabIndex, index < 0 { return false }
        if cssVisibilityHidden { return false } // no remote stop, nor Select (e28279b3b)
        return canBecomeFirstResponder || (!disabled && !inert && handlers.contains("press"))
    }

    override func didUpdateFocus(in context: UIFocusUpdateContext, with coordinator: UIFocusAnimationCoordinator) {
        super.didUpdateFocus(in: context, with: coordinator)
        if context.nextFocusedItem === self {
            presenter?.focusKey = props["testId"]
            presenter?.focusGuides.focused(self)
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
        guard presses.contains(where: { $0.type == .select }), !disabled, !cssVisibilityHidden, handlers.contains("press") else { return false }
        if down { presenter?.press(id) }
        return true
    }
}

extension Presenter {
    /// The shown, focusable node with the `testId` that last held the focus.
    var focusReturn: NodeView? {
        guard let key = focusKey else { return nil }
        return views.values.first { $0.props["testId"] == key && $0.window != nil && $0.canBecomeFocused }
    }
}

extension ExactView {
    /// When the focused view goes (a branch that swaps its subtree), the
    /// focus engine asks where focus belongs: the replacement with the same
    /// `testId`, rather than whatever the engine would pick.
    override public var preferredFocusEnvironments: [any UIFocusEnvironment] {
        if let node = session.presenter.focusReturn { return [node] }
        return super.preferredFocusEnvironments
    }
}

/// `focusGuide="auto"`: a focus guide over each such node, so a remote move
/// that enters its box from outside lands on the descendant that last held
/// the focus, else on the node (the engine then picks its first focusable
/// item) — react-native-tvos's TVFocusGuideView `autoFocus`. A guide is off
/// while the focus is inside its node, so moves within it stay the engine's.
final class FocusGuides {
    unowned let presenter: Presenter
    private var guides: [UInt32: (node: NodeView, guide: UIFocusGuide)] = [:]
    private var last: [UInt32: WeakNode] = [:]
    private final class WeakNode { weak var node: NodeView?; init(_ node: NodeView) { self.node = node } }

    init(presenter: Presenter) { self.presenter = presenter }

    func sync() {
        let wanted = Set(presenter.chrome.ids("focusGuide").filter { presenter.views[$0]?.props["focusGuide"] == "auto" })
        for (id, entry) in guides where !wanted.contains(id) || presenter.views[id] !== entry.node {
            entry.node.removeLayoutGuide(entry.guide)
            guides[id] = nil
            last[id] = nil
        }
        for id in wanted where guides[id] == nil {
            guard let node = presenter.views[id] else { continue }
            let guide = UIFocusGuide()
            node.addLayoutGuide(guide)
            NSLayoutConstraint.activate([
                guide.leadingAnchor.constraint(equalTo: node.leadingAnchor), guide.trailingAnchor.constraint(equalTo: node.trailingAnchor),
                guide.topAnchor.constraint(equalTo: node.topAnchor), guide.bottomAnchor.constraint(equalTo: node.bottomAnchor),
            ])
            guides[id] = (node, guide)
        }
        let window = presenter.session?.view?.window
        update(window.flatMap { UIFocusSystem.focusSystem(for: $0)?.focusedItem } as? UIView)
    }

    /// `view` took the focus: each guide around it remembers it.
    func focused(_ view: NodeView) {
        for (id, entry) in guides where view.isDescendant(of: entry.node) { last[id] = WeakNode(view) }
        update(view)
    }

    private func update(_ focused: UIView?) {
        for (id, entry) in guides {
            entry.guide.isEnabled = !(focused?.isDescendant(of: entry.node) ?? false)
            let remembered = last[id]?.node.flatMap { $0.window != nil && $0.isDescendant(of: entry.node) && $0.canBecomeFocused ? $0 : nil }
            entry.guide.preferredFocusEnvironments = [remembered ?? entry.node]
        }
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
