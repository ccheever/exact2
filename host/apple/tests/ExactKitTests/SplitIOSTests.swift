#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1127 §1 on UIKit: collapsed, the split nests the detail's stack in
/// the list's, so a back swipe has both stacks' pop recognizers under the
/// finger. The detail's pops while it holds a route beneath its top; at its
/// root the list's pops, back to the list. The other must not begin, or it
/// takes the swipe and nothing goes back. UIKit, so a simulator runs it:
/// bun host/apple/build.mjs --test --ios
final class SplitIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        super.tearDown()
    }

    private func boot(_ fixture: String = "SPLIT_BACK", size: CGSize = CGSize(width: 402, height: 874)) throws -> ExactSession {
        if ExactEnv.authoredChrome { throw XCTSkip("under the agent's chrome the stack is not split") }
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_\(fixture)_PLAN"], "build.mjs --test --ios compiles the fixture")))
        let session = ExactApp.shared.makeSession(label: "split-\(UUID().uuidString)") // no state of another test's
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(origin: .zero, size: size))
        window.frame = CGRect(origin: .zero, size: size)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        return session
    }
    private func until(_ what: String, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(20)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(done(), what)
    }
    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }
    private func has(_ session: ExactSession, _ testId: String) -> Bool {
        session.presenter.views.values.contains { $0.props["testId"] == testId }
    }
    /// Whether the runner reads `testId`'s node as inactive (the agent's tree).
    private func inactive(_ session: ExactSession, _ testId: String) throws -> Bool {
        let id = try node(session, testId).id
        let tree = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"tree"}"#).utf8)) as? [String: Any])
        let rows = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
        return rows.first { ($0["id"] as? NSNumber)?.uint32Value == id }?["inactive"] as? Bool == true
    }

    func testACollapsedSplitsBackSwipeIsTheStackWithARouteBeneathItsTop() throws {
        let session = try boot()
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a list whose rows push is a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        XCTAssertTrue(split.controller.isCollapsed, "an iPhone's width collapses it")
        func begins(_ nav: UINavigationController) throws -> Bool {
            var pop = nav.interactivePopGestureRecognizer
            if #available(iOS 26.0, *) { pop = nav.interactiveContentPopGestureRecognizer ?? pop }
            return navigation.popMayBegin(try XCTUnwrap(pop), from: CGPoint(x: 100, y: 400), in: nav.view, velocity: CGPoint(x: 600, y: 20))
        }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is pushed") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertTrue(try begins(split.list), "at the detail's root the list's stack pops, back to the list")
        XCTAssertFalse(try begins(split.detail), "the detail's own has nothing beneath its root")

        XCTAssertNil(agent.tap(["id": Int(try node(session, "notes-a").id)])["error"])
        until("the notes are pushed") { split.shown.count == 3 && !agent.nativeInFlight() }
        XCTAssertTrue(try begins(split.detail), "the detail's stack pops its top")
        XCTAssertFalse(try begins(split.list), "the list's would take the whole detail stack")
    }

    /// Whether a stack splits is read again with each batch, its routes
    /// kept: a list whose rows come later makes a split, and a root that
    /// says `navigationSplit="none"` later makes a stack again.
    func testAStackSplitsWhenItsRowsComeAndPushesWhenItsRootSaysNone() throws {
        let session = try boot("SPLIT_CHANGE")
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("the stack is up") { navigation.primaryNavigation != nil && !agent.nativeInFlight() }
        XCTAssertNil(navigation.split, "no row pushes yet")
        let root = try XCTUnwrap(navigation.primaryNavigation?.viewControllers.first as? RouteController)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "fill").id)])["error"])
        until("the rows came: a split") { navigation.split != nil && !agent.nativeInFlight() }
        XCTAssertTrue(navigation.split?.list.viewControllers.first === root, "the same route, in the list's column")
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { navigation.split?.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertNil(agent.back()["error"])
        until("back at the list") { navigation.split?.shown.count == 1 && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "stop").id)])["error"])
        until("the root says none: a stack again") { navigation.split == nil && navigation.primaryNavigation != nil && !agent.nativeInFlight() }
        XCTAssertTrue(navigation.primaryNavigation?.viewControllers.first === root, "the same route, in a stack")
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-b").id)])["error"])
        until("a row pushes") { navigation.primaryNavigation?.viewControllers.count == 2 && !agent.nativeInFlight() }
    }

    /// LLP 1127 D8: beside the detail the list is live with it; while the
    /// list overlays the detail, only the list is; with the detail alone, or
    /// collapsed, only the detail.
    func testOnlyTheListIsLiveWhileItOverlaysTheDetail() throws {
        let session = try boot(size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        // The size class expands the split or collapses it, whatever the window's width.
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { split.shown.count == 2 && !agent.nativeInFlight() }
        let cases: [(UIUserInterfaceSizeClass, UISplitViewController.DisplayMode, UISplitViewController.SplitBehavior, Bool, Bool)] = [
            (.regular, .oneBesideSecondary, .tile, false, false),
            (.regular, .oneOverSecondary, .overlay, false, true),
            (.regular, .secondaryOnly, .tile, true, false),
            (.compact, .automatic, .automatic, true, false),
        ]
        for (size, mode, behavior, list, detail) in cases {
            split.controller.traitOverrides.horizontalSizeClass = size
            split.controller.preferredSplitBehavior = behavior
            split.controller.preferredDisplayMode = mode
            let shows = "\(size == .compact ? "collapsed" : "mode \(mode.rawValue)")"
            until("the split is \(shows)") { split.controller.isCollapsed == (size == .compact) && (size == .compact || split.controller.displayMode == mode) && !agent.nativeInFlight() }
            until("\(shows): the list \(list ? "in" : "")active, the detail \(detail ? "in" : "")active") {
                (try? inactive(session, "row-a")) == list && (try? inactive(session, "notes-a")) == detail
            }
            let facts = "beside \(split.beside.map(\.node.id)) under \(split.under.map(\.node.id)) blocks \(session.presenter.owners.told.keys.sorted())"
            XCTAssertEqual(try inactive(session, "row-a"), list, "the list, \(shows): \(facts)")
            XCTAssertEqual(try inactive(session, "notes-a"), detail, "the detail, \(shows): \(facts)")
            XCTAssertEqual(navigation.isInactiveRoute(containing: try node(session, "notes-a")), detail, "the host's, \(shows)")
        }
    }

    /// LLP 1127 D8: a sheet over the expanded split covers both columns
    /// first, the list beside the detail with them, which still show under
    /// it; a full-screen cover hides them. The inspector is the split's own
    /// column (D14): the list and the detail beside it stay live.
    func testAPresentationOverTheSplitCoversItsColumnsAndTheInspectorDoesNot() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("the inspector's column is iOS 26's") }
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet is presented only in a scene's window") }
        let session = try boot(size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertTrue(split.showsList, "the list beside the detail")
        func live(_ what: String, list: Bool, detail: Bool) throws {
            until("\(what): the list \(list ? "" : "not ")live, the detail \(detail ? "" : "not ")live") {
                (try? inactive(session, "row-a")) == !list && (try? inactive(session, "notes-a")) == !detail
            }
            XCTAssertEqual(navigation.isInactiveRoute(containing: try node(session, "row-a")), !list, "the host's list, \(what)")
            XCTAssertEqual(navigation.isInactiveRoute(containing: try node(session, "notes-a")), !detail, "the host's detail, \(what)")
        }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "share-a").id)])["error"])
        until("the sheet is up") { session.presenter.modals.routes.count == 1 && !agent.nativeInFlight() }
        XCTAssertTrue(split.showsList, "the list still beside the detail, under the sheet")
        try live("under a sheet", list: false, detail: false)
        let list = try XCTUnwrap(split.list.viewControllers.first as? RouteController)
        XCTAssertTrue(navigation.splitShown.under.contains { $0 === list }, "the list shows under the sheet")
        XCTAssertNil(agent.back()["error"])
        until("the sheet is gone") { session.presenter.modals.routes.isEmpty && !agent.nativeInFlight() }
        try live("the sheet gone", list: true, detail: true)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "cover-a").id)])["error"])
        until("the cover is up") { session.presenter.modals.routes.count == 1 && !agent.nativeInFlight() }
        try live("under a full-screen cover", list: false, detail: false)
        XCTAssertTrue(navigation.splitShown.beside.isEmpty && navigation.splitShown.under.isEmpty, "nothing of the split shows under the cover")
        XCTAssertNil(agent.back()["error"])
        until("the cover is gone") { session.presenter.modals.routes.isEmpty && !agent.nativeInFlight() }
        try live("the cover gone", list: true, detail: true)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "info-a").id)])["error"])
        until("its info is in the inspector's column") { split.inspecting != nil && !agent.nativeInFlight() }
        XCTAssertTrue(session.presenter.modals.routes.isEmpty, "no sheet")
        // Whether the list stays beside the inspector is UIKit's.
        try live("beside the inspector", list: split.showsList, detail: true)
    }
    /// LLP 1127 §1, item 2: UIKit takes the detail's controller out of the
    /// list's as the split expands (an iPhone Duo unfolded on a book's
    /// notes), and nests it again as it collapses. Neither is a Back: the
    /// book and its notes stay shown, and Back from them, collapsed, returns
    /// to the book and then the list. The size class changes on the window,
    /// as an unfolding display changes it: the split still reads compact
    /// when UIKit moves the detail out.
    func testExpandingAndCollapsingKeepTheDetail() throws {
        let session = try boot()
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        let has = { (testId: String) in session.presenter.views.values.contains { $0.props["testId"] == testId } }
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        XCTAssertTrue(split.controller.isCollapsed, "an iPhone's width collapses it")
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is pushed") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "notes-a").id)])["error"])
        until("its notes are pushed") { split.shown.count == 3 && !agent.nativeInFlight() }
        let window = try XCTUnwrap(self.window)
        window.frame = CGRect(x: 0, y: 0, width: 1100, height: 800)
        window.rootViewController?.view.subviews.first?.frame = window.bounds
        window.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        until("the book and its notes show beside the list") { split.shown.count == 3 && has("notes-a") }
        XCTAssertTrue(try inactive(session, "notes-a"), "expanded, the notes cover the book")
        window.traitOverrides.horizontalSizeClass = .compact
        until("collapsed") { split.controller.isCollapsed && !agent.nativeInFlight() }
        until("the notes are on top") { split.nested && split.shown.count == 3 && has("notes-a") }
        XCTAssertNil(agent.back()["error"])
        until("back at the book") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertFalse(try inactive(session, "notes-a"), "collapsed, the book is live")
        XCTAssertNil(agent.back()["error"])
        until("back at the list") { split.shown.count == 1 && !has("notes-a") && !agent.nativeInFlight() }
    }

    /// An iPhone Duo unfolding: UIKit asks which display mode to expand to
    /// while the split still reads compact, then takes the detail's
    /// controller out of the list's, its `didShow` arriving before the split
    /// says it expanded. That is no Back: the router keeps the book and its
    /// notes. The order is UIKit's on the Duo simulator, replayed here.
    func testTheDetailLeavingTheListAsUIKitExpandsIsNoBack() throws {
        let session = try boot()
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        let has = { (testId: String) in session.presenter.views.values.contains { $0.props["testId"] == testId } }
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is pushed") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "notes-a").id)])["error"])
        until("its notes are pushed") { split.shown.count == 3 && has("notes-page") && !agent.nativeInFlight() }
        XCTAssertTrue(split.nested)
        XCTAssertEqual(split.controller.traitCollection.horizontalSizeClass, .compact)
        _ = split.splitViewController(split.controller, displayModeForExpandingToProposedDisplayMode: .oneBesideSecondary)
        split.list.setViewControllers(Array(split.list.viewControllers.prefix(1)), animated: false)
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        XCTAssertTrue(has("notes-page") && has("notes-a"), "the router keeps the book and its notes")
    }

    /// LLP 1127 D14: Exact's close of the inspector's column empties the
    /// column once UIKit's transition is over. An inspector shown again
    /// before then supersedes the close: it keeps its route, and the column's
    /// hiding is no person's close.
    func testAnInspectorShownAgainWhileItClosesKeepsItsRoute() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("the inspector's column is iOS 26's") }
        let session = try boot(size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "info-a").id)])["error"])
        until("its info is in the inspector's column") { split.inspecting != nil && split.controller.transitionCoordinator == nil && !agent.nativeInFlight() }
        let route = try XCTUnwrap(split.inspecting), nav = try XCTUnwrap(split.inspector)
        var reported = 0
        let report = split.inspectorClosed
        split.inspectorClosed = { reported += 1; report() }
        split.inspect(nil, in: nil)
        let closing = try XCTUnwrap(split.controller.transitionCoordinator, "the column closes in a transition")
        // Registered after the close's own completion, so run after it.
        var closed = false
        closing.animate(alongsideTransition: nil) { _ in closed = true }
        split.inspect(route, in: nav)
        until("the close's transition is over") { closed && split.controller.transitionCoordinator == nil && !agent.nativeInFlight() }
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) // the delegate's reports, queued
        XCTAssertTrue(nav.viewControllers.first === route, "the inspector keeps its route: \(nav.viewControllers)")
        XCTAssertTrue(split.inspector === nav)
        XCTAssertTrue(split.controller.isShowing(.inspector), "the column shows again")
        XCTAssertEqual(reported, 0, "the superseded close is no person's")
    }

    /// The book, collapsed: UIKit's Back from it pops the detail's controller
    /// off the list's, which the person may also swipe (`InteractivePop`).
    private func collapsedBook(_ session: ExactSession, _ agent: Agent) throws -> SplitIOS {
        let navigation = session.presenter.navigation
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is pushed") { split.shown.count == 2 && split.nested && !agent.nativeInFlight() }
        return split
    }

    /// #1079's written-anew case in a collapsed split: during UIKit's Back
    /// from the book, the app writes the book anew under its key. The pop took
    /// the detail's controller, and the book with it, so the app goes back
    /// with UIKit rather than its pending sync pushing the book again.
    func testABackFromTheCollapsedDetailTakesTheAppBackAfterTheDetailWasWrittenAnew() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let session = try boot(), agent = Agent(session: session), navigation = session.presenter.navigation
        let split = try collapsedBook(session, agent), book = try node(session, "book-a")
        split.list.popViewController(animated: true)
        XCTAssertNotNil(split.list.transitionCoordinator, "the app writes the book anew before UIKit's pop is over")
        session.presenter.press(try node(session, "renew-a").id)
        XCTAssertFalse(try node(session, "book-a") === book, "the book written anew")
        until("the app went back from the book with UIKit") {
            split.list.transitionCoordinator == nil && !agent.nativeInFlight() && split.shown.count == 1 && !has(session, "book-a")
        }
        XCTAssertEqual(navigation.selectedKey, split.shown.first?.key, "the app at the list")
    }

    /// The same Back cancelled, as a swipe let go: UIKit popped nothing, so no
    /// Back reaches the app, and the book written anew takes the old one's place.
    func testACancelledBackFromTheCollapsedDetailKeepsTheDetailWrittenAnew() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let session = try boot(), agent = Agent(session: session), navigation = session.presenter.navigation
        let split = try collapsedBook(session, agent)
        let pop = InteractivePop()
        navigation.setAppDelegate(split.list, pop)
        defer { navigation.setAppDelegate(split.list, nil) }
        pop.interactive = true
        let library = try XCTUnwrap(split.list.viewControllers.first)
        split.list.popViewController(animated: true)
        until("UIKit's pop animates the list beneath") { library.viewIfLoaded?.window != nil }
        pop.interaction.update(0.4)
        session.presenter.press(try node(session, "renew-a").id)
        pop.interaction.cancel()
        pop.interactive = false
        let book = try node(session, "book-a")
        until("the book written anew is the detail") {
            split.list.transitionCoordinator == nil && !agent.nativeInFlight() && split.nested && (split.detail.topViewController as? RouteController)?.node === book
        }
        XCTAssertEqual(split.shown.count, 2, "the list and the book")
    }

    /// The app replaces the notes with another visit while the finger is down
    /// on a swipe in the detail's own stack: the app's stack counts the
    /// list's route beneath the detail's, as deep as when the swipe began, so
    /// the swipe's source was replaced in place and the app goes back from
    /// the replacement, rather than its pending sync pushing it where the
    /// person swiped the notes away.
    func testASwipeInTheDetailTakesTheAppBackAfterTheAppReplacedItsSource() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "UIKit animates a pop only in a window a scene holds") }
        let session = try boot(), agent = Agent(session: session), navigation = session.presenter.navigation
        let split = try collapsedBook(session, agent)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "notes-a").id)])["error"])
        until("the notes are pushed") { split.shown.count == 3 && !agent.nativeInFlight() }
        let pop = InteractivePop()
        navigation.setAppDelegate(split.detail, pop)
        defer { navigation.setAppDelegate(split.detail, nil) }
        pop.interactive = true
        let book = try XCTUnwrap(split.detail.viewControllers.first)
        split.detail.popViewController(animated: true)
        until("UIKit's pop animates the book beneath") { book.viewIfLoaded?.window != nil }
        pop.interaction.update(0.4)
        session.presenter.press(try node(session, "swap-a").id)
        XCTAssertTrue(has(session, "swap-z"), "the notes replaced with another visit")
        pop.interaction.finish()
        pop.interactive = false
        until("the app went back from the replacement, to the book") {
            split.detail.transitionCoordinator == nil && !agent.nativeInFlight() && split.shown.count == 2 && !has(session, "notes-page")
        }
        XCTAssertTrue(split.shown.last === book, "the book, where the person swiped")
    }

    /// LLP 1127 D14, as UIKit's own inspector comes and goes, on an iPhone
    /// Duo's width, where it hides the list: the column slides in over
    /// UIKit's animation with its route laid out where it ends, still, while
    /// the detail's content moves to its new width inside that animation and
    /// the list keeps its sidebar's look as it slides away. Closed, the
    /// column slides out with the route's last pixels, the detail moving back
    /// with it.
    func testTheInspectorsColumnMovesWithTheDetailAndKeepsItsRoute() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("the inspector's column is iOS 26's") }
        let session = try boot(size: CGSize(width: 951, height: 669))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { split.shown.count == 2 && !agent.nativeInFlight() }
        let books = try node(session, "books"), notes = try node(session, "notes-a")
        let wide = notes.frame
        XCTAssertNil(agent.tap(["id": Int(try node(session, "info-a").id)])["error"])
        until("its info is in the inspector's column") { split.inspecting != nil }
        let route = try XCTUnwrap(split.inspecting), info = try node(session, "info-page")
        let column = try XCTUnwrap(route.navigationController?.view.superview)
        let slide = try XCTUnwrap(Self.longest(column.layer), "the column slides in")
        XCTAssertEqual(info.frame.width, route.view.bounds.width, "its route is laid out at the column's width")
        for view in [info] + info.subviews { XCTAssertNil(Self.longest(view.layer), "its route is laid out still: \(view)") }
        let moving = try XCTUnwrap(Self.longest(notes.layer), "the detail's content moves inside the column's animation")
        XCTAssertEqual(moving.duration, slide.duration, accuracy: 0.01)
        XCTAssertNotEqual(notes.frame.width, wide.width, "to its width beside the inspector")
        XCTAssertFalse(split.showsList, "the inspector hides the list")
        XCTAssertTrue(session.presenter.showsAsSidebar(books), "the list keeps its sidebar's look as it slides away")

        until("the column is in") { split.controller.transitionCoordinator == nil && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "info-close").id)])["error"])
        until("the info's route is gone") { !session.presenter.views.values.contains { $0.props["testId"] == "info-page" } }
        XCTAssertTrue(route.view.window != nil && !route.view.subviews.isEmpty, "the column goes with the route's last pixels")
        XCTAssertNotNil(Self.longest(notes.layer), "the detail's content moves back inside the column's animation")
        until("closed") { split.controller.transitionCoordinator == nil && !agent.nativeInFlight() }
        XCTAssertEqual(notes.frame.width, wide.width, accuracy: 0.5)
    }

    /// LLP 1127 D14 on an iPhone Duo folded and unfolded with its inspector
    /// shown. Collapsing with the inspector shown, UIKit presents the
    /// inspector's column as a sheet of its own: the route goes to the sheet
    /// it says, which shows it (the column, emptied for it, was a blank sheet
    /// over it). Expanding, the sheet goes at once and the route is in its
    /// column, as UIKit's own inspector is from the expansion's first frame:
    /// never an empty column, a sheet sliding away or showing emptied, or a
    /// column sliding in.
    func testFoldedTheInspectorIsItsSheetAndUnfoldedItsColumnAtOnce() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("the inspector's column is iOS 26's") }
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet is presented only in a scene's window") }
        let session = try boot(size: CGSize(width: 951, height: 669))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        let window = try XCTUnwrap(self.window)
        window.traitOverrides.horizontalSizeClass = .regular
        until("a split, expanded") { navigation.split?.controller.isCollapsed == false && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "info-a").id)])["error"])
        until("its info is in the inspector's column") { split.inspecting != nil && split.controller.transitionCoordinator == nil && !agent.nativeInFlight() }
        let route = try XCTUnwrap(split.inspecting)

        window.traitOverrides.horizontalSizeClass = .compact
        until("folded, the info is a sheet") { split.controller.isCollapsed && session.presenter.modals.routes.count == 1 && !agent.nativeInFlight() }
        XCTAssertFalse(split.controller.presentedViewController === split.inspector, "UIKit presents no column of its own")
        XCTAssertTrue(route.navigationController.map { nav in session.presenter.modals.navigations.contains { $0 === nav } } == true, "the route is in its sheet")
        XCTAssertTrue(route.view.window != nil && route.node.isDescendant(of: route.view), "the sheet shows the route")

        window.traitOverrides.horizontalSizeClass = .regular
        var seen: [String] = []
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline, !(split.inspecting === route && session.presenter.modals.routes.isEmpty && !agent.nativeInFlight()) {
            if !split.controller.isCollapsed {
                if split.controller.isShowing(.inspector), split.inspecting == nil { seen.append("an empty column") }
                if window.rootViewController?.presentedViewController?.transitionCoordinator?.isAnimated == true { seen.append("the sheet sliding away") }
                if split.inspecting === route, let column = split.inspector?.view.superview, Self.longest(column.layer) != nil { seen.append("the column sliding in") }
                if split.inspecting === route, let sheet = window.rootViewController?.presentedViewController, sheet.view.window != nil,
                   sheet.presentationController?.containerView?.isHidden == false { seen.append("an emptied sheet over the column") }
            }
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005))
        }
        XCTAssertTrue(split.inspecting === route && session.presenter.modals.routes.isEmpty, "unfolded, the route is in its column and its sheet is gone")
        XCTAssertEqual(Set(seen), [], "unfolded, the column shows the route at once")
    }

    /// An iPhone Duo unfolding with its inspector in a sheet: a sync the
    /// expansion starts can find the split collapsing still, UIKit having
    /// asked which column to put on top and not yet said it collapsed. The
    /// sheet keeps the route until the column can take it: closed, the
    /// inspector was gone until a later sync slid the column in. The order is
    /// UIKit's on the Duo simulator, replayed here by asking before the
    /// expansion.
    func testAnInspectorsSheetKeepsItsRouteUntilTheColumnCanTakeIt() throws {
        guard #available(iOS 26.0, *) else { throw XCTSkip("the inspector's column is iOS 26's") }
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet is presented only in a scene's window") }
        let session = try boot(size: CGSize(width: 951, height: 669))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        let window = try XCTUnwrap(self.window)
        window.traitOverrides.horizontalSizeClass = .compact
        until("a split, collapsed") { navigation.split?.controller.isCollapsed == true && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is pushed") { split.shown.count == 2 && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "info-a").id)])["error"])
        until("its info is a sheet") { session.presenter.modals.routes.count == 1 && !agent.nativeInFlight() }
        let info = try node(session, "info-page")
        _ = split.splitViewController(split.controller, topColumnForCollapsingToProposedTopColumn: .secondary)
        window.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed }
        RunLoop.main.run(until: Date().addingTimeInterval(0.3)) // the syncs the expansion starts
        XCTAssertEqual(session.presenter.modals.routes.map(\.node), [info], "the sheet keeps the route")
        XCTAssertTrue(info.window != nil, "and shows it")
    }

    /// The longest animation on `layer`, if any.
    private static func longest(_ layer: CALayer) -> CAAnimation? {
        (layer.animationKeys() ?? []).compactMap { layer.animation(forKey: $0) }.max { $0.duration < $1.duration }
    }

    /// As UIKit collapses the split it puts the detail's controller in the
    /// list's while the split still reads expanded. A sync meanwhile leaves
    /// the list's stack to UIKit: reset to its root, UIKit pushed the detail
    /// twice and asserted (an iPhone Duo folded on a book). The order is
    /// UIKit's on the Duo simulator, replayed here.
    func testASyncWhileUIKitCollapsesLeavesTheListsStackToIt() throws {
        let session = try boot(size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        XCTAssertNil(agent.tap(["id": Int(try node(session, "row-a").id)])["error"])
        until("the book is in the detail") { split.shown.count == 2 && !agent.nativeInFlight() }
        let stack = split.shown
        _ = split.splitViewController(split.controller, topColumnForCollapsingToProposedTopColumn: .primary)
        let moving = UIViewController()
        split.list.setViewControllers(split.list.viewControllers + [moving], animated: false)
        split.show(stack, animated: false)
        XCTAssertTrue(split.list.viewControllers.last === moving, "the list's stack is UIKit's while it collapses")
    }
}
#endif
