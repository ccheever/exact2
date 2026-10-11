// The Mac's ways back from a pushed route (LLP 1115.000 D5.1; LLP 1115 D5,
// "Back is always there"): Go ▸ Back ⌘[ in the menu bar (DevMenuMac.swift),
// a navigational toolbar item while a route is pushed (ToolbarMac.swift),
// and the trackpad's swipe (`BackSwipe`, below). Each goes the way iOS's
// Back button and edge swipe go (`NavigationRules.back`): an authored Back
// control is what they press, and a route that declares none goes back by
// the root's `navigate` with the location beneath, or with no `navigate`
// handler by the runner's own `back`. A sheet or modal route is not pushed:
// its close and Escape are its own.
#if os(macOS)
import AppKit

extension NavigationHost {
    /// The selected route when it was pushed, with its navigation root: not
    /// the first of its stack, nor the modal route a later push would stay
    /// inside (`NavigationRules.segments`), whose close is its own. Read
    /// from what the last batch showed (`selection`), so a batch pays nothing.
    var pushedRoute: (root: NodeView, route: NodeView)? {
        guard let selection, let root = presenter.views[selection.root], selection.stack.indices.contains(selection.at) else { return nil }
        let stack = selection.stack[...selection.at].compactMap { presenter.views[$0] }
        guard stack.count == selection.at + 1, let route = stack.last else { return nil }
        let owners = NavigationRules.segments(presentations: stack.map { $0.props["navigationPresentation"] })
        return selection.at > (owners.last?.lowerBound ?? 0) ? (root, route) : nil
    }

    /// The Back controls a route declares: the live elements in it whose
    /// `id` is the root's `navigationBack`.
    private func backControls(in route: NodeView, of root: NodeView) -> [NodeView] {
        guard let name = root.props["navigationBack"] else { return [] }
        return presenter.carrying("id").filter { $0.props["id"] == name && presenter.menus.contains(route, $0) }
    }

    /// Whether the platform's Back is offered (the toolbar's chevron shows):
    /// a route is pushed, and it is a visit of the router (its key is a
    /// visit's id, which has one beneath it) or declares a Back control. A
    /// declared control that is disabled keeps the way offered and refuses it.
    var backOffered: Bool {
        guard let (root, route) = pushedRoute else { return false }
        return route.props["navigationKey"].flatMap { UInt64($0) } != nil || !backControls(in: route, of: root).isEmpty
    }

    /// How Back goes from the selected route, resolved at use as iOS's is
    /// (`NavigationRules.back`); nil when it does not.
    var back: NavigationRules.Back? {
        guard let (root, route) = pushedRoute else { return nil }
        let named = backControls(in: route, of: root)
        let control = NavigationRules.backControl(named: root.props["navigationBack"], among: named, id: \.id, htmlID: { $0.props["id"] },
                                                  pressable: { $0.handlers.contains("press") }, disabled: \.disabled, inActiveRoute: { _ in true })
        return NavigationRules.back(control: control?.id, declared: !named.isEmpty, hearsNavigate: root.handlers.contains("navigate"),
                                    beneath: { [presenter] in route.props["navigationKey"].flatMap { presenter.session?.runtime.locationBeneath($0) } })
    }

    /// Whether Back goes now: a way back, and nothing modal over the page
    /// (a dialog in the top layer, a sheet on the window).
    var canGoBack: Bool {
        presenter.dialogs.active == nil && presenter.viewport.window?.attachedSheet == nil && back != nil
    }

    /// The platform's Back, taken: the authored control pressed, else the
    /// root's `navigate` with the location beneath, else the runner's `back`.
    @discardableResult func goBack() -> Bool {
        guard canGoBack, let (root, route) = pushedRoute, let how = back else { return false }
        switch how {
        case .press(let id): presenter.press(id)
        case .navigate(let location): presenter.session?.navigate(location, at: root)
        case .pop:
            if let key = route.props["navigationKey"] { presenter.session?.hostBack(key) }
        }
        return true
    }

