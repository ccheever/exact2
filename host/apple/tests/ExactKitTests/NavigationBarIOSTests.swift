#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 Stage 1 over the native fixture's own plan and hatch module,
/// which `build.mjs --test --ios` compiles for this simulator: Exact's bar on
/// frame one from the authored header, the content area it leaves, Back as
/// UIKit's back button, the forwarded delegate, and the module's hatches —
/// their moments, the authored control a hatch-made item clicks, and the
/// development check of what Exact owns. UIKit synthesizes no touches for a
/// unit test; a bar item is driven as a tap drives it, by its action.
final class NavigationBarIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        // The module artifact is the process's: later tests run without it.
        NativeViews.uninstallTable()
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

    /// The fixture booted in a window, its module's hatches connected or not.
    private func fixture(_ label: String, module: Bool) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let path = try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")
        let plan = try Data(contentsOf: URL(fileURLWithPath: path))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        // In the test host's scene when it has one: UIKit presents a sheet
        // only from a window a scene holds.
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        if module {
            session.natives.installArtifact(try XCTUnwrap(env["EXACT_FIXTURE_MODULE"], "build.mjs --test --ios builds the fixture's module"))
        }
        until("fixture route and native work ready") {
            session.presenter.navigation.primaryNavigation?.topViewController is RouteController &&
                !Agent(session: session).nativeInFlight()
        }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func journal(_ session: ExactSession) -> String { session.agent(#"{"op":"logs","since":0}"#) }

    /// How many times the authored Back control's action ran.
    private func backs(_ session: ExactSession) -> Int { journal(session).components(separatedBy: "(back)").count - 1 }

    private func state(_ session: ExactSession, _ slot: String) -> Any? {
        let text = session.agent(#"{"op":"state"}"#)
        let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
        return (json?["slots"] as? [String: Any])?[slot] ?? json?[slot]
    }

    /// What a tap on a bar item does: its action, sent to its target.
    private func tap(_ item: UIBarButtonItem?) throws {
        let item = try XCTUnwrap(item)
        let target = try XCTUnwrap(item.target as? NSObject, "the item's target is retained")
        _ = target.perform(try XCTUnwrap(item.action), with: item)
    }

    func testAuthoredClearTitleInkAndAbsenceUseDifferentAppearances() throws {
        let session = try fixture("title-ink", module: false)
        let heading = try node(session, "title-home")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let item = try XCTUnwrap(nav.topViewController?.navigationItem)
        XCTAssertNil(item.standardAppearance)
        session.presenter.apply(wireBatch([["op": "style", "id": heading.id,
            "style": ["text_color": [0, 0, 0, 0], "written": ["text_color"]]]]))
        let appearance = try XCTUnwrap(item.standardAppearance)
        XCTAssertEqual((appearance.titleTextAttributes[.foregroundColor] as? UIColor)?.cgColor.alpha, 0)
        XCTAssertEqual((appearance.largeTitleTextAttributes[.foregroundColor] as? UIColor)?.cgColor.alpha, 0)
        session.presenter.apply(wireBatch([["op": "style", "id": heading.id,
            "style": ["text_color": [255, 0, 0, 255]]]]))
        XCTAssertNil(item.standardAppearance, "an inherited colour leaves UIKit's title alone")
    }

    func testTransparentTitleSurvivesRepeatedRoutePreparation() throws {
        let session = try fixture("transparent-title-ink", module: false)
        let p = session.presenter
        let heading = try node(session, "title-home")
        let route = try node(session, "route-home")
        let item = try XCTUnwrap(p.navigation.primaryNavigation?.topViewController?.navigationItem)
        p.apply(wireBatch([
            ["op": "props", "id": route.id, "set": ["navigationBar": "transparent"]],
            ["op": "style", "id": heading.id, "style": ["text_color": [0, 0, 0, 0], "written": ["text_color"]]]
        ]))
        for _ in 0..<2 {
            let edge = try XCTUnwrap(item.scrollEdgeAppearance)
            XCTAssertNil(edge.backgroundEffect)
            XCTAssertEqual(edge.backgroundColor?.cgColor.alpha ?? 0, 0)
            XCTAssertEqual((edge.titleTextAttributes[.foregroundColor] as? UIColor)?.cgColor.alpha, 0)
            XCTAssertEqual((edge.largeTitleTextAttributes[.foregroundColor] as? UIColor)?.cgColor.alpha, 0)
            p.apply(wireBatch([["op": "props", "id": heading.id, "set": ["accessibilityLabel": "Title"]]]))
        }
        p.apply(wireBatch([["op": "style", "id": heading.id, "style": ["text_color": [255, 0, 0, 255]]]]))
        XCTAssertEqual(item.scrollEdgeAppearance, NavigationHost.clearScrollEdge)
    }

    func testAPushAfterKeyboardDismissalInstallsTheDestinationBeforeReturning() throws {
        let session = try fixture("synchronous-push", module: false)
        let presenter = session.presenter
        let nav = try XCTUnwrap(presenter.navigation.primaryNavigation)
        // The hide notification can follow the press that ended editing. A
        // remaining keyboard inset must not postpone UIKit's stack change.
        presenter.applyKeyboard(top: 540, duration: 0, curve: 0)
        let button = try node(session, "open-plain")
        presenter.press(button.id)
        XCTAssertTrue((nav.topViewController as? RouteController)?.node === (try node(session, "route-plain")),
                       "the real destination is installed without pumping the run loop")
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertNotNil(route.node.window)
        XCTAssertGreaterThan(route.node.bounds.height, 0)
        until("push finished") { nav.transitionCoordinator == nil }
    }

    func testAPushFocusesTheRealFirstAutofocusField() throws {
        let session = try fixture("synchronous-autofocus", module: false)
        let presenter = session.presenter
        let nav = try XCTUnwrap(presenter.navigation.primaryNavigation)
        presenter.press(try node(session, "open-focused-plain").id)
        let first = try XCTUnwrap(try node(session, "plain-first").field)
        let second = try XCTUnwrap(try node(session, "plain-second").field)
        XCTAssertTrue(first.isFirstResponder, "normal autofocus chooses the first real field before returning")
        XCTAssertFalse(second.isFirstResponder)
        XCTAssertTrue(first.isDescendant(of: try XCTUnwrap(nav.topViewController).view))
        until("push finished") { nav.transitionCoordinator == nil }
        XCTAssertTrue(first.isFirstResponder, "there is no delayed focus handoff")
    }

    func testAppearanceNoneKeepsTheHeaderAndItsButtonAuthored() throws {
        let session = try fixture("authored-header", module: false)
        let p = session.presenter
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        let header = try node(session, "header-home")
        let button = try node(session, "compose-home")
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        p.apply(wireBatch([["op": "style", "id": Int(button.id), "style": ["appearance": "none"]]]))
        XCTAssertNotNil(HeaderShape.bar(of: route.node), "only explicit appearance opts out, not a compiler-implied row")
        p.apply(wireBatch([["op": "style", "id": Int(header.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        XCTAssertNil(HeaderShape.bar(of: route.node))
        XCTAssertTrue(nav.isNavigationBarHidden)
        XCTAssertFalse(header.isHidden)
        XCTAssertNil(route.lifted)
        // No items kept for a bar that is gone: the agent aims at an item by its press.
        XCTAssertTrue(route.barPresses.isEmpty)
        XCTAssertNil(route.navigationItem.rightBarButtonItems)
        XCTAssertNil(route.navigationItem.leftBarButtonItems)
        XCTAssertNil(Agent(session: session).barItem(button))
        XCTAssertEqual(p.navigation.bar(of: route.node, pushed: true), .hidden,
                       "an authored header does not gain a system Back bar when pushed")
        p.press(button.id)
        p.apply(wireBatch([["op": "style", "id": Int(header.id), "style": ["appearance": "auto", "written": ["appearance"]]]]))
        XCTAssertNotNil(HeaderShape.bar(of: route.node))
        XCTAssertFalse(nav.isNavigationBarHidden)
        XCTAssertTrue(header.isHidden)
        XCTAssertEqual(state(session, "composed") as? Double, 1)
    }

    func testAppearanceNoneOnAButtonKeepsOnlyThatButtonAuthored() throws {
        let session = try fixture("authored-header-button", module: false)
        let p = session.presenter
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let header = try node(session, "header-home")
        let button = try node(session, "compose-home")
        p.apply(wireBatch([["op": "style", "id": Int(button.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        XCTAssertNotNil(HeaderShape.bar(of: route.node))
        XCTAssertFalse(nav.isNavigationBarHidden)
        XCTAssertFalse(header.isHidden)
        XCTAssertFalse(button.isHidden)
        XCTAssertGreaterThan(button.bounds.height, 0)
        XCTAssertGreaterThanOrEqual(button.convert(button.bounds, to: route.view).minY, route.view.safeAreaInsets.top - 0.5)
        XCTAssertFalse((route.navigationItem.rightBarButtonItems ?? []).contains { $0.accessibilityIdentifier == "compose-home" })
        XCTAssertNotNil(route.navigationItem.titleView as? UISegmentedControl)
        XCTAssertNotNil(route.navigationItem.searchController)
        XCTAssertNil(Agent(session: session).substituted(button))
        XCTAssertNil(Agent(session: session).substituted(header), "the ancestor must not refuse a touch on the authored button")
        p.press(button.id)
        XCTAssertEqual(state(session, "composed") as? Double, 1)
        p.apply(wireBatch([["op": "style", "id": Int(button.id), "style": ["appearance": "auto", "written": ["appearance"]]]]))
        XCTAssertTrue(header.isHidden)
        XCTAssertTrue((route.navigationItem.rightBarButtonItems ?? []).contains { $0.accessibilityIdentifier == "compose-home" })
    }

    func testAppearanceNoneOnTheTitleTapKeepsItsWholeFaceAuthored() throws {
        let session = try fixture("authored-title-tap", module: false)
        let p = session.presenter
        p.press(try node(session, "chat").id)
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        until("chat is pushed") { nav.transitionCoordinator == nil }
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let tap = try node(session, "title-group")
        let heading = try node(session, "title-chat")
        XCTAssertTrue(tap.isButton, "exercise the button role around the heading")
        p.apply(wireBatch([["op": "style", "id": Int(tap.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        let shape = try XCTUnwrap(HeaderShape.bar(of: route.node, back: "back"))
        XCTAssertTrue(shape.authored.contains { $0 === tap })
        XCTAssertFalse(shape.placed.contains { $0 === heading || $0.isDescendant(of: tap) })
        XCTAssertFalse(shape.header.isHidden)
        XCTAssertFalse(heading.isHidden)
        XCTAssertNil(route.navigationItem.titleView)
        XCTAssertEqual(route.navigationItem.title, "")
        XCTAssertNil(Agent(session: session).substituted(tap))
        p.press(tap.id)
        XCTAssertEqual(state(session, "titlePresses") as? Double, 1)
        p.apply(wireBatch([["op": "style", "id": Int(tap.id), "style": ["appearance": "auto", "written": ["appearance"]]]]))
        XCTAssertTrue(shape.header.isHidden)
        XCTAssertNotNil(route.navigationItem.titleView)
    }

    /// A header partly projected (its Back opted out): the title's
    /// pressable wrapper goes with the title, and a node the projection hid
    /// shows again once it is authored and displayed.
    func testAPartlyProjectedHeaderHidesTheTitlesWrapperAndGivesItBack() throws {
        let session = try fixture("authored-back-title", module: false)
        let p = session.presenter
        p.press(try node(session, "chat").id)
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        until("chat is pushed") { nav.transitionCoordinator == nil && (nav.topViewController as? RouteController)?.node.props["testId"] == "route-chat" }
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let back = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "back" && $0.isDescendant(of: route.node) })
        let tap = try node(session, "title-group")
        p.apply(wireBatch([["op": "style", "id": Int(back.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        let shape = try XCTUnwrap(HeaderShape.bar(of: route.node, back: "back"))
        XCTAssertTrue(shape.placed.contains { $0 === tap }, "the wrapper is the title's")
        XCTAssertTrue(tap.isHidden, "no empty pressable box under the native title")
        XCTAssertFalse(back.isHidden)
        // Hidden by CSS as it leaves the projection, then displayed: CSS decides.
        p.apply(wireBatch([["op": "style", "id": Int(tap.id), "style": ["appearance": "none", "written": ["appearance"], "display": "none"]]]))
        p.apply(wireBatch([["op": "style", "id": Int(tap.id), "style": ["appearance": "none", "written": ["appearance"], "display": "flex"]]]))
        XCTAssertFalse(tap.isHidden)
    }

    func testAppearanceNoneOnBackLeavesOnlyTheAuthoredBack() throws {
        let session = try fixture("authored-back", module: false)
        let p = session.presenter
        p.press(try node(session, "chat").id)
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        until("chat is pushed") { nav.transitionCoordinator == nil }
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let back = try node(session, "back")
        p.apply(wireBatch([["op": "style", "id": Int(back.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        XCTAssertTrue(route.navigationItem.hidesBackButton)
        XCTAssertFalse(back.isHidden)
        XCTAssertFalse(try node(session, "header-chat").isHidden)
        XCTAssertNil(Agent(session: session).substituted(back))
        p.apply(wireBatch([["op": "style", "id": Int(back.id), "style": ["appearance": "auto", "written": ["appearance"]]]]))
        XCTAssertFalse(route.navigationItem.hidesBackButton)
        XCTAssertTrue(try node(session, "header-chat").isHidden)
        p.apply(wireBatch([["op": "style", "id": Int(back.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        XCTAssertTrue(route.navigationItem.hidesBackButton)
        p.press(back.id)
        until("the authored Back pops exactly once") { nav.viewControllers.count == 1 && nav.transitionCoordinator == nil }
        XCTAssertEqual(backs(session), 1)
    }

    func testAppearanceNoneOnSearchAndSegmentsKeepsTheirControlsAuthored() throws {
        let session = try fixture("authored-header-controls", module: false)
        let p = session.presenter
        let route = try XCTUnwrap(p.navigation.primaryNavigation?.topViewController as? RouteController)
        let search = try node(session, "header-search"), segments = try node(session, "header-segments")
        p.apply(wireBatch([search, segments].map { ["op": "style", "id": Int($0.id), "style": ["appearance": "none", "written": ["appearance"]]] }))
        XCTAssertNil(route.navigationItem.searchController)
        XCTAssertNil(route.navigationItem.titleView)
        XCTAssertFalse(search.isHidden)
        XCTAssertFalse(segments.isHidden)
        XCTAssertFalse(try node(session, "header-home").isHidden)
        p.apply(wireBatch([search, segments].map { ["op": "style", "id": Int($0.id), "style": ["appearance": "auto", "written": ["appearance"]]] }))
        XCTAssertNotNil(route.navigationItem.searchController)
        XCTAssertNotNil(route.navigationItem.titleView as? UISegmentedControl)
    }

    func testHeaderOptOutRetiresAnActiveSearchProjection() throws {
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a search presentation completes only in a scene’s window")
        }
        let session = try fixture("authored-header-search", module: false)
        let p = session.presenter
        // Isolate search retirement from the header-segment appearance regression.
        for id in ["segment-all", "segment-missed"] {
            let tab = try node(session, id)
            p.apply(wireBatch([["op": "style", "id": Int(tab.id), "style": ["appearance": "auto", "written": ["appearance"]]]]))
        }
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let header = try node(session, "header-home")
        let search = try XCTUnwrap(route.navigationItem.searchController)
        search.isActive = true
        XCTAssertTrue(search.searchBar.searchTextField.becomeFirstResponder())
        until("native search is presented") { search.isActive && !search.isBeingPresented }
        p.apply(wireBatch([["op": "style", "id": Int(header.id), "style": ["appearance": "none", "written": ["appearance"]]]]))
        until("native search is retired") { !search.isActive && search.presentingViewController == nil }
        XCTAssertFalse(search.searchBar.searchTextField.isFirstResponder)
        XCTAssertNil(route.search)
        XCTAssertNil(route.navigationItem.searchController)
        XCTAssertNil(route.navigationItem.titleView)
        XCTAssertTrue(nav.isNavigationBarHidden)
        XCTAssertFalse(header.isHidden)
        p.apply(wireBatch([["op": "style", "id": Int(header.id), "style": ["appearance": "auto", "written": ["appearance"]]]]))
        XCTAssertNotNil(route.navigationItem.searchController)
        XCTAssertFalse(nav.isNavigationBarHidden)
    }

    func testAHeaderShapedRouteIsTheBarOnFrameOneWithNoAppCode() throws {
        let session = try fixture("bar-frame-one", module: false)
        let navigation = session.presenter.navigation
        let nav = try XCTUnwrap(navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        XCTAssertFalse(nav.isNavigationBarHidden, "the first route is header-shaped")
        XCTAssertTrue(nav.navigationBar.prefersLargeTitles, "a level-1 heading is a large title")
        XCTAssertEqual(top.navigationItem.title, "Fixture")
        XCTAssertEqual(top.navigationItem.largeTitleDisplayMode, .always)
        XCTAssertEqual(top.navigationItem.rightBarButtonItems?.count, 1, "the header's Compose button")
        XCTAssertEqual(top.navigationItem.rightBarButtonItems?.first?.accessibilityLabel, "Compose")
        XCTAssertNil(top.navigationItem.rightBarButtonItems?.first?.tintColor, "the page's inherited `color` is not the button's: UIKit's tint")
        XCTAssertNil(top.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" }, "no module, no hatch")
        let header = try node(session, "header-home")
        XCTAssertTrue(header.isHidden, "iOS does not paint the header the bar shows")
        XCTAssertEqual(header.bounds.height, 0, "and it takes no space")
        // The content starts where the bar ends: the controller's safe area.
        let content = try node(session, "fixture")
        let bottom = top.view.safeAreaInsets.top
        XCTAssertGreaterThan(bottom, 0)
        XCTAssertEqual(content.convert(content.bounds, to: top.view).minY, bottom, accuracy: 0.5)
        // The bar item presses the authored button, as a tap on it would.
        XCTAssertEqual(state(session, "composed") as? Double, 0)
        try tap(top.navigationItem.rightBarButtonItems?.first)
        until("Compose pressed once") { state(session, "composed") as? Double == 1 }
    }

    /// LLP 1116.003 (r5 weather's Cities, r5 mood journal's sheet Save:
    /// "ScrollView covers its middle"): a real touch on a header button the
    /// bar draws aims at its bar item, where a person touches it, never at
    /// the hidden authored node in the content.
    func testARealTouchAimsAtTheBarItemUIKitDraws() throws {
        let session = try fixture("bar-aim", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let compose = try node(session, "compose-home")
        let header = try node(session, "header-home")
        XCTAssertTrue(header.isHidden, "the bar shows the header: its authored button is hidden")
        let reply = Agent(session: session).aim(["id": Int(compose.id), "aim": ["press": true]])
        if (reply["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "the test host is in the background: a real touch's aim refuses it") }
        let aim = try XCTUnwrap(reply["aim"] as? [String: Any], "\(reply)")
        let w = try XCTUnwrap(aim["window"] as? [Double])
        let point = CGPoint(x: w[0], y: w[1])
        let item = try XCTUnwrap(nav.topViewController?.navigationItem.rightBarButtonItems?.first)
        let frame = item.frame(in: nav.navigationBar) ?? .null
        XCTAssertTrue(nav.navigationBar.convert(frame, to: nil).contains(point), "at Compose's item: \(point), \(frame)")
        let hit = try XCTUnwrap(window?.hitTest(point, with: nil))
        XCTAssertTrue(hit.isDescendant(of: nav.navigationBar), "the bar takes the touch: \(hit)")
        XCTAssertEqual(aim["hit"] as? Int, TouchLog.landing(hit)["node"] as? Int, "the dispatch log must see it land where the bar is: \(aim)")
    }

    /// A bar item takes the colour its author wrote on its button, or the
    /// tint written on its symbol (LLP 1115.000 D2): a header's Cancel in the
    /// app's accent, a Delete in the platform's red. A colour the button
    /// only inherits, or one the compiler implied, leaves UIKit's.
    func testABarItemTakesItsButtonsAuthoredColour() throws {
        let p = Presenter()
        let red: [String: Any] = ["sys": "systemRedColor", "c": [[255, 56, 60, 255], [255, 66, 69, 255]]]
        let page = [17, 17, 17, 255], wrote = ["text_color"]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"], "style": ["text_color": [181, 86, 43, 255], "written": wrote]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Cancel"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "style": ["text_color": red, "written": wrote]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Delete"], "style": ["text_color": red]],
            ["op": "create", "id": 5, "kind": "view", "style": ["text_color": page, "written": wrote]],
            ["op": "create", "id": 6, "kind": "button", "handlers": ["press"], "style": ["text_color": page]],
            ["op": "create", "id": 7, "kind": "text", "props": ["text": "Edit"], "style": ["text_color": page]],
            ["op": "create", "id": 8, "kind": "button", "handlers": ["press"], "style": ["text_color": [0, 122, 255, 255]]],
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "Implied"]],
            ["op": "create", "id": 10, "kind": "button", "handlers": ["press"], "style": ["text_color": page]],
            ["op": "create", "id": 11, "kind": "image", "props": ["imageSource": "symbol:delete", "symbolName": "trash"],
             "style": ["text_color": red, "written": ["text_color"]]],
            ["op": "create", "id": 12, "kind": "button", "handlers": ["press"], "style": ["text_color": page]],
            ["op": "create", "id": 13, "kind": "image", "props": ["imageSource": "symbol:add", "symbolName": "plus"], "style": ["text_color": [0, 122, 255, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 8, "ids": [9]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 12, "ids": [13]],
            ["op": "children", "id": 5, "ids": [1, 3, 6, 8, 10, 12]],
            ["op": "roots", "ids": [5]],
        ]))
        func item(_ id: UInt32) throws -> HeaderShape.Item { HeaderShape.Item(try XCTUnwrap(p.views[id])) }
        XCTAssertEqual(try item(1).title, "Cancel")
        XCTAssertEqual(try item(1).tint, [[181, 86, 43, 255], [181, 86, 43, 255]])
        let role = try XCTUnwrap(try item(3).tint, "a platform colour its author wrote is kept")
        XCTAssertEqual(role.count, 2, "light and dark")
        XCTAssertGreaterThan(role[0][0], 200, "red: \(role)")
        XCTAssertLessThan(role[0][2], 100, "red: \(role)")
        XCTAssertNil(try item(6).tint, "an inherited colour is UIKit's to draw")
        XCTAssertNil(try item(8).tint, "a colour the compiler implied is UIKit's to draw, though it differs from the page's")
        XCTAssertEqual(try item(10).symbol, "trash")
        XCTAssertNotNil(try item(10).tint, "a color its author wrote on the symbol")
        XCTAssertNil(try item(12).tint, "a color the symbol was only given (a sheet's) is UIKit's")
    }

    /// LLP 1075.003 §9.6: a button whose one child is a filled box is an
    /// image item drawn from it; a popover's invoker is an item whose menu,
    /// titled by the popover's `aria-label`, holds the popover's rows, each
    /// pressing its authored button.
    func testABadgeButtonIsAnImageItemAndAPopoverInvokerAMenuItem() throws {
        let session = try fixture("bar-badge-menu", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let left = top.navigationItem.leftBarButtonItems ?? []
        let badge = try XCTUnwrap(left.first { $0.accessibilityLabel == "Profile" }, "the badge button is an item")
        let image = try XCTUnwrap(badge.image, "drawn from the box")
        XCTAssertEqual(image.size, CGSize(width: 36, height: 36))
        XCTAssertEqual(image.renderingMode, .alwaysOriginal, "the box's own colours, not the bar's tint")
        XCTAssertNil(badge.menu)
        try tap(badge)
        until("the badge pressed its button") { state(session, "composed") as? Double == 1 }
        let more = try XCTUnwrap(left.first { $0.accessibilityLabel == "More" }, "the popover's invoker is an item")
        let menu = try XCTUnwrap(more.menu, "with the popover as its menu")
        XCTAssertNil(more.action, "UIKit opens the menu on a tap")
        XCTAssertEqual(menu.title, "Actions", "the popover's `aria-label` titles the menu, as a hand-built bar item's `UIMenu(title:)`")
        XCTAssertTrue(menu.children.first is UIDeferredMenuElement, "rows read as it opens")
        let pop = try XCTUnwrap(session.presenter.carrying("popover").first { $0.props["id"] == "bar-menu" })
        let rows = session.presenter.menus.items(of: pop)
        let row = try XCTUnwrap(rows.first as? UIAction)
        XCTAssertEqual(row.title, "Bump from the bar")
        // Where the agent shows UIKit's menus rather than painting its own
        // (as here), its tap on that item says what it cannot do, by name:
        // UIKit's pull-down opens from the bar under a real touch alone.
        let invoker = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "menu-item" })
        let refusal = session.presenter.navigation.activateChrome(invoker)?["error"] as? String
        XCTAssertTrue(refusal?.hasPrefix("unsupported: bar item") == true, "\(refusal ?? "nil")")
    }

    /// LLP 1021 D3: only a menu-shaped popover is a bar item's pull-down. An
    /// item opening any other popover (Harmony's pins and members) is a plain
    /// item whose tap opens the popover itself, lifted and anchored under the
    /// item, as a native button in the page opens one; a second tap closes it.
    func testABarItemOpeningAPopoverThatIsNoMenuOpensItFromTheItem() throws {
        let session = try fixture("bar-popover", module: false)
        let agent = Agent(session: session)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "open-plain").id)])["error"])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the plain route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let plain = try XCTUnwrap(nav.topViewController)
        let about = try XCTUnwrap(plain.navigationItem.rightBarButtonItems?.first { $0.accessibilityLabel == "About" }, "the invoker is an item")
        XCTAssertNil(about.menu, "no pull-down: its popover is no menu")
        XCTAssertNotNil(about.action)
        let pop = try node(session, "plain-about")
        // Opening it moves nothing around where it was written (Harmony's room composer jumped).
        let route = try node(session, "route-plain")
        func frames() -> [String: CGRect] {
            var out: [String: CGRect] = [:]
            for v in session.presenter.views.values where v !== pop && v.isDescendant(of: route) && !v.isDescendant(of: pop) {
                if let id = v.props["testId"] { out[id] = v.convert(v.bounds, to: nil) }
            }
            return out
        }
        let before = frames()
        try tap(about)
        until("the popover opens") { session.presenter.menus.isOpen(pop) && pop.window != nil && !pop.isHidden }
        spin(0.3)
        let after = frames()
        for (id, frame) in before { XCTAssertEqual(after[id], frame, "\(id) stays put while the popover is open") }
        let bar = nav.navigationBar.convert(nav.navigationBar.bounds, to: nil)
        let frame = pop.convert(pop.bounds, to: nil)
        XCTAssertGreaterThan(frame.minY, bar.minY, "from the item in the bar: \(frame) \(bar)")
        XCTAssertLessThan(frame.minY, bar.maxY + 20, "beside the item, not centred on the page: \(frame)")
        XCTAssertGreaterThan(frame.maxX, bar.midX, "on the item's side, the trailing one: \(frame)")
        // A real touch light-dismisses before the item acts (PopoverTouch):
        // one on the item that opened it is not outside it, so the item's
        // tap toggles it; one elsewhere on the bar is.
        let invoker = try node(session, "about-plain")
        guard case .success(let (barView, itemRect))? = session.presenter.navigation.barItem(invoker) else { return XCTFail("the item's rect") }
        let onItem = UIView(frame: itemRect), elsewhere = UIView(frame: CGRect(x: 0, y: 0, width: 20, height: 20))
        barView.addSubview(onItem); barView.addSubview(elsewhere)
        defer { onItem.removeFromSuperview(); elsewhere.removeFromSuperview() }
        session.presenter.menus.lightDismiss(onItem)
        XCTAssertTrue(session.presenter.menus.isOpen(pop), "a touch on its item is not outside it")
        // The agent's tap dismisses on the hidden button first: that is its invoker too.
        session.presenter.menus.lightDismiss(invoker)
        XCTAssertTrue(session.presenter.menus.isOpen(pop), "nor is the agent's, on the item's button")
        try tap(about)
        until("a second tap closes it") { !session.presenter.menus.isOpen(pop) }
        try tap(about)
        until("it opens again") { session.presenter.menus.isOpen(pop) }
        session.presenter.menus.lightDismiss(elsewhere)
        until("a touch elsewhere on the bar dismisses it") { !session.presenter.menus.isOpen(pop) }
        // A press of its own runs first; the item's popover then opens from it (`invoke`).
        session.presenter.menus.invoke(invoker)
        until("an invocation after a press opens it") { session.presenter.menus.isOpen(pop) }
    }

    /// LLP 1075.003 §9.6: a header's `input type="search"` is the item's
    /// search controller; what the reader types is the field's `input`.
    func testAHeaderSearchFieldIsTheItemsSearchControllerAndTypingIsItsInput() throws {
        let session = try fixture("bar-search", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let search = try XCTUnwrap(top.navigationItem.searchController, "the header's search field")
        XCTAssertEqual(search.searchBar.placeholder, "Search the fixture")
        // LLP 1115.000 D11.2: where the field goes and whether it hides as
        // the content scrolls are UIKit's own answers, not pinned by Exact.
        XCTAssertEqual(top.navigationItem.preferredSearchBarPlacement, .automatic, "UIKit's placement")
        XCTAssertTrue(top.navigationItem.hidesSearchBarWhenScrolling, "it hides on scroll, as UIKit's does")
        // A field its author made stick vertically stays in view while empty.
        let input = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "header-search" })
        let header = try XCTUnwrap((top as? RouteController)?.search)
        input.style["position_type"] = .string("sticky")
        input.style["left"] = .number(0)
        header.applyHiding()
        XCTAssertTrue(top.navigationItem.hidesSearchBarWhenScrolling, "sticky with no vertical inset does not stick on that axis")
        input.style["top"] = .string("auto")
        header.applyHiding()
        XCTAssertTrue(top.navigationItem.hidesSearchBarWhenScrolling, "`top: auto` is no inset")
        input.style["top"] = .number(0)
        header.applyHiding()
        XCTAssertFalse(top.navigationItem.hidesSearchBarWhenScrolling, "a field sticky at its top stays in view")
        input.style["position_type"] = nil
        input.style["top"] = nil
        input.style["left"] = nil
        header.applyHiding()
        XCTAssertTrue(top.navigationItem.hidesSearchBarWhenScrolling)
        search.searchBar.text = "abc"
        search.searchResultsUpdater?.updateSearchResults(for: search)
        until("the field's input ran with the text") { state(session, "query") as? String == "abc" }
        XCTAssertFalse(top.navigationItem.hidesSearchBarWhenScrolling, "a search in effect keeps its field in view, as UIKit keeps an active search's")
        // LLP 1115.000 D9.2: the keyboard's Search key puts the keyboard
        // away, as a hand-built search bar's delegate does, and the search
        // stays as typed.
        let field = search.searchBar.searchTextField
        XCTAssertTrue(field.becomeFirstResponder())
        search.searchBar.delegate?.searchBarSearchButtonClicked?(search.searchBar)
        XCTAssertFalse(field.isFirstResponder, "Search puts the keyboard away")
        XCTAssertEqual(search.searchBar.text, "abc")
        XCTAssertEqual(state(session, "query") as? String, "abc")
    }

    /// A search colour changes only the ink; clearing it restores UIKit's placeholder.
    func testSearchPlaceholderColourKeepsTheFontAndResets() throws {
        let session = try fixture("search-placeholder", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let search = try XCTUnwrap(nav.topViewController?.navigationItem.searchController)
        let field = search.searchBar.searchTextField
        let source = try node(session, "header-search")
        let original = field.attributedPlaceholder
        let font = try XCTUnwrap(field.font)
        func project() {
            session.presenter.navigation.prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
        }
        func ink() -> UIColor? { field.attributedPlaceholder?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor }
        // Unsaid, the search field's own ink, darker than a text field's
        // `placeholderText` (LLP 1104 D3, amended 2026-10-16).
        XCTAssertNotEqual(ink()?.resolvedColor(with: field.traitCollection), UIColor.placeholderText.resolvedColor(with: field.traitCollection))
        let red: [String: BatchValue] = ["placeholder_color": .array([.number(255), .number(0), .number(0), .number(255)])]
        source.applyStyle(red)
        let authored = try XCTUnwrap(source.placeholderColor)
        project()
        XCTAssertEqual(ink(), authored)
        XCTAssertEqual(field.attributedPlaceholder?.attribute(.font, at: 0, effectiveRange: nil) as? UIFont, font)
        source.applyStyle([:])
        project()
        XCTAssertEqual(field.attributedPlaceholder, original, "the same text with no colour restores UIKit's attributes")
        source.applyStyle(red)
        project()
        source.applyProps(set: ["placeholder": ""], clear: [])
        project()
        XCTAssertTrue(field.attributedPlaceholder == nil || field.attributedPlaceholder?.length == 0)
        source.applyProps(set: ["placeholder": "Search again"], clear: [])
        project()
        XCTAssertEqual(field.attributedPlaceholder?.string, "Search again")
        XCTAssertEqual(ink(), authored)
    }

    /// LLP 1075.003 §9.8: a header's text tablist is the item's title view,
    /// a segmented control; a segment's tap presses its tab.
    func testAHeaderTablistIsTheItemsSegmentedTitleView() throws {
        let session = try fixture("bar-segments", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let control = try XCTUnwrap(top.navigationItem.titleView as? UISegmentedControl, "the tablist is the title view")
        XCTAssertEqual(control.numberOfSegments, 2)
        XCTAssertEqual(control.titleForSegment(at: 1), "Missed")
        XCTAssertEqual(control.selectedSegmentIndex, 0)
        XCTAssertEqual(top.navigationItem.title, "Fixture", "the heading stays the title")
        XCTAssertFalse((top.navigationItem.rightBarButtonItems ?? []).contains { $0.title == "All" || $0.title == "Missed" }, "tabs are not items")
        let press = try XCTUnwrap(control.allTargets.first as? SegmentPress, "the control's target")
        XCTAssertNotNil(press.host)
        XCTAssertEqual(press.tabs, [try node(session, "segment-all").id, try node(session, "segment-missed").id])
        // A tap: the segment selected, then the control's action for a
        // value change sent to its target.
        control.selectedSegmentIndex = 1
        let action = try XCTUnwrap(control.actions(forTarget: press, forControlEvent: .valueChanged)?.first, "registered for a value change")
        _ = press.perform(NSSelectorFromString(action), with: control)
        until("the tab pressed") { state(session, "segment") as? String == "missed" }
        until("the selection follows aria-selected") { control.selectedSegmentIndex == 1 }
        let tapped = Agent(session: session).tap(["id": Int(try node(session, "segment-all").id)])
        XCTAssertNil(tapped["error"], "\(tapped)")
        XCTAssertEqual(tapped["native"] as? String, "segmented-control")
        until("the agent presses the header projection") { state(session, "segment") as? String == "all" && control.selectedSegmentIndex == 0 }

        // LLP 1115.000 D11.6: the title takes its own width, the size UIKit
        // fits it to. Exact held each segment to 90 points at least, so two
        // short titles made a control 180 wide.
        let own = { control.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)).width }
        XCTAssertEqual(control.frame.width, own(), accuracy: 0.5, "its own width")
        XCTAssertLessThan(control.frame.width, 180, "two short titles: narrower than 90 points a segment")
        // Sized for the text size and weight it shows, when they change and
        // no batch follows (every batch leaves it alone otherwise).
        let segments = try XCTUnwrap((top as? RouteController)?.titleSegments)
        for (category, weight) in [(UIContentSizeCategory.accessibilityExtraLarge, UILegibilityWeight.regular), (.accessibilityExtraLarge, .bold)] {
            control.frame.size = CGSize(width: 1, height: 1)
            control.traitOverrides.preferredContentSizeCategory = category
            control.traitOverrides.legibilityWeight = weight
            control.updateTraitsIfNeeded()
            XCTAssertEqual(segments.sized?.traits, [category, weight.rawValue] as [AnyHashable])
            XCTAssertEqual(control.frame.width, own(), accuracy: 0.5, "sized again, to its own width")
            XCTAssertGreaterThan(control.frame.width, 1)
            XCTAssertGreaterThan(control.frame.height, 1)
        }
    }

    func testARIAUnavailableHeaderItemStaysPressableAndIsSpokenDisabled() throws {
        let session = try fixture("aria-header-item", module: false)
        let button = try node(session, "compose-home")
        session.presenter.apply(wireBatch([["op": "props", "id": Int(button.id), "set": ["accessibilityDisabled": "true"]]]))
        let top = try XCTUnwrap(session.presenter.navigation.primaryNavigation?.topViewController)
        let item = try XCTUnwrap(top.navigationItem.rightBarButtonItems?.first { $0.accessibilityLabel == "Compose" })
        XCTAssertTrue(item.isEnabled)
        XCTAssertTrue(item.accessibilityTraits.contains(.notEnabled))
        try tap(item)
        until("ARIA-unavailable bar item still presses") { state(session, "composed") as? Double == 1 }
        session.presenter.apply(wireBatch([["op": "props", "id": Int(button.id), "set": ["accessibilityDisabled": "false"]]]))
        let restored = try XCTUnwrap(top.navigationItem.rightBarButtonItems?.first { $0.accessibilityLabel == "Compose" })
        XCTAssertFalse(restored.accessibilityTraits.contains(.notEnabled))
    }

    func testARIAUnavailableHeaderTabCarriesStateToTitleSegment() throws {
        let session = try fixture("aria-header-segment", module: false)
        let tab = try node(session, "segment-missed")
        session.presenter.apply(wireBatch([["op": "props", "id": Int(tab.id), "set": ["accessibilityDisabled": "true"]]]))
        let top = try XCTUnwrap(session.presenter.navigation.primaryNavigation?.topViewController)
        let control = try XCTUnwrap(top.navigationItem.titleView as? LabelledSegmentedControl)
        control.layoutIfNeeded()
        XCTAssertTrue(control.isEnabledForSegment(at: 1))
        XCTAssertTrue(try XCTUnwrap(control.segmentViews.last).accessibilityTraits.contains(.notEnabled))
    }

    func testAPushedRouteShowsBackAsTheAuthoredControlAndAPopPressesItOnce() throws {
        let session = try fixture("bar-push", module: false)
        let agent = Agent(session: session)
        let tapped = try node(session, "detail")
        let reply = agent.tap(["id": Int(tapped.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, Int(tapped.id), "the native projection stays above its authored holders")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the detail route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController), home = nav.viewControllers[0]
        XCTAssertEqual(detail.navigationItem.title, "Detail")
        XCTAssertEqual(detail.navigationItem.largeTitleDisplayMode, .never, "a level-2 heading is inline")
        XCTAssertFalse(detail.navigationItem.hidesBackButton, "the route has an enabled Back control")
        XCTAssertEqual(home.navigationItem.backButtonDisplayMode, .minimal, "the authored Back is a symbol alone")
        XCTAssertEqual(detail.navigationItem.rightBarButtonItems?.count, 1, "Back is never an item; Compose is")
        // Under a fixed bar an authored scrollTop is the browser's: the scroll
        // view sits below the bar, so 80 is an offset of 80 and 0 is 0.
        let list = try node(session, "list-detail")
        _ = agent.tap(["id": Int(try node(session, "scroll-80").id)])
        until("scrollTop 80") { list.scroll?.contentOffset.y == 80 }
        _ = agent.tap(["id": Int(try node(session, "scroll-0").id)])
        until("scrollTop 0") { list.scroll?.contentOffset.y == 0 }
        // UIKit's own pop (its back button) is a completed pop: Back, once.
        nav.popViewController(animated: true)
        until("Back dispatched and the stack follows the router") {
            (state(session, "nav") as? [String: Any]).map { (($0["tabs"] as? [[String: Any]])?.first?["stack"] as? [Any])?.count == 1 } ?? false
        }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 1)
        XCTAssertEqual(backs(session), 1, "Back once")
    }

    /// LLP 1115 D5: a pushed route that declares no Back control still has
    /// UIKit's back button and edge swipe, as a hand-built screen does, and
    /// a completed pop goes back as the web's history Back does: the root's
    /// `navigate` with the location beneath, once, and no Back is pressed.
    func testAPushedRouteWithNoBackControlStillGoesBackByTheRootsNavigate() throws {
        let session = try fixture("bar-back-always", module: false)
        let agent = Agent(session: session)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "open-plain").id)])["error"])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the plain route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let plain = try XCTUnwrap(nav.topViewController)
        XCTAssertEqual(plain.navigationItem.title, "Plain")
        XCTAssertFalse(plain.navigationItem.hidesBackButton, "Back is always there")
        XCTAssertTrue(session.presenter.navigation.canInvokeBack)
        let pop = try XCTUnwrap(nav.interactivePopGestureRecognizer)
        XCTAssertTrue(session.presenter.navigation.popMayBegin(pop, from: CGPoint(x: 4, y: 400), in: nav.view, velocity: CGPoint(x: 600, y: 20)),
                      "the edge swipe may begin")
        // UIKit's own pop (its back button) is a completed pop.
        nav.popViewController(animated: true)
        until("the root's navigate went back and the stack follows the router") {
            (state(session, "nav") as? [String: Any]).map { (($0["tabs"] as? [[String: Any]])?.first?["stack"] as? [Any])?.count == 1 } ?? false
        }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 1)
        XCTAssertEqual(journal(session).components(separatedBy: "(follow)").count - 1, 1, "navigate once")
        XCTAssertEqual(backs(session), 0, "no Back control was pressed")
        XCTAssertFalse(journal(session).contains("back gesture refused"))
    }

    /// LLP 1116 D6: a root that names no Back control (`navigationBack`
    /// left out, which the compiler lowers to the empty name) keeps the
    /// native stack, and a pushed route's system Back goes by the root's
    /// `navigate` even where an element is `id="back"`: no control is named.
    func testARootThatNamesNoBackControlKeepsTheNativeStackAndTheSystemBack() throws {
        let session = try fixture("bar-unnamed-back", module: false)
        let root = try node(session, "navigation")
        root.props["navigationBack"] = ""
        let agent = Agent(session: session)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "detail").id)])["error"])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("Detail is pushed onto the native stack") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController), home = nav.viewControllers[0]
        XCTAssertFalse(nav.isNavigationBarHidden)
        XCTAssertFalse(detail.navigationItem.hidesBackButton, "the system Back, with no control named")
        XCTAssertEqual(home.navigationItem.backButtonDisplayMode, .default, "no authored control shapes it")
        XCTAssertEqual(detail.navigationItem.leftBarButtonItems?.first?.accessibilityIdentifier, "back", "Detail's `id=\"back\"` button is an item like any other")
        XCTAssertTrue(session.presenter.navigation.canInvokeBack)
        let pop = try XCTUnwrap(nav.interactivePopGestureRecognizer)
        XCTAssertTrue(session.presenter.navigation.popMayBegin(pop, from: CGPoint(x: 4, y: 400), in: nav.view, velocity: CGPoint(x: 600, y: 20)))
        nav.popViewController(animated: true)
        until("the root's navigate went back and the stack follows the router") {
            (state(session, "nav") as? [String: Any]).map { (($0["tabs"] as? [[String: Any]])?.first?["stack"] as? [Any])?.count == 1 } ?? false
        }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 1)
        XCTAssertEqual(journal(session).components(separatedBy: "(follow)").count - 1, 1, "navigate once")
        XCTAssertEqual(backs(session), 0, "nothing named, nothing pressed")
    }

    /// LLP 1116 D2: a bar item's, the search field's, a title segment's and
    /// a tab bar item's `accessibilityIdentifier` is its authored `testId`,
    /// so XCUITest and Maestro address them as any other node.
    func testBarItemsTheSearchFieldSegmentsAndTabsCarryTheirTestIds() throws {
        let session = try fixture("bar-test-ids", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let top = try XCTUnwrap(nav.topViewController)
        let items = (top.navigationItem.leftBarButtonItems ?? []) + (top.navigationItem.rightBarButtonItems ?? [])
        XCTAssertEqual(Set(items.compactMap(\.accessibilityIdentifier)), ["badge-item", "menu-item", "compose-home"])
        XCTAssertEqual(top.navigationItem.searchController?.searchBar.searchTextField.accessibilityIdentifier, "header-search")
        let control = try XCTUnwrap(top.navigationItem.titleView as? LabelledSegmentedControl)
        XCTAssertEqual(control.accessibilityIdentifier, "header-segments")
        control.layoutIfNeeded()
        XCTAssertEqual(control.segmentViews.map(\.accessibilityIdentifier), ["segment-all", "segment-missed"])
        XCTAssertEqual(control.segmentViews.map(\.accessibilityLabel), [nil, nil], "no name of their own: UIKit's, their words")
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        XCTAssertEqual(tabs.viewControllers?.map(\.tabBarItem.accessibilityIdentifier), ["tab-home", "tab-second"])
    }

    /// LLP 1115 D5 with no `navigate` handler on the root: the button and
    /// the edge swipe still go, and a completed pop is the runner's own
    /// `back` (`host back` in the journal), with nothing dispatched.
    func testAPushedRouteWithNoBackControlAndNoNavigateHandlerIsTheRunnersOwnBack() throws {
        let session = try fixture("bar-host-back", module: false)
        let root = try node(session, "navigation")
        root.handlers.remove("navigate")
        XCTAssertNil(Agent(session: session).tap(["id": Int(try node(session, "open-plain").id)])["error"])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the plain route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        root.handlers.remove("navigate") // a batch may have written the root's handlers again
        XCTAssertFalse(try XCTUnwrap(nav.topViewController).navigationItem.hidesBackButton, "Back is always there")
        let pop = try XCTUnwrap(nav.interactivePopGestureRecognizer)
        XCTAssertTrue(session.presenter.navigation.popMayBegin(pop, from: CGPoint(x: 4, y: 400), in: nav.view, velocity: CGPoint(x: 600, y: 20)))
        nav.popViewController(animated: true)
        until("the runner went back and the stack follows the router") {
            (state(session, "nav") as? [String: Any]).map { (($0["tabs"] as? [[String: Any]])?.first?["stack"] as? [Any])?.count == 1 } ?? false
        }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 1)
        let lines = journal(session)
        XCTAssertEqual(lines.components(separatedBy: "host back").count - 1, 1, "the runner's own back, once: \(lines)")
        XCTAssertEqual(lines.components(separatedBy: "(follow)").count - 1, 0, "no navigate")
        XCTAssertEqual(backs(session), 0)
    }

    /// A pop UIKit finishes with no enabled Back control to press (here,
    /// disabled just before the bar's pop, as a tap racing that batch does)
    /// presses nothing, and the native stack goes back to the one the router
    /// still declares, not one route short of it. The Brooks port's fork
    /// needed a repair for this; main already restores the stack, with the
    /// batch before the pop or during it, and this keeps it so.
    func testAPopThatOutlivesItsBackControlRestoresTheDeclaredStack() throws {
        let session = try fixture("bar-back-gone", module: false)
        let agent = Agent(session: session)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "detail").id)])["error"])
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        until("the detail route is pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let p = session.presenter
        let name = try XCTUnwrap(p.views.values.first { $0.props["navigationBack"] != nil }?.props["navigationBack"])
        let back = try XCTUnwrap(p.views.values.first { $0.props["id"] == name && $0.handlers.contains("press") })
        p.apply(wireBatch([["op": "props", "id": Int(back.id), "set": ["disabled": "true"], "clear": []]]))
        nav.popViewController(animated: true)
        until("the transition ends and the declared stack is back") { nav.transitionCoordinator == nil && nav.viewControllers.count == 2 }
        spin(0.2)
        XCTAssertEqual(nav.viewControllers.count, 2, "the router still declares the detail route")
        XCTAssertEqual(backs(session), 0, "no enabled Back control, so nothing is pressed")
    }

    func testAForwardedCustomTransitionPopsOnceAndACancelledOneNever() throws {
        let session = try fixture("bar-delegate", module: false)
        let navigation = session.presenter.navigation
        _ = Agent(session: session).tap(["id": Int(try node(session, "detail").id)])
        let nav = try XCTUnwrap(navigation.primaryNavigation)
        until("pushed") { nav.viewControllers.count == 2 && nav.transitionCoordinator == nil }
        let app = Transitions()
        navigation.setAppDelegate(nav, app)
        XCTAssertTrue(nav.delegate !== app, "Exact keeps the slot")
        // Cancelled: an interactive pop the app drives and abandons.
        app.interactive = true
        nav.popViewController(animated: true)
        spin(0.05)
        app.interaction.update(0.3)
        app.interaction.cancel()
        until("the cancelled pop returns") { nav.transitionCoordinator == nil && nav.viewControllers.count == 2 }
        spin(0.2)
        XCTAssertTrue(app.animated, "the app's animator ran, through the forwarded delegate")
        XCTAssertEqual(backs(session), 0, "no Back")
        let phase = { (navigation.observation()["transition"] as? [String: Any])?["phase"] as? String }
        XCTAssertFalse(navigation.changing, "the cancelled pop is over")
        XCTAssertEqual(phase(), "cancelled")
        // Completed: the same animator, finished.
        app.interactive = false
        nav.popViewController(animated: true)
        until("the completed pop lands") { nav.transitionCoordinator == nil && nav.viewControllers.count == 1 }
        spin(0.2)
        XCTAssertGreaterThan(app.shown, 0, "didShow forwarded to the app")
        XCTAssertEqual(backs(session), 1, "Back once")
        XCTAssertEqual(phase(), "completed")
    }

    func testHatchOwnershipAcceptsTheContentViewportAndDetectsARemovedRoute() throws {
        let session = try fixture("hatch-content-mount", module: true)
        let navigation = session.presenter.navigation
        until("the route hatch is connected") { journal(session).contains("hatch route 0: built") }
        _ = Agent(session: session).clock(["settle": true])
        let route = try XCTUnwrap(navigation.primaryNavigation?.topViewController as? RouteController)
        let mount = try XCTUnwrap(route.contentViewport)
        XCTAssertTrue(route.node.superview === mount)
        let violation = "route 0: view changed outside Exact, which owns it"
        navigation.checkOwned()
        XCTAssertFalse(journal(session).contains(violation), "Exact's own content mount is valid")
        defer { mount.addSubview(route.node) }
        route.node.removeFromSuperview()
        navigation.checkOwned()
        XCTAssertTrue(journal(session).contains(violation), "the first real ownership violation is still reported")
    }

    func testTheModulesHatchesRunAtTheirMomentsAndActThroughAuthoredControls() throws {
        let session = try fixture("bar-hatches", module: true)
        let log = { self.journal(session) }
        until("hatches connected and replayed") { log().contains("hatch route 0: built") }
        let lines = log()
        let navigationAt = try XCTUnwrap(lines.range(of: "hatch navigation #1: built"))
        XCTAssertLessThan(navigationAt.lowerBound, try XCTUnwrap(lines.range(of: "hatch route 0: built")).lowerBound, "a stack's hatch runs before its routes'")
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        XCTAssertEqual(nav.navigationBar.tintColor, .systemIndigo, "the long tail is the hatch's")
        let home = try XCTUnwrap(nav.topViewController)
        // A route prepared again with nothing changed keeps its projected
        // items and runs no hatch.
        let items = home.navigationItem.rightBarButtonItems ?? []
        session.presenter.navigation.prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
        spin(0.2)
        XCTAssertFalse(log().contains("hatch route 0: changed"), "an unchanged route runs no hatch")
        XCTAssertTrue(items.elementsEqual(home.navigationItem.rightBarButtonItems ?? [], by: ===), "an unchanged route is not projected again")
        let more = home.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" }
        XCTAssertEqual(home.navigationItem.rightBarButtonItems?.count, 1, "Exact's Compose stays beside the hatch's item")
        // The hatch-made control clicks the authored one.
        try tap(more)
        until("the hatch's click pressed Compose") { state(session, "composed") as? Double == 1 }
        // A pushed route's hatch; a write to what Exact owns on its content
        // scroll view is journaled by name at the next batch; a route the
        // router drops ends its handle.
        let agent = Agent(session: session)
        _ = agent.tap(["id": Int(try node(session, "detail").id)])
        let key = try XCTUnwrap(try node(session, "route-detail").props["navigationKey"])
        until("detail built") { log().contains("hatch route \(key): built") && nav.transitionCoordinator == nil }
        let detail = try XCTUnwrap(nav.topViewController)
        XCTAssertNotNil(detail.navigationItem.leftBarButtonItems?.first { $0.accessibilityIdentifier == "hatch-more" })
        _ = agent.tap(["id": Int(try node(session, "violate").id)])
        until("the hatch saw the word") { log().contains("hatch route \(key): changed") }
        _ = agent.tap(["id": Int(try node(session, "violate").id)])
        until("the check journaled it") { log().contains("route \(key): contentInset changed outside Exact, which owns it") }
        nav.popViewController(animated: true)
        until("detail ended") { log().contains("hatch route \(key): ended") }
    }
    func testExplicitGroupsSplitGlassWithoutChangingButtonOrderOrDefaultGrouping() throws {
        let presenter = Presenter()
        defer { presenter.reset() }
        let header = NodeView(id: 100, kind: "view", presenter: presenter)
        header.props["semanticTag"] = "header"
        let route = RouteController(header)
        var buttons: [NodeView] = []
        for (groupID, names) in [(101, ["share"]), (102, ["favorite", "bookmark"]), (103, ["info"])] {
            let group = NodeView(id: UInt32(groupID), kind: "view", presenter: presenter)
            group.props["accessibilityRole"] = "group"
            header.container.addSubview(group)
            for name in names {
                let button = NodeView(id: UInt32(200 + buttons.count), kind: "button", presenter: presenter)
                button.props["testId"] = name
                group.container.addSubview(button)
                buttons.append(button)
            }
        }
        let shape = buttons.map { HeaderShape.Item($0) }
        XCTAssertEqual(shape.map(\.toolbarGroup), [101, 102, 102, 103])
        let items = presenter.navigation.barItems(shape, route)
        XCTAssertEqual(items.compactMap(\.accessibilityIdentifier), ["share", "favorite", "bookmark", "info"])
        if #available(iOS 26.0, *) {
            XCTAssertEqual(items.count, 6, "two system spaces split three groups")
            XCTAssertEqual(items[1].width, 0)
            XCTAssertEqual(items[4].width, 0)
        } else { XCTAssertEqual(items.count, 4) }
        for button in buttons { header.container.addSubview(button) }
        let plain = buttons.map { HeaderShape.Item($0) }
        XCTAssertNotEqual(shape[0].source, plain[0].source, "regrouping invalidates the projected bar")
        XCTAssertEqual(presenter.navigation.barItems(plain, route).count, 4)
    }

    func testActionsFocusAndBlurTheProjectedSearchField() throws {
        let session = try fixture("action-search", module: false)
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let search = try XCTUnwrap(nav.topViewController?.navigationItem.searchController)
        let source = try node(session, "header-search")
        source.props["id"] = "action-search"
        session.presenter.focusElement(["action-search"])
        XCTAssertTrue(search.searchBar.searchTextField.isFirstResponder)
        session.presenter.blurElement(["action-search"])
        XCTAssertFalse(search.searchBar.searchTextField.isFirstResponder)
    }

}

/// An app's delegate: a cross-fade, interactive when asked.
private final class Transitions: NSObject, UINavigationControllerDelegate, UIViewControllerAnimatedTransitioning {
    var interactive = false, animated = false, shown = 0
    let interaction = UIPercentDrivenInteractiveTransition()
    func navigationController(_ nav: UINavigationController, animationControllerFor operation: UINavigationController.Operation,
                              from: UIViewController, to: UIViewController) -> UIViewControllerAnimatedTransitioning? { self }
    func navigationController(_ nav: UINavigationController,
                              interactionControllerFor animator: UIViewControllerAnimatedTransitioning) -> UIViewControllerInteractiveTransitioning? {
        interactive ? interaction : nil
    }
    func navigationController(_ nav: UINavigationController, didShow controller: UIViewController, animated: Bool) { shown += 1 }
    func transitionDuration(using context: UIViewControllerContextTransitioning?) -> TimeInterval { 0.2 }
    func animateTransition(using context: UIViewControllerContextTransitioning) {
        animated = true
        guard let to = context.viewController(forKey: .to), let view = context.view(forKey: .to) else { return context.completeTransition(false) }
        view.frame = context.finalFrame(for: to)
        view.alpha = 0
        context.containerView.addSubview(view)
        UIView.animate(withDuration: transitionDuration(using: context), animations: { view.alpha = 1 }) { _ in
            context.completeTransition(!context.transitionWasCancelled)
        }
    }


}
#endif
