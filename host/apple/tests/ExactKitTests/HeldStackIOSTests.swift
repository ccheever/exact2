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

    /// A warm deep link into a tab built earlier (Brooks): one update selects
    /// Plain, held, and pushes Detail onto it. Plain builds its navigation
    /// controller with Detail on top, which shows, not Plain's root.
    func testATabSelectedWithAPushOntoItsHeldStackShowsThePush() throws {
        let session = try fixture("held-select-push")
        let navigation = session.presenter.navigation
        let (tabs, _, root) = try held(session)
        try press(session, "tab-titled")
        until("Titled is selected") { tabs.selectedIndex == 1 && !navigation.inTransition }
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try press(session, "open-from-titled")
        until("Plain shows Detail") {
            let nav = tabs.viewControllers?.first as? UINavigationController
            return tabs.selectedIndex == 0 && nav?.viewControllers.count == 2 && nav?.transitionCoordinator == nil && !navigation.inTransition
        }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        let top = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertEqual(top.node.props["testId"], "route-detail")
        XCTAssertTrue(nav.viewControllers.first === root, "the root moved in under it")
        XCTAssertTrue(tabs.selectedViewController === nav)
        XCTAssertTrue(navigation.primaryNavigation === nav)
        XCTAssertTrue(navigation.activeRoute === top)
        XCTAssertNotNil(top.view.window, "Detail is in the window")
        XCTAssertNil(root.view.window, "Plain's root is under it")
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
    /// A `navigation` hook that reads the tab's own controller while Exact
    /// builds it (the fixture module's, `EXACT_FIXTURE_READ_TABS`) gets that
    /// controller: it is published before the hook runs.
    func testAHookReadingTheTabWhileItIsBuiltGetsIt() throws {
        setenv("EXACT_FIXTURE_READ_TABS", "1", 1)
        defer { unsetenv("EXACT_FIXTURE_READ_TABS"); NativeViews.uninstallTable() }
        let session = try fixture("held-reentrant", module: true)
        let navigation = session.presenter.navigation
        until("the hatches have offered the tabs") { navigation.tabContainerAsked }
        let (tabs, held, root) = try held(session)
        try press(session, "title")
        until("Plain has its navigation controller") { tabs.viewControllers?.first is UINavigationController }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        XCTAssertTrue(held.built === nav)
        XCTAssertTrue(navigation.navigation(forHatch: held) === nav)
        XCTAssertTrue(nav.viewControllers == [root])
    }

    /// The bar a held root comes to show opens its level-1 title expanded,
    /// as a stack built with that root does, its content at the top.
    func testABarAppearingOpensItsLargeTitleExpanded() throws {
        let session = try fixture("held-large")
        let (tabs, _, _) = try held(session)
        try press(session, "title")
        until("Plain has its navigation controller") { tabs.viewControllers?.first is UINavigationController }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        until("the bar shows") { !nav.isNavigationBarHidden }
        XCTAssertGreaterThan(nav.navigationBar.frame.height, 90, "the large title is expanded")
        let list = try XCTUnwrap(try node(session, "list").scrollView)
        XCTAssertEqual(list.contentOffset.y, -list.adjustedContentInset.top, accuracy: 0.5, "the content rests at its top")
    }

    /// Two routes pushed onto a held tab in one batch, as a deep link does:
    /// one stack, its top on it, and no Back read from the root's appearance.
    func testTwoRoutesPushedAtOnceEndOnTheTop() throws {
        let session = try fixture("held-deep")
        let navigation = session.presenter.navigation
        let (tabs, _, root) = try held(session)
        try press(session, "open-two")
        let declared = { ["route-plain", "route-detail", "route-more"].filter { id in session.presenter.views.values.contains { $0.props["testId"] == id } }.count }
        until("the router declares more than the root") { declared() > 2 }
        until("Plain holds the whole stack") { (tabs.viewControllers?.first as? UINavigationController)?.viewControllers.count == declared() }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        until("navigation is at rest") { nav.transitionCoordinator == nil && !navigation.inTransition }
        XCTAssertTrue(nav.viewControllers.first === root)
        XCTAssertEqual((nav.viewControllers.last as? RouteController)?.node.props["testId"], "route-more")
        XCTAssertEqual(navigation.observation()["route"] as? String, (nav.viewControllers.last as? RouteController)?.key, "no Back was read")
    }

    /// A tab whose bar is asked for while a sheet presents over it is built
    /// once the presentation is over, not in its middle.
    func testATabWaitsForASheetsPresentationBeforeItIsBuilt() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet presents only from a scene's window") }
        let session = try fixture("held-waits")
        let navigation = session.presenter.navigation
        let (tabs, held, _) = try held(session)
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try press(session, "open-sheet")
        until("the sheet is presenting") { navigation.realizeWaits }
        try press(session, "title")
        XCTAssertTrue(tabs.viewControllers?.first === held, "not while the sheet presents")
        until("the sheet is up") { !navigation.realizeWaits }
        until("then Plain has its navigation controller") { tabs.viewControllers?.first is UINavigationController }
        XCTAssertEqual(navigation.presentedNavigations.count, 1, "the sheet stays up")
    }

    func testADeferredHeaderProjectionKeepsTheTabContainerInItsViewport() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet presents only from a scene's window") }
        let session = try fixture("held-rail"), p = session.presenter, navigation = p.navigation
        let (tabs, held, root) = try held(session)
        let allocation = tabs.view.convert(tabs.view.bounds, to: p.viewport)
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try press(session, "open-sheet")
        until("the sheet is presenting") { navigation.realizeWaits }
        try press(session, "rail")
        XCTAssertTrue(tabs.viewControllers?.first === held)
        XCTAssertTrue(navigation.syncAfterTransitionArmed, "the header waits for a sync after the transition")
        until("the deferred projection finishes") {
            tabs.viewControllers?.first is UINavigationController && !navigation.syncAfterTransitionArmed && !navigation.inTransition
                && (try? self.node(session, "held-header-rail"))?.isHidden == true
        }
        let nav = try XCTUnwrap(tabs.viewControllers?.first as? UINavigationController)
        XCTAssertTrue(nav.viewControllers.first === root)
        XCTAssertEqual(root.navigationItem.rightBarButtonItems?.count, 2)
        XCTAssertEqual(tabs.view.convert(tabs.view.bounds, to: p.viewport), allocation)
        XCTAssertTrue(p.root.isDescendant(of: p.viewport))
        let content = try XCTUnwrap(root.contentViewport)
        XCTAssertLessThanOrEqual(content.contentSize.width, content.bounds.width + 0.5, "the deferred header leaves no horizontal range")
        XCTAssertEqual(navigation.presentedNavigations.count, 1, "the sheet stays up")
    }

    /// The route's node waits in the window while its controller moves: with
    /// a sheet up over the tab, whose presentation makes the tab container
    /// inert, the field's focus is as it was and VoiceOver's element is told
    /// again.
    func testTheMoveKeepsTheFocusUnderASheetAndTellsVoiceOver() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet presents only from a scene's window") }
        let session = try fixture("held-focus")
        let navigation = session.presenter.navigation
        let (tabs, held, _) = try held(session)
        let field = try XCTUnwrap(try node(session, "draft").field)
        XCTAssertTrue(field.becomeFirstResponder())
        XCTAssertTrue(session.navigate("/sheet"))
        until("the sheet is up") { navigation.presentedNavigations.count == 1 && !navigation.inTransition }
        XCTAssertFalse(tabs.view.isUserInteractionEnabled && !tabs.view.accessibilityElementsHidden, "the sheet made the tabs inert")
        let focused = field.isFirstResponder
        let spoken = KeptFocus.voiceOverFocus, told = KeptFocus.announce
        defer { KeptFocus.voiceOverFocus = spoken; KeptFocus.announce = told }
        var announced: [Any?] = []
        KeptFocus.voiceOverFocus = { field }
        KeptFocus.announce = { announced.append($0) }
        _ = try XCTUnwrap(navigation.navigation(forHatch: held))
        XCTAssertEqual(field.isFirstResponder, focused, "the field never left the window")
        XCTAssertEqual(announced.count, 1)
        XCTAssertTrue(announced.first as AnyObject? === field, "VoiceOver is told its element again")
    }
    /// The sync armed for a transition's end comes whatever the transition
    /// does: with none running it comes after a short wait, and a reset
    /// disarms it.
    func testTheSyncArmedForATransitionsEndAlwaysComes() throws {
        let session = try fixture("held-armed")
        let navigation = session.presenter.navigation
        XCTAssertFalse(navigation.realizeWaits)
        navigation.syncAfterTransition()
        XCTAssertTrue(navigation.syncAfterTransitionArmed)
        until("the armed sync ran") { !navigation.syncAfterTransitionArmed }
        navigation.syncAfterTransition()
        navigation.reset()
        XCTAssertFalse(navigation.syncAfterTransitionArmed, "a reset disarms it")
    }

    /// A hatch reading a held tab's controller while a sheet presents gets
    /// it at once; the route moves into it and it takes the tab's place
    /// only once the presentation is over.
    func testAHatchReadDuringASheetsPresentationWaitsForItsEnd() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet presents only from a scene's window") }
        let session = try fixture("held-hatch-waits")
        let navigation = session.presenter.navigation
        let (tabs, held, root) = try held(session)
        UIView.setAnimationsEnabled(true)
        defer { UIView.setAnimationsEnabled(false) }
        try press(session, "open-sheet")
        until("the sheet is presenting") { navigation.realizeWaits }
        let nav = try XCTUnwrap(navigation.navigation(forHatch: held))
        XCTAssertTrue(navigation.navigation(forHatch: held) === nav, "the same one, asked again")
        XCTAssertTrue(tabs.viewControllers?.first === held, "the tab keeps its place during the presentation")
        XCTAssertTrue(held.route === root)
        XCTAssertTrue(nav.viewControllers.isEmpty)
        until("the sheet is up") { !navigation.realizeWaits }
        until("then the controller holds the route in the tab's place") { tabs.viewControllers?.first === nav && nav.viewControllers == [root] }
        XCTAssertEqual(navigation.presentedNavigations.count, 1, "the sheet stays up")
    }
}
#endif
