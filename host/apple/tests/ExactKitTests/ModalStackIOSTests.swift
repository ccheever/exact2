#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A sheet over a sheet (LLP 1075.003 §9, LLP 1035.003 D5): the native
/// fixture's menu pushed over its Sheet route waits for Sheet's
/// presentation, and the agent's wait after an input waits with it.
final class ModalStackIOSTests: XCTestCase {
    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The native fixture booted in a window of the test host's scene: UIKit
    /// presents a sheet only from a window a scene holds.
    private func fixture(_ label: String) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func press(_ session: ExactSession, _ testId: String) throws {
        session.presenter.press(try node(session, testId).id)
    }

    /// The controllers presented over the fixture, bottom first.
    private func presented() -> [UIViewController] {
        Array(sequence(first: host?.presentedViewController, next: { $0?.presentedViewController }).compactMap { $0 })
    }

    /// Nothing presenting: the agent's wait after an input returns at once.
    func testTheWaitReturnsAtOnceWithNoSheetMoving() throws {
        let session = try fixture("sheet-wait-idle")
        let started = Date()
        XCTAssertTrue(session.agentInstance.awaitModalTransitions())
        XCTAssertLessThan(Date().timeIntervalSince(started), 0.1)
    }

    /// The menu pushed over Sheet while Sheet is still being presented (a
    /// drive's taps a few milliseconds apart, the Bluesky clone's prompt over
    /// its muted words sheet) waits: UIKit presents from the topmost
    /// controller only once it has finished presenting, so the second route
    /// is in the tree and not yet on screen, and the agent's wait after the
    /// input covers it. This window ends no presentation (see
    /// ModalDetentIOSTests), so the wait ends at its bound, saying so; the
    /// sheets presented in order, and closed, are driven by
    /// `scripts/smoke-native.mjs` on a simulator.
    func testASheetOverAPresentingSheetWaitsAndTheAgentWaitsWithIt() throws {
        let session = try fixture("sheet-over-presenting-sheet")
        let modals = session.presenter.modals
        try press(session, "sheet")
        XCTAssertEqual(presented().count, 1, "Sheet is being presented")
        XCTAssertTrue(modals.inTransition)
        try press(session, "sheet-menu")
        XCTAssertNotNil(try node(session, "menu-close"), "the menu's route is in the tree")
        XCTAssertEqual(presented().count, 1, "the menu waits while Sheet presents")
        XCTAssertEqual(modals.routes.map { $0.node.props["testId"] }, ["route-sheet"])
        let started = Date()
        XCTAssertFalse(session.agentInstance.awaitModalTransitions(bound: 0.3), "the wait covers Sheet still presenting")
        XCTAssertGreaterThanOrEqual(Date().timeIntervalSince(started), 0.3)
    }

    /// LLP 1035.001 D1: a root whose key names no route keeps what is left of
    /// the stack it showed. Chat-rooms (bake-off round 6) keyed its list route
    /// "rooms" while its root named the visit, `${top(nav).id}`, "0" at the
    /// stack's root: its new-room sheet opened, and Cancel (the sheet's route
    /// gone, the key "0" again) left the sheet presented over a blank screen and
    /// the list inactive beneath it. Now the list is selected again and live,
    /// and the sheet, its route gone, is owed its close. Batches as the
    /// runner writes them, to a session whose boot was refused: a view's
    /// first fit boots its session (a navigation transition's end fits it,
    /// in the batch that installs the stack), and here that would be the
    /// test host's baked plan over these batches. A refused boot boots
    /// nothing again (LLP 1031 D8).
    func testASheetWhoseRouteLeftWhileTheRootNamesNoRouteClosesAndLeavesItsBackgroundLive() throws {
        let session = ExactApp.shared.makeSession(label: "sheet-closed-root-names-none")
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
        self.host = host
        session.presenter.viewport.frame = host.view.bounds
        // A sheet waits for the first drawn frame, which this window never draws.
        session.firstDrawMs = ExactEnv.wall()
        let apply = { (ops: [[String: Any]]) in session.presenter.apply(wireBatch(ops)) }
        let frame = { (id: Int) -> [String: Any] in ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0] }
        apply([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": "0"]], frame(1),
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "rooms"]], frame(2),
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["testId": "room-1"]],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 402.0, "h": 44.0],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
        ])
        let navigation = session.presenter.navigation
        let rooms = try XCTUnwrap(session.presenter.views[2]), row = try XCTUnwrap(session.presenter.views[3])
        XCTAssertFalse(navigation.isInactiveRoute(containing: row))
        // Open the sheet: the root names its visit, which its route carries.
        apply([
            ["op": "create", "id": 4, "kind": "view", "props": ["navigationKey": "1", "navigationPresentation": "modal"]], frame(4),
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["id": "back", "testId": "room-cancel"]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 80.0, "h": 44.0],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "props", "id": 1, "set": ["navigationKey": "1"]],
        ])
        until("the new-room sheet is presented") { presented().count == 1 }
        XCTAssertEqual(navigation.selectedKey, "1")
        XCTAssertTrue(navigation.isInactiveRoute(containing: row), "the list is inert beneath the sheet")
        // Cancel: the sheet's route leaves, and the root names visit 0 again, which no route carries.
        apply([
            ["op": "children", "id": 4, "ids": []],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "destroy", "id": 5],
            ["op": "destroy", "id": 4],
            ["op": "props", "id": 1, "set": ["navigationKey": "0"]],
        ])
        XCTAssertEqual(navigation.selectedKey, "rooms", "the route left of the stack shown is selected")
        XCTAssertFalse(navigation.isInactiveRoute(containing: row), "the list is live again")
        XCTAssertTrue((navigation.primaryNavigation?.viewControllers.first as? RouteController)?.node === rooms)
        // This window finishes no presentation (ModalDetentIOSTests), so the sheet's close waits for it here,
        // owed; on a simulator the sheet goes and the list shows (`agent ios`: Cancel, then tap a room).
        XCTAssertTrue(navigation.syncOwed, "the sheet's close is owed")
    }
}
#endif
