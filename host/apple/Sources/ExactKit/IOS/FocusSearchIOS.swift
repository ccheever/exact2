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
// 1k pt/s — and found nothing each time. So exact2's containers answer the
// search with no items unless a view UIKit can focus may be in the window:
// anything exact2 did not make itself (a field, a text area, a web view, a
// segmented control, a menu's button, a swipe's table, a platform view).
// Then the search is UIKit's own, as before.
#if os(iOS)
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
            || view is UIImageView || view is UIVisualEffectView || view.next is UINavigationController { return }
        candidates.add(view)
    }

    /// The child focus items `container` gives UIKit's focus search.
    static func items(_ container: UIView, _ own: () -> [any UIFocusItem]) -> [any UIFocusItem] {
        guard let window = container.window, candidates.allObjects.contains(where: { $0.window === window }) else { return [] }
        return own()
    }
}
#endif
