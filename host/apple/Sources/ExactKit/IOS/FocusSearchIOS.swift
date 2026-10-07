// UIKit's focus search, answered for exact2's own views (LLP 1008 §9).
// On an iPad with a hardware keyboard UIKit runs a focus system. While no
// item holds UIKit focus — exact2's keyboard focus is the first responder's
// (Tab is its own key command) and none of its views is a UIKit focus item
// — every view that appears or hides asks for a focus update
// (`-[UIFocusSystem _focusEnvironmentDidAppear:]` and `…WillDisappear…`),
// and each update searches for a default item to defer focus to: a focus
// map of the window, every view's frame and eligibility. A list scrolling
// slowly built, parked and took back rows all the time and paid that
// search for each one — 13% of the main thread on an M1 iPad Pro at
// 1k pt/s, and one 20-40 ms frame a second once its throttle backed off —
// and found nothing each time. So the viewport, the one way into a
// session's views, answers the search with no items unless a view UIKit
// can focus may be in the window: anything exact2 did not make itself (a
// field, a text area, a web view, a segmented control, a menu's button, a
// swipe's table, a platform view). Then the search is given those views
// alone, each a focus item container of its own: a feed that always holds a
// field, a web view, a map or a player otherwise had UIKit walk every view
// of the session for each update — 30–40 ms a second of an iPad's main
// thread in a fast fling of the Extra Heavy feed — to reach the few it can
// focus. Only the viewport answers: UIKit logs, at every init, each view of
// a class that answers `focusItems(in:)`.
#if os(iOS) || os(tvOS)
import UIKit

enum FocusSearch {
    /// UIKit views placed in exact2's views, which may hold focus items.
    private static let candidates = NSHashTable<UIView>.weakObjects()

    /// `view` joins one of exact2's containers.
    static func joined(_ view: UIView) {
        // exact2's own views, an image and a material's backdrop are never
        // focus items; a route's navigation container holds exact2's nodes,
        // whose own foreign views join the nodes themselves.
        if view is NodeView || view is PlainView || view is ScrollView || view is MetalView
            || view is UIImageView || view is UIVisualEffectView || view.next is UINavigationController
            || view is NativeButtonIOS { return } // a native button's node keeps the focus (LLP 1069.011 D4)
        candidates.add(view)
    }

    /// The focus items `container` gives UIKit's focus search: the views
    /// UIKit may focus inside it, shown and in `rect`.
    fileprivate static func items(_ container: UIView, in rect: CGRect) -> [any UIFocusItem] {
        guard let window = container.window else { return [] }
        return candidates.allObjects.filter { view in
            guard view.window === window, view.isDescendant(of: container) else { return false }
            var v: UIView? = view
            while let current = v, current !== container { if current.isHidden || current.alpha < 0.01 { return false }; v = current.superview }
            return container.convert(view.bounds, from: view).intersects(rect)
        }
    }
}
/// A session's viewport (`Presenter.viewport`), which answers the search.
final class Viewport: ScrollView {
    // tvOS focuses exact2's own nodes (RemoteTVOS), so its search sees every view.
    #if !os(tvOS)
    override func focusItems(in rect: CGRect) -> [any UIFocusItem] { FocusSearch.items(self, in: rect) }
    #endif
}
#endif
