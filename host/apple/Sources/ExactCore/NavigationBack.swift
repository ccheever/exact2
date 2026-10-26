// The platform's Back from a navigation root's selected route (LLP 1035.001
// D1, LLP 1115 D5), as pure functions every host applies: UIKit's stack, the
// Mac's toolbar and the watch's `NavigationStack` (LLP 1126.002 D5).
import Foundation

package enum NavigationBack {
    /// D1: the Back control is resolved at use, never captured at a
    /// gesture's start: the lowest view id among live controls whose HTML
    /// `id` is the container's `navigationBack`, that handle `press`, and are
    /// owned by the active route, else by no route at all (a header the
    /// routes share, where an app keeps one Back for every screen); a
    /// control in another route is that route's. A disabled one resolves to
    /// nothing. `nil` when no such control is live — and then no gesture may
    /// begin and no dismissal may complete.
    package static func control<Control>(
        named target: String?,
        among controls: [Control],
        id: (Control) -> UInt32,
        htmlID: (Control) -> String?,
        pressable: (Control) -> Bool,
        disabled: (Control) -> Bool,
        inActiveRoute: (Control) -> Bool,
        inAnyRoute: (Control) -> Bool = { _ in true }
    ) -> Control? {
        guard let target else { return nil }
        let named = controls.filter { htmlID($0) == target && pressable($0) }
        let own = named.filter(inActiveRoute)
        let candidates = own.isEmpty ? named.filter { !inAnyRoute($0) } : own
        return candidates.filter { !disabled($0) }.min { id($0) < id($1) }
    }

    /// How the platform's Back (its button, edge swipe, sheet swipe-down)
    /// goes from the selected route.
    package enum Way: Equatable {
        /// Press the authored Back control, this view.
        case press(UInt32)
        /// Deliver this location to the navigation root's `navigate`.
        case navigate(String)
        /// The runner's own `back` of the router (no `navigate` handler).
        case pop
    }

    /// LLP 1115 D5, Back is always there: a route that declares a Back
    /// control (a live element in it whose `id` is the root's
    /// `navigationBack`) goes back by pressing it, and is refused while it
    /// does not resolve (disabled, or no `press`) — the author's say, as
    /// D1 has it. A route that declares none still goes back, as a
    /// hand-built UIKit screen does and as the web's history Back does
    /// (LLP 1038 D11, amended 2026-10-03): the root's `navigate` with the
    /// location of the visit beneath, or with no `navigate` handler the
    /// runner's own `back` of the router, the same on the web. With no
    /// visit beneath there is nothing to go back to, and it is refused.
    package static func way(control: UInt32?, declared: Bool, hearsNavigate: Bool, beneath: () -> String?) -> Way? {
        if let control { return .press(control) }
        guard !declared, let location = beneath(), !location.isEmpty else { return nil }
        return hearsNavigate ? .navigate(location) : .pop
    }
}
