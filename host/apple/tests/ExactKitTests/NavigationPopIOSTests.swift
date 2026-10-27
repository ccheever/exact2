#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The end of a pop UIKit runs (`NavigationHost.completeTransition`): the app
/// follows a pop UIKit finished, whatever became of the source's key or of a
/// route the app selected again meanwhile, and a cancelled pop dispatches no
/// Back. The tree is the test's own, written to the presenter as the runner
/// writes it; a Back is the press of the route's `navigationBack` control,
/// which the test answers as the app would, in the same turn.
final class NavigationPopIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    /// UIKit animates a pop only in a window a scene holds.
    private var hasScene: Bool { UIApplication.shared.connectedScenes.contains { $0 is UIWindowScene } }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private struct Stack {
        let session: ExactSession
        let nav: UINavigationController
        let apply: ([[String: Any]]) -> Void
        var key: String? { (nav.topViewController as? RouteController)?.key }
    }

    /// Settings (2), Updates (3, its Back 4) and Log (5, its Back 6) under
    /// the root (1), Log selected, and UIKit's stack written from them.
    private func stack(_ label: String) throws -> Stack {
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        XCTAssertNotNil(session.boot(plan: Data(), size: CGSize(width: 402, height: 874)).error, "no plan in no bytes")
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.frame = host.view.bounds
        self.window = window
        session.presenter.viewport.frame = host.view.bounds
        let apply = { (ops: [[String: Any]]) in session.presenter.apply(wireBatch(ops)) }
        apply([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": "log"]], frame(1),
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "settings"]], frame(2),
            ["op": "create", "id": 3, "kind": "view", "props": ["navigationKey": "updates"]], frame(3),
            ["op": "create", "id": 5, "kind": "view", "props": ["navigationKey": "log"]], frame(5),
        ] + back(4, in: 3) + back(6, in: 5) + [
            ["op": "children", "id": 1, "ids": [2, 3, 5]],
            ["op": "roots", "ids": [1]],
        ])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("UIKit holds the three routes") { nav.viewControllers.count == 3 && nav.transitionCoordinator == nil }
        return Stack(session: session, nav: nav, apply: apply)
    }

    private func frame(_ id: Int) -> [String: Any] { ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0] }

    private func back(_ id: Int, in route: Int) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": ["id": "back"]],
         ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 80.0, "h": 44.0],
         ["op": "children", "id": route, "ids": [id]]]
    }

    /// An interactive pop of the top route, its animation started. Once the
    /// test finishes or cancels it, the stack Exact writes next animates on
    /// its own (`interactive` false), not under this interaction.
    private func swipe(_ s: Stack) -> InteractivePop {
        let pop = InteractivePop()
        s.session.presenter.navigation.setAppDelegate(s.nav, pop)
        pop.interactive = true
        let destination = s.nav.viewControllers[s.nav.viewControllers.count - 2]
        s.nav.popViewController(animated: true)
        until("UIKit's pop animates the route beneath") { destination.viewIfLoaded?.window != nil }
        pop.interaction.update(0.4)
        return pop
    }

    /// Log's key moves while the finger is down (a route that redirects as
    /// it loads): the source rule no longer names it, and the app still goes
    /// back once UIKit's pop is over.
    func testAFinishedPopTakesTheAppBackAfterTheSourcesKeyMoved() throws {
        guard hasScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let s = try stack("pop-key-moved")
        var pressed: [UInt32] = []
        s.session.presenter.onPress = { id in
            pressed.append(id)
            s.apply([["op": "children", "id": 1, "ids": [2, 3]], ["op": "destroy", "id": 6], ["op": "destroy", "id": 5],
                     ["op": "props", "id": 1, "set": ["navigationKey": "updates"]]])
        }
        let pop = swipe(s)
        s.apply([["op": "props", "id": 5, "set": ["navigationKey": "log-2"]], ["op": "props", "id": 1, "set": ["navigationKey": "log-2"]]])
        pop.interaction.finish()
        pop.interactive = false
        until("the pop lands on Updates") { s.nav.transitionCoordinator == nil && s.nav.viewControllers.count == 2 && s.key == "updates" }
        XCTAssertEqual(pressed, [6], "Log's Back, once")
        XCTAssertEqual(s.session.presenter.navigation.selectedKey, "updates")
        s.session.presenter.navigation.setAppDelegate(s.nav, nil)
    }

    /// The app writes Log anew, under the same key, during a pop UIKit's
    /// back button started: the source's node is gone, and the app still goes
    /// back once UIKit's pop is over, through the new Log's Back.
    func testAFinishedBackButtonPopTakesTheAppBackAfterTheSourceWasWrittenAnew() throws {
        guard hasScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let s = try stack("pop-source-anew")
        var pressed: [UInt32] = []
        s.session.presenter.onPress = { id in
            pressed.append(id)
            s.apply([["op": "children", "id": 1, "ids": [2, 3]], ["op": "destroy", "id": 12], ["op": "destroy", "id": 11],
                     ["op": "props", "id": 1, "set": ["navigationKey": "updates"]]])
        }
        s.nav.popViewController(animated: true)
        XCTAssertNotNil(s.nav.transitionCoordinator, "the app writes Log anew before UIKit's pop is over")
        s.apply([["op": "create", "id": 11, "kind": "view", "props": ["navigationKey": "log"]], frame(11)] + back(12, in: 11) + [
            ["op": "children", "id": 1, "ids": [2, 3, 11]], ["op": "destroy", "id": 6], ["op": "destroy", "id": 5],
        ])
        until("the pop lands on Updates") { s.nav.transitionCoordinator == nil && s.nav.viewControllers.count == 2 && s.key == "updates" }
        XCTAssertEqual(pressed, [12], "the new Log's Back, once")
        XCTAssertEqual(s.session.presenter.navigation.selectedKey, "updates")
    }

    /// The app puts Draft under Log while the finger is down: the swipe took
    /// Log alone, so the app goes back through Log's own Back, to Draft, not
    /// by a history jump past Draft to the Updates UIKit shows.
    func testASwipeGoesBackThroughTheSourcesBackAfterTheAppInsertedARouteUnderIt() throws {
        guard hasScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let s = try stack("pop-inserted-under")
        var pressed: [UInt32] = []
        s.session.presenter.onPress = { id in
            pressed.append(id)
            s.apply([["op": "children", "id": 1, "ids": [2, 3, 9]], ["op": "destroy", "id": 6], ["op": "destroy", "id": 5],
                     ["op": "props", "id": 1, "set": ["navigationKey": "draft"]]])
        }
        let pop = swipe(s)
        s.apply([["op": "create", "id": 9, "kind": "view", "props": ["navigationKey": "draft"]], frame(9)] + back(10, in: 9) + [
            ["op": "children", "id": 1, "ids": [2, 3, 9, 5]],
        ])
        pop.interaction.finish()
        pop.interactive = false
        until("Draft shows over Updates") { s.nav.transitionCoordinator == nil && s.nav.viewControllers.count == 3 && s.key == "draft" }
        XCTAssertEqual(pressed, [6], "Log's Back, once")
        s.session.presenter.navigation.setAppDelegate(s.nav, nil)
    }

    /// The app replaces Log in place with Day while the finger is down, and
    /// the swipe is cancelled: UIKit popped nothing, so no Back reaches the
    /// app, and Day takes Log's place.
    func testACancelledPopDispatchesNoBackAfterTheSourceWasReplaced() throws {
        guard hasScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let s = try stack("pop-cancelled-replaced")
        var pressed: [UInt32] = []
        s.session.presenter.onPress = { pressed.append($0) }
        let pop = swipe(s)
        s.apply([["op": "create", "id": 7, "kind": "view", "props": ["navigationKey": "day"]], frame(7)] + back(8, in: 7) + [
            ["op": "children", "id": 1, "ids": [2, 3, 7]], ["op": "destroy", "id": 6], ["op": "destroy", "id": 5],
            ["op": "props", "id": 1, "set": ["navigationKey": "day"]],
        ])
        pop.interaction.cancel()
        pop.interactive = false
        until("Day takes Log's place") { s.nav.transitionCoordinator == nil && s.nav.viewControllers.count == 3 && s.key == "day" }
        XCTAssertEqual(pressed, [], "a cancelled pop performed no Back")
        s.session.presenter.navigation.setAppDelegate(s.nav, nil)
    }

    /// UIKit pops to Settings while the app selects Updates again, a route
    /// it retained: the pop took Updates, so the app goes back from it
    /// rather than UIKit pushing it again.
    func testAPopToTheRootTakesARouteTheAppSelectedAgainMeanwhile() throws {
        guard hasScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let s = try stack("pop-retained-selected")
        var pressed: [UInt32] = []
        s.session.presenter.onPress = { id in
            pressed.append(id)
            s.apply([["op": "props", "id": 1, "set": ["navigationKey": "settings"]]])
        }
        s.nav.popToRootViewController(animated: true)
        XCTAssertNotNil(s.nav.transitionCoordinator, "the app selects Updates before UIKit's pop is over")
        s.apply([["op": "props", "id": 1, "set": ["navigationKey": "updates"]]])
        until("the pop lands on Settings") { s.nav.transitionCoordinator == nil && s.nav.viewControllers.count == 1 && s.key == "settings" }
        XCTAssertEqual(pressed, [4], "Updates' Back, once")
        XCTAssertEqual(s.session.presenter.navigation.selectedKey, "settings")
    }
}
#endif
