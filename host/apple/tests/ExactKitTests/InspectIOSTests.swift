#if os(iOS) && EXACT_INSPECT
import XCTest
@testable import ExactKit
import UIKit

/// Inspect mode's hit test (LLP 1122 D5, T2c; InspectIOS.swift): a tap
/// selects the nearest view, walking up from the window's own hit view, that
/// stands for a plan node, and the walk stops at a system bar. A plan node
/// the bar hosts (the drawn title) is selected; UIKit's own parts (Back, the
/// bar's material) select nothing, and say so.
final class InspectIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        Inspect.set(false, seq: nil)
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The native fixture's plan, booted on Home.
    private func home() throws -> (ExactSession, UIWindow) {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixtures' plans")))
        let session = ExactApp.shared.makeSession(label: "inspect")
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = window.screen.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        return (session, window)
    }

    /// The native fixture's plan, Chat pushed on Home's stack.
    private func chat() throws -> (ExactSession, UIWindow, UINavigationController, RouteController) {
        let (session, window) = try home()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let chat = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "chat" })
        _ = Agent(session: session).tap(["id": Int(chat.id)])
        until("Chat pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        spin(0.3)
        return (session, window, nav, try XCTUnwrap(nav.topViewController as? RouteController))
    }

    func testATapSelectsTheNodeItLandsOnAndTheBarsDrawnTitleButNotUIKitsBack() throws {
        let (session, window, nav, route) = try chat()
        // Content: the node under the point, the nearest that stands for a plan node.
        // (One whose middle the window's hit test gives to it, or to a part of
        // it that stands for no other node.)
        let content = try XCTUnwrap(session.presenter.views.values.first { node in
            guard node.window === window, !node.isHidden, node.bounds.width > 20, node.bounds.height > 20, !node.isDescendant(of: nav.navigationBar) else { return false }
            let box = node.convert(node.bounds, to: window)
            var hit = window.hitTest(CGPoint(x: box.midX, y: box.midY), with: nil)
            while let v = hit, !(v is NodeView) { hit = v.superview }
            return hit === node
        })
        let box = content.convert(content.bounds, to: window)
        let landed = InspectHost.target(at: CGPoint(x: box.midX, y: box.midY), in: window, ExactApp.shared)
        XCTAssertEqual(landed.id, content.id, "\(landed)")
        // The drawn title in the bar: the heading's group, a plan node the bar hosts.
        let title = try XCTUnwrap(route.navigationItem.titleView as? HeaderTitleView)
        let titleBox = title.convert(title.bounds, to: window)
        let picked = InspectHost.target(at: CGPoint(x: titleBox.midX, y: titleBox.midY), in: window, ExactApp.shared)
        XCTAssertNotNil(picked.id, "the bar's drawn title is the app's: \(picked)")
        XCTAssertEqual(picked.id, title.groupId)
        // Back, UIKit's own: nothing, said, and the walk does not climb out of the bar.
        let bar = nav.navigationBar.convert(nav.navigationBar.bounds, to: window)
        let back = InspectHost.target(at: CGPoint(x: bar.minX + 22, y: bar.midY), in: window, ExactApp.shared)
        XCTAssertNil(back.id, "Back is UIKit's, not a view of the app: \(back)")
        XCTAssertEqual(back.why, "That is part of UIKit's navigation bar, not a view of the app.")
    }

    func testATapOnTheTabBarSelectsTheTabUnderItAndItsEmptySpaceNothing() throws {
        let (session, window) = try home()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        let bar = tabs.tabBar, root = try XCTUnwrap(session.presenter.navigation.container)
        let tablist = try XCTUnwrap(NavigationTabs.of(root, session.presenter))
        // Each item's button (UIKit's, in order; no accessibility client runs here to make elements).
        var buttons: [CGRect] = [], pending: [UIView] = bar.subviews
        while !pending.isEmpty { let v = pending.removeFirst(); if v is UIControl { let b = v.convert(v.bounds, to: window).integral; if !buttons.contains(b) { buttons.append(b) } } else { pending += v.subviews } }
        buttons.sort { $0.minX < $1.minX }
        XCTAssertEqual(buttons.count, 2, "the fixture's two tabs show in UIKit's tab bar")
        let items = zip(tablist.tabs, buttons).map { (id: $0.id, box: $1) }
        let second = try XCTUnwrap(items.last)
        let picked = InspectHost.target(at: CGPoint(x: second.box.midX, y: second.box.midY), in: window, ExactApp.shared)
        XCTAssertEqual(picked.id, second.id, "\(picked)")
        // The bar's own space, on no item: nothing, said.
        let box = bar.convert(bar.bounds, to: window)
        var empty: CGPoint?
        for y in stride(from: box.minY + 2, to: box.maxY, by: 4) { for x in stride(from: box.minX + 2, to: box.maxX, by: 4) {
            let p = CGPoint(x: x, y: y)
            guard empty == nil, !items.contains(where: { $0.box.contains(p) }), let hit = window.hitTest(p, with: nil), hit.isDescendant(of: bar) else { continue }
            empty = p
        } }
        let space = try XCTUnwrap(empty, "the tab bar has space outside its items")
        let none = InspectHost.target(at: space, in: window, ExactApp.shared)
        XCTAssertNil(none.id, "\(none)")
        XCTAssertEqual(none.why, "That is part of UIKit's tab bar, not a view of the app.")
    }

    func testInspectModeEndsEditingAndTheStatusBarsScrollToTopTillItEnds() throws {
        let (session, window, _, route) = try chat()
        let field = UITextField(frame: CGRect(x: 20, y: 300, width: 200, height: 40))
        route.view.addSubview(field)
        defer { field.removeFromSuperview() }
        XCTAssertTrue(field.becomeFirstResponder())
        func toTop() -> [UIScrollView] {
            var found: [UIScrollView] = [], pending: [UIView] = [window]
            while !pending.isEmpty { let v = pending.removeFirst(); if let s = v as? UIScrollView, s.scrollsToTop, !(s is UITextView), !s.isDescendant(of: field) { found.append(s) }; pending += v.subviews }
            return found
        }
        let before = toTop()
        XCTAssertEqual(before.count, 1, "the route's content scroller takes the status-bar tap")
        Inspect.set(true, seq: 1)
        InspectHost.show(ExactApp.shared, on: true)
        XCTAssertFalse(field.isFirstResponder, "the keyboard goes: no typing reaches the app")
        XCTAssertEqual(toTop(), [], "a status-bar tap scrolls nothing")
        session.presenter.syncScrollsToTop() // (as after any batch)
        XCTAssertEqual(toTop(), [])
        Inspect.set(false, seq: 1)
        InspectHost.show(ExactApp.shared, on: false)
        XCTAssertEqual(toTop(), before)
    }

    func testInspectSelectionsReportTheRunnersNodeWithAClickTokenOfTheSession() throws {
        let (session, window, _, _) = try chat()
        Inspect.set(true, seq: 3)
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.window === window && !$0.isHidden && $0.props["testId"] != nil })
        let body = try XCTUnwrap(Inspect.select(session, id: node.id, clicked: true))
        XCTAssertEqual(body["kind"] as? String, "selected")
        XCTAssertEqual(body["inspect"] as? Int, 3)
        let selected = try XCTUnwrap(body["selected"] as? [String: Any])
        XCTAssertEqual(selected["node"] as? Int, Int(node.id))
        XCTAssertNotNil(selected["planDigest"] as? String, "the runner's own node, with its plan's digest")
        XCTAssertFalse((selected["rows"] as? [Any] ?? []).isEmpty)
        XCTAssertEqual(selected["planDigest"] as? String, BuildChannel.planDigest(ExactApp.shared), "status and selections name the plan from one source, the runner")
        let click = try XCTUnwrap(body["click"] as? String)
        XCTAssertTrue(Inspect.answersClick(["click": click]), "the outline answering it scrolls nothing")
        XCTAssertFalse(Inspect.answersClick(["click": "elsewhere.1"]))
        // Off: the session's tokens end with it.
        Inspect.set(false, seq: 3)
        XCTAssertFalse(Inspect.answersClick(["click": click]))
        XCTAssertNil(Inspect.select(session, id: node.id, clicked: true), "no selection outside inspect mode")
    }
}
#endif