    /// `state.navigation.back` (the agent): how Back goes now, or that it does not.
    var backObservation: Any {
        guard canGoBack, let how = back else { return NSNull() }
        switch how {
        case .press(let id): return ["press": Int(id)]
        case .navigate(let location): return ["navigate": location]
        case .pop: return "pop"
        }
    }
}

/// Go ▸ Back ⌘[: the way back of the session in the key window, which the
/// adapter that owns the windows keeps current (`DevMenu.session`); enabled
/// only while that session has somewhere to go back to.
final class GoMenuTarget: NSObject, NSMenuItemValidation {
    @objc func back(_ sender: Any?) { DevMenu.session?.presenter.navigation.goBack() }
    func validateMenuItem(_ item: NSMenuItem) -> Bool { DevMenu.session?.presenter.navigation.canGoBack == true }
}

/// The trackpad's swipe back. With two fingers it is AppKit's swipe
/// tracking from scroll events, as Safari's is (`trackSwipeEvent`: the
/// person's "Swipe between pages" setting, the direction lock, natural
/// scrolling and the distance that completes or cancels it are all
/// AppKit's); with three, when that is the setting, `swipe(with:)`. The
/// page does not follow the fingers: this host shows a route change at once,
/// a push too, so Back lands as the fingers lift.
final class BackSwipe {
    unowned let presenter: Presenter
    private(set) var tracking = false
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// AppKit's tracking of the rest of the gesture, calling back with its
    /// amount, phase and whether it is over. A test's stands in for it.
    var track: (NSEvent, @escaping (CGFloat, NSEvent.Phase, Bool) -> Void) -> Void = { event, handler in
        event.trackSwipeEvent(options: .lockDirection, dampenAmountThresholdMin: -1, max: 1) { amount, phase, complete, _ in
            handler(amount, phase, complete)
        }
    }

    /// Whether a scroll event starts a swipe back: a trackpad's gesture
    /// (phased, not its momentum), more sideways than not, toward the page
    /// before (content moving right, the direction as the person scrolls),
    /// with swipe tracking on in System Settings and a way back.
    static func begins(gesture: Bool, dx: CGFloat, dy: CGFloat, enabled: Bool, available: Bool) -> Bool {
        gesture && enabled && available && dx > 0 && abs(dx) > abs(dy)
    }

    /// A scroll event the page's scroll view passed up (it does for a
    /// sideways gesture it is pinned for, `wantsScrollEventsForSwipeTracking`).
    /// True when it began a swipe back, which AppKit then tracks. A script's
    /// wheel is not a trackpad's stream of events, and is never tracked.
    func scroll(_ event: NSEvent, enabled: Bool = NSEvent.isSwipeTrackingFromScrollEventsEnabled && !ExactEnv.agentMode) -> Bool {
        guard !tracking, Self.begins(gesture: !event.phase.isEmpty && event.momentumPhase.isEmpty, dx: event.scrollingDeltaX, dy: event.scrollingDeltaY,
                                     enabled: enabled, available: presenter.navigation.canGoBack) else { return false }
        tracking = true
        // The side the gesture set out for: one that ends on the other side
        // (the fingers came back past where they started) goes nowhere.
        var side: CGFloat = 0
        track(event) { [weak self] amount, phase, complete in
            if side == 0, amount != 0 { side = amount < 0 ? -1 : 1 }
            if phase == .ended, amount * side > 0 { self?.presenter.navigation.goBack() }
            if complete { self?.tracking = false }
        }
        return true
    }

    /// The three-finger swipe (`NSResponder.swipe(with:)`): `deltaX` is
    /// positive toward the page before, as AppKit reports it for the
    /// person's scroll direction.
    func swiped(deltaX: CGFloat) -> Bool {
        deltaX > 0 && presenter.navigation.goBack()
    }
}
#endif
