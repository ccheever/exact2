// The Mac's ways back from a pushed route (LLP 1115.000 D5.1; LLP 1115 D5,
// "Back is always there"): Go ▸ Back ⌘[ in the menu bar (DevMenuMac.swift),
// a navigational toolbar item, disabled at a stack's root (ToolbarMac.swift),
// and the trackpad's swipe (`BackSwipe`, below). Each goes the way iOS's
// Back button and edge swipe go (`NavigationBack.way`): an authored Back
// control is what they press, and a route that declares none goes back by
// the root's `navigate` with the location beneath, or with no `navigate`
// handler by the runner's own `back`. A sheet or modal route is not pushed:
// its close and Escape are its own.
//
// Forward (LLP 1115.000 D5.5) goes where Back left, as a browser's does: Go ▸
// Forward ⌘], the swipe the other way, and a mouse's forward button. The
// mouse's back and forward buttons go Back and Forward at their release
// unless a `pointerup` prevented it, which is Chrome's rule.
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
        // In a split (LLP 1115.002 D4) the detail's pane begins at a row's
        // detail, which was not pushed: Back leads from what is above it.
        let lower = owners.last?.lowerBound ?? 0
        return selection.at > (lower == 0 ? split.base : lower) ? (root, route) : nil
    }

    /// The Back controls a route declares: the live elements in it whose
    /// `id` is the root's `navigationBack`.
    private func backControls(in route: NodeView, of root: NodeView) -> [NodeView] {
        guard let name = root.props["navigationBack"] else { return [] }
        // The views of that name alone (`ChromeIndex.named`), in id order as `carrying` gives them:
        // asked by every batch's check of the window's bar, not a sort of every named view.
        return (presenter.chrome.named[name] ?? []).sorted().compactMap { presenter.views[$0] }.filter { presenter.menus.contains(route, $0) }
    }

    /// What `canGoBack` decides from, without the runner: the pushed route,
    /// its Back controls and theirs states, whether the root hears
    /// `navigate`, and what is modal. Where none of these moved, only the
    /// router's place beneath can, and a router op says so (the window's
    /// bar presents for it); `locationBeneath` is a call to the owner.
    var backFacts: [UInt32] {
        guard let (root, route) = pushedRoute else { return [] }
        let controls: [UInt32] = backControls(in: route, of: root).flatMap { control -> [UInt32] in
            [control.id, (control.disabled ? 1 : 0) | (control.handlers.contains("press") ? 2 : 0)]
        }
        let facts: [UInt32] = [route.id, root.handlers.contains("navigate") ? 1 : 0, presenter.alerts.modal == nil ? 0 : 1,
                               presenter.dialogs.routeActive ? 1 : 0]
        return facts + controls
    }

    /// Whether the platform's Back is offered (the toolbar's chevron shows):
    /// a route is pushed, and it is a visit of the router (its key is a
    /// visit's id, which has one beneath it) or declares a Back control. A
    /// declared control that is disabled keeps the way offered and refuses it.
    var backOffered: Bool {
        guard let (root, route) = pushedRoute else { return false }
        return route.props["navigationKey"].flatMap { UInt64($0) } != nil || !backControls(in: route, of: root).isEmpty
    }

    /// Whether the app has a navigation stack: its navigation root is
    /// mounted (lowering gives the first root with a `navigationKey` a
    /// `navigationBack`, named or empty).
    var hasStack: Bool { !presenter.carrying("navigationBack").isEmpty }

    /// Whether the window's toolbar shows the chevron: the selected route,
    /// pushed or the first of its stack, did not ask for a screen with no
    /// bar (`navigationBar="none"`, LLP 1115.000 D5.2). At the root of a
    /// stack it is there and disabled (`canGoBack`), as Finder's and
    /// Safari's is, so the title bar is the same on every route. The author
    /// hid the bar, not the way back: Go ▸ Back, ⌘[ and the swipe go from a
    /// route with no bar as from any other.
    ///
    /// In a window that shows a list beside its detail (LLP 1115.002 D4, §5
    /// Q10) the chevron comes with a push inside the detail and goes with
    /// it, as SwiftUI's does: a list beside its detail has no Back to show,
    /// and a chevron that is never enabled is not in a hand-built window.
    var backInToolbar: Bool {
        guard hasStack else { return false }
        guard let selection, selection.stack.indices.contains(selection.at) else { return true }
        guard presenter.views[selection.stack[selection.at]]?.props["navigationBar"] != "none" else { return false }
        return !split.active || pushedRoute != nil
    }

    /// How Back goes from the selected route, resolved at use as iOS's is
    /// (`NavigationBack.way`); nil when it does not.
    var back: NavigationBack.Way? { pushedRoute.flatMap { way(from: $0.root, $0.route) } }

    /// How Back goes from `route`, under `root`.
    private func way(from root: NodeView, _ route: NodeView) -> NavigationBack.Way? {
        let named = backControls(in: route, of: root)
        let control = NavigationBack.control(named: root.props["navigationBack"], among: named, id: \.id, htmlID: { $0.props["id"] },
                                                  pressable: { $0.handlers.contains("press") }, disabled: \.disabled, inActiveRoute: { _ in true })
        return NavigationBack.way(control: control?.id, declared: !named.isEmpty, hearsNavigate: root.handlers.contains("navigate"),
                                    beneath: { [presenter] in route.props["navigationKey"].flatMap { presenter.session?.runtime.locationBeneath($0) } })
    }

    /// Whether Back goes now: a way back, and nothing modal over the page
    /// (a dialog in the top layer, a sheet on the window).
    var canGoBack: Bool {
        guard back != nil, presenter.alerts.modal == nil else { return false }
        if let dialog = presenter.dialogs.active {
            return pushedRoute?.route === dialog && presenter.dialogs.routeActive && dialog.window?.attachedSheet == nil
        }
        return presenter.viewport.window?.attachedSheet == nil
    }

    /// The platform's Back, taken: the authored control pressed, else the
    /// root's `navigate` with the location beneath, else the runner's `back`.
    @discardableResult func goBack() -> Bool {
        guard canGoBack, let (root, route) = pushedRoute, let how = back else { return false }
        take(how, from: root, route)
        return true
    }

    private func take(_ how: NavigationBack.Way, from root: NodeView, _ route: NodeView) {
        switch how {
        // A Back control its sheet's row stands for is pressed as that row presses it (RouteSheetMac.swift).
        case .press(let id): presenter.press(id, fromNativeMenu: presenter.views[id].flatMap(presenter.dialogs.rowShows) != nil)
        case .navigate(let location): presenter.session?.navigate(location, at: root)
        case .pop:
            if let key = route.props["navigationKey"] { presenter.session?.hostBack(key) }
        }
    }

    /// The selected route when it is a sheet (a modal route over the route
    /// beneath it), with its navigation root.
    private var sheetRoute: (root: NodeView, route: NodeView)? {
        guard let selection, selection.at > 0, selection.stack.indices.contains(selection.at),
              let root = presenter.views[selection.root], let route = presenter.views[selection.stack[selection.at]],
              ["modal", "fullscreen"].contains(route.props["navigationPresentation"] ?? "") else { return nil }
        return (root, route)
    }

    /// Where Forward goes (LLP 1115.000 D5.5): the nearest location Back
    /// left on the selected stack, which the runner keeps as a browser's
    /// session history does, and the navigation root that hears it.
    private var ahead: (root: NodeView, location: String)? {
        guard let selection, let root = presenter.views[selection.root],
              let location = presenter.session?.runtime.locationAhead() else { return nil }
        return (root, location)
    }

    /// Whether Forward goes now: somewhere ahead, and nothing modal over
    /// the page (an alert, a dialog in the top layer, a sheet on the window).
    var canGoForward: Bool {
        presenter.alerts.modal == nil && presenter.dialogs.active == nil
            && presenter.viewport.window?.attachedSheet == nil && ahead != nil
    }

    /// The platform's Forward, taken as the web host takes a browser's (LLP
    /// 1038 D7): the root's `navigate` with the location ahead, else the
    /// runner's own `go`.
    @discardableResult func goForward() -> Bool {
        guard canGoForward, let (root, location) = ahead else { return false }
        if root.handlers.contains("navigate") { presenter.session?.navigate(location, at: root) }
        else { presenter.session?.hostGo(location) }
        return true
    }

    /// The agent's `back` from a route (LLP 1116.006 D4): a pushed one's
    /// as ⌘[ takes it (`goBack`); a sheet's as iOS's swipe down does, which
    /// its `closedby="none"` refuses. nil with neither on top; the way, and
    /// why the app refused it ("" when it did not).
    func agentBack() -> (way: String, refused: String)? {
        if pushedRoute != nil {
            if !canGoBack && back != nil { return ("route", "a dialog or a sheet is open over the page") }
            return ("route", goBack() ? "" : "its navigationBack control is disabled or has no press handler")
        }
        if let (_, route) = sheetRoute { return ("sheet", dismiss(route) ?? "") }
        // A split's detail has no Back (LLP 1115.002 D4). A person leaves
        // it by taking the selection off its row, and the pane's routes go
        // as their stack goes back: that is the way back from it.
        guard split.active, split.shown?.detail != nil else { return nil }
        return ("route", split.closeDetail() ? "" : "the detail's stack did not go back: its navigationBack control is disabled or has no press handler")
    }

    /// The platform's dismissal of a modal route: Escape or Command-period
    /// in its sheet and a click behind a `closedby="any"` one (LLP 1115.002
    /// D7.4), and the agent's `back`. It goes as the stack goes back, which
    /// the app decides; nil when it went, else why it was refused.
    @discardableResult func dismiss(_ route: NodeView) -> String? {
        let route = presenter.dialogs.dismissalOwner(of: route)
        guard let selection, let root = presenter.views[selection.root] else { return "its stack has no navigation root" }
        if NavigationRules.modalRefusesDismissal(closedby: route.props["closedby"]) { return "its closedby is \"none\"" }
        guard let how = way(from: root, route) else { return "its navigationBack control is disabled or has no press handler" }
        if case .press(let id) = how, presenter.dialogs.active !== route, presenter.dialogs.entries.contains(where: { $0.routeBase === route }) {
            // A pushed child hides and gates the base's control. Dismissing its sheet still invokes that control's action.
            guard let control = presenter.views[id], !sequence(first: control as NSView, next: { self.presenter.menus.parent(of: $0) })
                .contains(where: { ($0 as? NodeView)?.props["inert"] == "true" }) else { return "its navigationBack control is inert" }
            presenter.commitPendingEdit()
            presenter.onPress?(id)
        } else { take(how, from: root, route) }
        return nil
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

/// Go ▸ Back ⌘[ and Forward ⌘]: the ways of the session in the key window,
/// which the adapter that owns the windows keeps current (`DevMenu.session`);
/// each enabled only while that session has somewhere to go.
final class GoMenuTarget: NSObject, NSMenuItemValidation {
    @objc func back(_ sender: Any?) { DevMenu.session?.presenter.navigation.goBack() }
    @objc func forward(_ sender: Any?) { DevMenu.session?.presenter.navigation.goForward() }
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let navigation = DevMenu.session?.presenter.navigation else { return false }
        return item.action == #selector(forward(_:)) ? navigation.canGoForward : navigation.canGoBack
    }
}

