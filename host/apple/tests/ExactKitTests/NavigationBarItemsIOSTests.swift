#if os(iOS)
import ObjectiveC
import UIKit
import XCTest
@testable import ExactKit

/// What `setRightBarButtonItems(_:animated:)` was asked, in order: UIKit
/// shows no animation of the bar's items in its layers to test for.
private var trailingSets: [Bool] = []

extension UINavigationItem {
    /// Swapped with `setRightBarButtonItems(_:animated:)`, so this name calls UIKit's.
    @objc fileprivate func recordedRightBarButtonItems(_ items: [UIBarButtonItem]?, animated: Bool) {
        trailingSets.append(animated)
        recordedRightBarButtonItems(items, animated: animated)
    }
}

extension NavigationBarIOSTests {
    /// A press that changes the bar of a route that shows sets its items with
    /// UIKit's animation, as a hand-built bar's
    /// `setRightBarButtonItems(_:animated: true)` does, whether the bar held
    /// controls or only a readout.
    func testABarsItemsChangedOnARouteThatShowsAreSetWithUIKitsAnimation() throws {
        let session = try fixture("bar-items-animated", module: false, plan: "BAR_SHARE")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        nav.view.layoutIfNeeded()
        let uikit = try XCTUnwrap(class_getInstanceMethod(UINavigationItem.self, #selector(UINavigationItem.setRightBarButtonItems(_:animated:))))
        let recorded = try XCTUnwrap(class_getInstanceMethod(UINavigationItem.self, #selector(UINavigationItem.recordedRightBarButtonItems(_:animated:))))
        method_exchangeImplementations(uikit, recorded)
        defer { method_exchangeImplementations(uikit, recorded) }
        trailingSets = []
        let items = { nav.topViewController?.navigationItem.rightBarButtonItems ?? [] }
        let more = try XCTUnwrap(items().first { $0.accessibilityIdentifier == "more-home" })
        // UIKit's send; a test process with no connected scene (a parallel worker's bare xctest) refuses it (false), so
        // there the item's own target and action, which UIKit's send calls
        if !UIApplication.shared.sendAction(try XCTUnwrap(more.action), to: more.target, from: more, for: nil) {
            _ = (more.target as AnyObject?)?.perform(more.action)
        }
        until("the press adds its control") { items().contains { $0.accessibilityIdentifier == "star-home" } }
        XCTAssertEqual(trailingSets, [true], "the items are set once, animated")
        // A bar left only a readout, then given its controls back: each change animates
        let toggle = try node(session, "bare-toggle").id
        trailingSets = []
        session.presenter.onPress?(toggle)
        until("the controls go, the readout stays") { !items().contains { $0.accessibilityIdentifier == "share-home" } }
        session.presenter.onPress?(toggle)
        until("the controls come back") { items().contains { $0.accessibilityIdentifier == "share-home" } }
        XCTAssertEqual(trailingSets, [true, true], "the readout-only bar's changes animate too")
    }
}
#endif
