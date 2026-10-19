#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 §3.7 (HeldStackIOS.swift): a tab whose stack shows no bar
/// and has pushed nothing holds its route with no navigation controller, as
/// a hand-built UIKit app shows such a tab. The controller is built on the
/// stack's first push, when its route first shows a bar, and when a hatch
/// asks for it; the route moves into it unchanged and the push animates.
final class HeldStackIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []
    private var animationsWereEnabled = true

    override func setUp() {
        super.setUp()
        animationsWereEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
    }

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        UIView.setAnimationsEnabled(animationsWereEnabled)
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func fixture(_ label: String, module: Bool = false) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_HELD_TAB_PLAN"], "build.mjs --test --ios compiles the held-tab fixture")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: frame)
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: frame.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        if module {
            session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"], "build.mjs --test --ios builds the fixture's module"))
        }
        until("the tabs are installed") { session.presenter.navigation.tabOwner != nil }
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

    /// The Plain tab's held stack and its route, as installed.
    private func held(_ session: ExactSession) throws -> (tabs: UITabBarController, held: HeldStack, root: RouteController) {
        let navigation = session.presenter.navigation
        let tabs = try XCTUnwrap(navigation.tabController)
        let held = try XCTUnwrap(tabs.viewControllers?.first as? HeldStack, "Plain shows no bar and has pushed nothing")
        let root = try XCTUnwrap(held.route)
        return (tabs, held, root)
    }

    func testATabThatShowsNoBarAndPushesNothingHasNoNavigationController() throws {
        let session = try fixture("held")
        let navigation = session.presenter.navigation
        let (tabs, held, root) = try held(session)
        XCTAssertNil(navigation.tabNavigations[navigation.tabPanels[0]])
        XCTAssertNil(navigation.primaryNavigation)
        XCTAssertTrue(navigation.primaryHeld === held)
        XCTAssertTrue(navigation.activeRoute === root)
        XCTAssertTrue(tabs.selectedViewController === held)
        XCTAssertTrue(tabs.viewControllers?[1] is UINavigationController, "a tab not yet built has one, out of the window")
        XCTAssertNotNil(root.view.window)
        XCTAssertEqual(root.view.frame, held.view.bounds)
        XCTAssertGreaterThan(root.view.safeAreaInsets.bottom, 0, "the route's safe area clears the tab bar")
        XCTAssertEqual(navigation.observation()["stack"] as? [String], [root.key])
        // The tab bar follows the route's content scroll view through its tab's controller.
        XCTAssertTrue(held.contentScrollView(for: .bottom) === root.contentScrollView(for: .bottom))
        // The other tab, first selected, starts with a bar: it is built as it fills.
        try press(session, "tab-titled")
        until("Titled is selected") { tabs.selectedIndex == 1 }
        let titled = try XCTUnwrap(tabs.viewControllers?[1] as? UINavigationController)
        XCTAssertFalse(titled.isNavigationBarHidden)
        XCTAssertTrue(navigation.primaryNavigation === titled)
        XCTAssertTrue(tabs.viewControllers?[0] === held, "Plain stays held")
    }

    func testTheFirstPushBuildsItWithTheRootAndAnimates() throws {
        let session = try fixture("held-push")
        let navigation = session.presenter.navigation
        let (tabs, _, root) = try held(session)
        let frame = root.view.convert(root.view.bounds, to: nil)
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try press(session, "open")
        until("Plain has its navigation controller") { tabs.viewControllers?.first is UINavigationController }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        XCTAssertNotNil(nav.transitionCoordinator, "the push animates")
        XCTAssertTrue(nav.viewControllers.first === root, "the root moved in")
        XCTAssertEqual(nav.viewControllers.count, 2)
        XCTAssertEqual(tabs.selectedIndex, 0)
        XCTAssertTrue(navigation.primaryNavigation === nav)
        XCTAssertNil(navigation.heldTabs[navigation.tabPanels[0]])
        XCTAssertEqual(navigation.tabStack(navigation.tabPanels[0]), nav)
        until("the push is over") { nav.transitionCoordinator == nil && !navigation.changing }
        XCTAssertEqual(nav.viewControllers.count, 2, "Detail is on top")
        XCTAssertTrue(navigation.primaryNavigation === nav)
        XCTAssertEqual(navigation.observation()["route"] as? String, (nav.viewControllers.last as? RouteController)?.key)
        XCTAssertEqual(navigation.observation()["stack"] as? [String], nav.viewControllers.compactMap { ($0 as? RouteController)?.key })
        XCTAssertEqual(navigation.agentBack(), "", "the platform's Back goes")
        until("popped to the root") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil && !navigation.changing }
        XCTAssertTrue(tabs.viewControllers?.first === nav, "the stack keeps its navigation controller")
        XCTAssertTrue(nav.isNavigationBarHidden)
        XCTAssertEqual(root.view.convert(root.view.bounds, to: nil), frame, "the root is where it was")
    }

    func testABarBecomingVisibleBuildsIt() throws {
        let session = try fixture("held-title")
        let navigation = session.presenter.navigation
        let (tabs, _, root) = try held(session)
        try press(session, "title")
        until("Plain has its navigation controller") { tabs.viewControllers?.first is UINavigationController }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        XCTAssertTrue(nav.viewControllers == [root])
        until("the bar shows") { !nav.isNavigationBarHidden }
        XCTAssertEqual(root.navigationItem.title, "plain")
        XCTAssertTrue(navigation.primaryNavigation === nav)
    }

    func testASheetLeavesItHeld() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet presents only from a scene's window") }
        let session = try fixture("held-sheet")
        let navigation = session.presenter.navigation
        let (tabs, held, root) = try held(session)
        try press(session, "open-sheet")
        until("the sheet is up") { navigation.presentedNavigations.count == 1 && !navigation.inTransition }
        XCTAssertTrue(tabs.viewControllers?.first === held)
        XCTAssertTrue(held.route === root)
        try press(session, "close")
        until("the sheet is down") { navigation.presentedNavigations.isEmpty && !navigation.inTransition }
        XCTAssertTrue(tabs.viewControllers?.first === held)
        XCTAssertNotNil(root.view.window)
    }

    func testAHatchAskingForItBuildsIt() throws {
        let session = try fixture("held-hatch")
        let navigation = session.presenter.navigation
        let (tabs, held, root) = try held(session)
        let item = held.tabBarItem
        let field = try XCTUnwrap(try node(session, "draft").field)
        XCTAssertTrue(field.becomeFirstResponder())
        let nav = try XCTUnwrap(navigation.navigation(forHatch: held))
        XCTAssertTrue(field.isFirstResponder, "the route stayed in the window as it moved")
        XCTAssertTrue(tabs.viewControllers?.first === nav)
        XCTAssertTrue(nav.viewControllers == [root])
        XCTAssertTrue(nav.tabBarItem === item, "the tab keeps its item")
        XCTAssertTrue(nav.isNavigationBarHidden)
        XCTAssertEqual(tabs.selectedIndex, 0)
        XCTAssertNotNil(root.view.window)
        XCTAssertTrue(navigation.navigation(forHatch: nav) === nav)
        XCTAssertTrue(navigation.navigation(forHatch: held) === nav, "a hatch that kept the tab's handle reads the same controller again")
    }

    /// The fixture module's own container reads every tab's `controller`
    /// (`ExactTab`): through the host's table, each held tab is built.
    func testTheAppsContainerReadingATabsControllerBuildsIt() throws {
        setenv("EXACT_FIXTURE_CONTAINER", "app", 1)
        defer { unsetenv("EXACT_FIXTURE_CONTAINER"); NativeViews.uninstallTable() }
        let session = try fixture("held-owned", module: true)
        let navigation = session.presenter.navigation
        until("the app's container holds the tabs") { navigation.tabController == nil && navigation.tabOwner != nil }
        XCTAssertTrue(navigation.heldTabs.isEmpty)
        let plain = try XCTUnwrap(navigation.tabNavigations[navigation.tabPanels[0]])
        XCTAssertTrue(plain.parent === navigation.tabOwner)
        XCTAssertEqual(plain.viewControllers.compactMap { ($0 as? RouteController)?.node.props["testId"] }, ["route-plain"])
    }
}
#endif
