// The rules UIKit's navigation, sheet, keyboard and focus projection
// follows (LLP 1035.001 D1–D5, D8), as pure functions of what the
// presenter knows — no controller, no window, no run loop, no clock — so
// the macOS test target can hold every one of them and a reader finds the
// contract in one place. `NavigationIOS.swift`, `ModalIOS.swift` and
// `PresenterIOS.swift` apply them; nothing here decides what UIKit owns
// (recognition, progress, cancellation, animation).
//
// @ref LLP 1035.001 §3; LLP 1008 §9
import CExact
import Foundation

enum NavigationRules {
    /// @ref LLP 1038 D6 — controller identities, not URLs: animate a
    /// push/pop only when either complete list is a prefix of the other.
    static func isPushOrPop<T: Equatable>(from: [T], to: [T]) -> Bool {
        from.starts(with: to) || to.starts(with: from)
    }

    /// D1: the routes are the first root's children carrying a
    /// `navigationKey`, in tree order, and the stack is the prefix through
    /// the one whose key is the root's. `nil` when the root's key matches
    /// no route: the stack is left alone and the refusal is journaled.
    static func stack(routeKeys: [String], selected: String) -> Range<Int>? {
        guard let index = routeKeys.firstIndex(of: selected) else { return nil }
        return 0..<(index + 1)
    }

    /// D1, when the root's key matches no route: the stack is left alone,
    /// less the routes that have left the tree. The deepest route of the
    /// stack last shown (`shown`, its keys through its selected route) that
    /// is still declared in one of the stacks is the selected one, so a
    /// closed sheet or a popped screen never leaves the route beneath it
    /// covered. `nil` when none is (nothing shown yet): nothing changes.
    static func survivor(of shown: [String], declared: [[String]]) -> String? {
        shown.last { key in declared.contains { $0.contains(key) } }
    }

    /// D4: presentation boundaries split the selected prefix into retained
    /// owners. A later push stays in the most recent presentation's stack.
    static func segments(presentations: [String?]) -> [Range<Int>] {
        var starts = [0]
        for (index, kind) in presentations.enumerated() where kind == "modal" || kind == "fullscreen" {
            starts.append(index)
        }
        return zip(starts, starts.dropFirst() + [presentations.count]).map { $0..<$1 }
    }

    /// D2: a completed transition presses the Back control exactly once —
    /// only for a native pop whose live source is still selected. A swipe
    /// captures its source; a Back-button pop removes it from our last write.
    /// A stack change Exact writes has no departed source; its completion
    /// must not dismiss a newer route selected while UIKit was animating.
    /// A cancelled swipe shows the same key. Sheets have their own path.
    ///
    /// One exception to "the source is still selected": the app replaced the
    /// source in place while the finger was down (same depth, the source's
    /// node gone — a finished screen giving way to its result). The person
    /// swiped that position away, so the gesture applies to its replacement:
    /// otherwise the replacement is pushed back in the moment the pop lands.
    static func dispatchesBack(shownKey: String, rootKey: String, sourceKey: String?, sourceReplaced: Bool = false, modalActive: Bool) -> Bool {
        !modalActive && shownKey != rootKey && (sourceKey == rootKey || sourceReplaced)
    }

    /// Whether the app is behind a pop UIKit finished: the app's top is among the routes the pop took
    /// from what Exact last wrote to UIKit, and UIKit's top is beneath it in the app's stack. It holds
    /// whatever became of the source's node or key during the pop. A route the app pushed meanwhile
    /// was never written to UIKit, so the app keeps it; a route the pop took counts even when the app
    /// selected it again meanwhile, as the person saw it leave.
    static func appIsBehind(poppedKeys: [String], uikitTop: String, shownKeys: [String], rootKey: String) -> Bool {
        guard poppedKeys.contains(rootKey), let top = shownKeys.firstIndex(of: uikitTop), let root = shownKeys.firstIndex(of: rootKey) else { return false }
        return top < root
    }