extension Presenter {
    /// A mouse's back or forward button came up (`buttonNumber` 3 or 4): Back
    /// or Forward, after the event's `pointerup` has run, unless one of its
    /// handlers prevented it (LLP 1115.000 D5.5). Chrome steps history at
    /// the release and lets a `mouseup` or `pointerup` cancel it; a press, a
    /// `pointerdown` or an `auxclick` does not.
    func historyButton(_ event: NSEvent) {
        guard event.type == .otherMouseUp, [3, 4].contains(event.buttonNumber) else { return }
        // A turn later: the window dispatches the up after this monitor.
        DispatchQueue.main.async { [weak self] in
            guard let self, self.mouseChain.upPrevented !== event else { return }
            if event.buttonNumber == 3 { self.navigation.goBack() } else { self.navigation.goForward() }
        }
    }
}

/// The trackpad's swipe back and forward. With two fingers it is AppKit's
/// swipe tracking from scroll events, as Safari's is (`trackSwipeEvent`: the
/// person's "Swipe between pages" setting, the direction lock, natural
/// scrolling and the distance that completes or cancels it are all
/// AppKit's); with three, when that is the setting, `swipe(with:)`, which is
/// also how some mouse drivers send their side buttons. The page does not
/// follow the fingers: this host shows a route change at once, a push too,
/// so Back or Forward lands as the fingers lift.
///
/// Only a scroll nothing took arrives: a scroller that can move that way
/// takes it, one whose `overscroll-behavior-x` is `contain` or `none` keeps
/// it at its edge, and a `wheel` handler's `preventDefault()` stops it, as
/// Chrome's history swipe is (LLP 1115.000 D5.5).
final class BackSwipe {
    unowned let presenter: Presenter
    private(set) var tracking = false
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// AppKit's tracking of the rest of the gesture, calling back with its
    /// amount, phase and whether it is over. Its bounds are -1 with a way
    /// forward and 1 with a way back, 0 on a side with neither, where
    /// AppKit damps the travel. A test's stands in for it.
    var track: (NSEvent, CGFloat, CGFloat, @escaping (CGFloat, NSEvent.Phase, Bool) -> Void) -> Void = { event, min, max, handler in
        event.trackSwipeEvent(options: .lockDirection, dampenAmountThresholdMin: min, max: max) { amount, phase, complete, _ in
            handler(amount, phase, complete)
        }
    }

