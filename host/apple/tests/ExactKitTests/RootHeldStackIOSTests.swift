#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class RootHeldStackIOSTests: XCTestCase {
    private final class Host: UIViewController {
        var appeared = false
        override func viewDidAppear(_ animated: Bool) { super.viewDidAppear(animated); appeared = true }
    }
    private var session: ExactSession?
    private var window: UIWindow?
    private var animations = true

    override func setUp() { animations = UIView.areAnimationsEnabled; UIView.setAnimationsEnabled(false) }
    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil
        UIView.setAnimationsEnabled(animations)
        super.tearDown()
    }
    private func until(_ message: String, _ predicate: () -> Bool) {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !predicate(), Date() < deadline { RunLoop.main.run(mode: .default, before: deadline) }
        XCTAssertTrue(predicate(), message)
    }
    private func fixture() throws -> ExactSession {
        let s = ExactApp.shared.makeSession(label: "held-root")
        session = s
        let view = ExactView(session: s), host = Host()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let w = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        w.frame = w.screen.bounds
        w.rootViewController = host; w.makeKeyAndVisible(); host.view.addSubview(view)
        window = w
        XCTAssertNil(s.boot(plan: try fixturePlan("HELD_ROOT"), size: w.bounds.size).error)
        view.frame = host.view.bounds; view.layoutIfNeeded()
        if scene != nil { until("the window appeared") { host.appeared } }
        if s.firstDrawMs == nil { s.firstDrawMs = ExactEnv.wall() }
        return s
    }
    private func node(_ s: ExactSession, _ id: String) throws -> NodeView {
        try XCTUnwrap(s.presenter.views.values.first { $0.props["testId"] == id && $0.window != nil }, id)
    }

    func testABarlessRootHoldsItsRouteAndRealizesOnItsFirstPush() throws {
        let s = try fixture(), n = s.presenter.navigation
        XCTAssertNil(n.primaryNavigation)
        let held = try XCTUnwrap(n.primaryOwner as? HeldStack)
        let route = try XCTUnwrap(held.route), frame = held.view.frame
        XCTAssertTrue(n.primaryHeld === held)
        XCTAssertTrue(n.activeRoute === route)
        XCTAssertEqual(n.primaryStacks, [held])
        XCTAssertNotNil(route.view.window)
        XCTAssertTrue(s.follow("/detail"))
        until("the root stack is realized") { n.primaryNavigation?.viewControllers.count == 2 && !n.inTransition }
        let nav = try XCTUnwrap(n.primaryNavigation)
        XCTAssertTrue(nav.viewControllers.first === route)
        XCTAssertEqual(nav.view.frame, frame)
        XCTAssertNil(held.parent)
        XCTAssertNil(held.view.superview)
        XCTAssertTrue(n.primaryOwner === nav)
    }

    func testShowingABarPreservesTheRootEditor() throws {
        let s = try fixture(), n = s.presenter.navigation
        XCTAssertNil(n.primaryNavigation)
        let field = try XCTUnwrap(try node(s, "field-start").field)
        XCTAssertTrue(field.becomeFirstResponder())
        s.presenter.press(try node(s, "title").id)
        until("the bar is shown") { n.primaryNavigation?.isNavigationBarHidden == false }
        XCTAssertTrue(field.isFirstResponder)
        XCTAssertEqual(field.text, "draft")
    }

    func testASheetLeavesTheRootHeld() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet needs a scene window") }
        let s = try fixture(), n = s.presenter.navigation
        let held = try XCTUnwrap(n.primaryOwner as? HeldStack)
        XCTAssertTrue(s.follow("/sheet"))
        until("the sheet is presented") { n.presentedNavigations.count == 1 && !n.inTransition }
        XCTAssertTrue(n.primaryOwner === held)
        XCTAssertNil(n.primaryNavigation)
        XCTAssertNotNil(held.route?.view.window)
    }
}
#endif