    /// What a route shows of the platform's navigation bar.
    enum Bar: Equatable {
        /// The route's `header`, projected: its heading the title, its
        /// buttons the items, and the system Back when it is pushed.
        case header
        /// The system Back alone, with no title: a pushed route with no bar
        /// of its own, as a screen a hand-built app pushes with nothing set.
        case back
        /// No bar: UIKit's is hidden while the route is on top.
        case hidden
    }

    /// LLP 1115.000 D5.2, D5.3: the bar is decided route by route, never
    /// once for a stack, and Back is always there. A route that says
    /// `navigationBar="none"` has no bar (a full-bleed screen; the edge
    /// swipe still pops it): no bar is something the author says, never
    /// inferred from a missing header or an authored Back control. A route
    /// whose `header` is the bar's (`header`: its first child, shown,
    /// holding one heading) shows that. Any other pushed route shows the
    /// system Back alone, where the platform's bar has a Back button
    /// (`backButton`; tvOS's has none, its Back is the remote's Menu). The
    /// first route of a stack, with nothing to go back to and no such
    /// header, has nothing to show.
    /// The rule itself is the kernel's (`exact_kernel::Bar::of`), which
    /// the Android host reads too (LLP 1115.001 D11).
    static func bar(asked: String?, header: Bool, pushed: Bool, backButton: Bool = true) -> Bar {
        switch exact_route_bar((asked == "none" ? 1 : 0) | (header ? 2 : 0) | (pushed ? 4 : 0) | (backButton ? 8 : 0)) {
        case 0: return .header
        case 1: return .back
        default: return .hidden
        }
    }

    /// D1: whether an interactive pop may begin at all — a stack to pop, no
    /// transition in flight, no sheet, a way back (`back`), and no
    /// context preview anywhere in the session.
    static func popMayBegin(depth: Int, changing: Bool, modalActive: Bool, hasBackControl: Bool, contextPreviewActive: Bool) -> Bool {
        depth > 1 && !changing && !modalActive && hasBackControl && !contextPreviewActive
    }

    /// D1's arbitration for a pan: past the 20-point edge a pan that starts
    /// over a `swiperight` node is that node's; otherwise the pan must be
    /// more horizontal than vertical. The edge itself stays navigation's.
    /// No motion yet is no evidence against: UIKit's pop recognizers ask
    /// before the finger has moved (a real swipe read zero velocity and zero
    /// travel there), having judged the direction themselves.
    /// The direction a pop's `shouldBegin` judges: the pan's velocity, or
    /// its travel so far when the velocity is still zero (LLP 1057.001 §7).
    static func popDirection(velocity: CGPoint, travel: CGPoint) -> CGPoint {
        velocity == .zero ? travel : velocity
    }

    static func panMayBegin(startX: CGFloat, overSwipeRight: Bool, velocity: CGPoint) -> Bool {
        if startX >= 20 && overSwipeRight { return false }
        if velocity == .zero { return true }
        return velocity.x > abs(velocity.y)
    }

    /// D1: `closedby="none"` refuses the platform's dismissal gesture and
    /// Escape; anything else (`closerequest`, absent) permits them. Explicit
    /// Close is the app's own control either way.
    static func modalRefusesDismissal(closedby: String?) -> Bool {
        closedby == "none"
    }

    /// D3 (as it stands): why a `focus(id)` command cannot be delivered now,
    /// or `nil` when it can. The reason is journaled, never silent.
    static func focusRefusal(mounted: Bool, disabled: Bool, zeroSize: Bool, hiddenAncestor: Bool, inertAncestor: Bool) -> String? {
        if !mounted { return "not mounted" }
        if disabled { return "disabled" }
        if zeroSize { return "zero size" }
        if hiddenAncestor { return "hidden ancestor" }
        if inertAncestor { return "inert ancestor" }
        return nil
    }
}