    /// The way a scroll event starts a swipe, 1 back and -1 forward, or nil:
    /// a trackpad's gesture (phased, not its momentum), more sideways than
    /// not, with swipe tracking on in System Settings and somewhere to go
    /// that way. Back is toward the page before: content moving right, the
    /// direction as the person scrolls.
    static func way(gesture: Bool, dx: CGFloat, dy: CGFloat, enabled: Bool, back: Bool, forward: Bool) -> CGFloat? {
        guard gesture, enabled, abs(dx) > abs(dy) else { return nil }
        return dx > 0 && back ? 1 : dx < 0 && forward ? -1 : nil
    }

    /// A scroll event the page's scroll view passed up (it does for a
    /// sideways gesture it is pinned for, `wantsScrollEventsForSwipeTracking`).
    /// True when it began a swipe, which AppKit then tracks. A script's
    /// wheel is not a trackpad's stream of events, and is never tracked.
    func scroll(_ event: NSEvent, enabled: Bool = NSEvent.isSwipeTrackingFromScrollEventsEnabled && !ExactEnv.agentMode) -> Bool {
        guard !tracking, !Self.kept(event) else { return false }
        let navigation = presenter.navigation
        let (back, forward) = (navigation.canGoBack, navigation.canGoForward)
        guard Self.way(gesture: !event.phase.isEmpty && event.momentumPhase.isEmpty, dx: event.scrollingDeltaX, dy: event.scrollingDeltaY,
                       enabled: enabled, back: back, forward: forward) != nil else { return false }
        tracking = true
        // The side the gesture set out for: one that ends on the other side
        // (the fingers came back past where they started) goes nowhere.
        var side: CGFloat = 0
        track(event, forward ? -1 : 0, back ? 1 : 0) { [weak self] amount, phase, complete in
            if side == 0, amount != 0 { side = amount < 0 ? -1 : 1 }
            if phase == .ended, amount * side > 0 {
                if side > 0 { self?.presenter.navigation.goBack() } else { self?.presenter.navigation.goForward() }
            }
            if complete { self?.tracking = false }
        }
        return true
    }

    /// Whether a scroller under the event keeps a sideways gesture
    /// (`overscroll-behavior-x: contain` or `none`). `NSScrollView` hands
    /// one it is pinned for to the view that tracks swipes even so.
    static func kept(_ event: NSEvent) -> Bool {
        guard let window = event.window, let frame = window.contentView?.superview ?? window.contentView,
              let hit = frame.hitTest(frame.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow) else { return false }
        return sequence(first: hit, next: \.superview).contains { ($0 as? ChainingScrollView).map { $0.containX && $0.scrollsX } ?? false }
    }

    /// The three-finger swipe (`NSResponder.swipe(with:)`): `deltaX` is
    /// positive toward the page before, as AppKit reports it for the
    /// person's scroll direction.
    func swiped(deltaX: CGFloat) -> Bool {
        deltaX > 0 ? presenter.navigation.goBack() : deltaX < 0 && presenter.navigation.goForward()
    }
}
#endif
