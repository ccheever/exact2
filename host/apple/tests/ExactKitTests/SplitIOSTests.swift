#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

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
        let unavailable = navigation.split?.empty.contentUnavailableConfiguration as? UIContentUnavailableConfiguration
        XCTAssertEqual(unavailable?.text, "No Selection", "nothing selected, the detail says so (LLP 1127 D7)")
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
    /// LLP 1127 D7: beside the detail the list's route is clear, so UIKit's sidebar ground shows behind it, as it
    /// shows behind a hand-built `UISplitViewController`'s and a SwiftUI `NavigationSplitView`'s sidebar list (both
    /// (230, 234, 238) on the iOS 27 iPad simulator); collapsed into a stack, the route paints the platform's
    /// background again.
    func testTheListsRouteIsClearOnTheSidebarsGround() throws {
        let session = try boot(size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let list = try XCTUnwrap(split.list.viewControllers.first as? RouteController)
        until("the list's route is clear beside the detail") { list.view.backgroundColor == .clear }
        split.controller.traitOverrides.horizontalSizeClass = .compact
        until("collapsed") { split.controller.isCollapsed && !agent.nativeInFlight() }
        until("the list's route paints the platform's background in the stack") {
            list.view.backgroundColor.map { $0.resolvedColor(with: list.traitCollection).cgColor.alpha == 1 } ?? false
        }
    }
    /// LLP 1127 D7: a press on a custom row's own view in the sidebar's plain list goes through its cell's selection
    /// (`GroupedListHost.select(carried:)`), as a tap on a row UIKit draws does: the book shows beside the list and its
    /// row stays selected there, and a press on the other row moves both.
    func testACustomRowsPressInTheSidebarSelectsItsCell() throws {
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id], "the plain list is UIKit's")
        let has = { (testId: String) in session.presenter.views.values.contains { $0.props["testId"] == testId && $0.window != nil } }
        for id in ["a", "b"] {
            let row = try node(session, "row-\(id)")
            until("row \(id) is carried into its cell") { list.cell(row.id).map { row.isDescendant(of: $0) } ?? false }
            let touch = RowTouch(row, at: CGPoint(x: 20, y: 10))
            row.touchesBegan([touch], with: nil)
            row.touchesEnded([touch], with: nil)
            until("book \(id) shows beside the list") { has("reading-\(id)") && !agent.nativeInFlight() }
            until("row \(id) stays selected beside it") { list.cell(row.id)?.isSelected == true }
            let other = try node(session, "row-\(id == "a" ? "b" : "a")")
            XCTAssertNotEqual(list.cell(other.id)?.isSelected, true, "the other row is not selected")
        }
    }

    /// LLP 1127: a move between the columns ends with either end. An iPhone Duo unfolding after a Back while folded
    /// has UIKit ask about a collapse within the expansion and end with the expansion alone; the detail then follows
    /// a row's press.
    func testAnExpansionThatAskedAboutACollapseEndsTheMove() throws {
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        // UIKit's calls, in its order, as measured on the iPhone Duo simulator (iOS 27.1)
        let c = split.controller
        _ = split.splitViewController(c, displayModeForExpandingToProposedDisplayMode: .oneBesideSecondary)
        _ = split.splitViewController(c, topColumnForCollapsingToProposedTopColumn: .primary)
        _ = split.splitViewController(c, displayModeForExpandingToProposedDisplayMode: .oneBesideSecondary)
        split.splitViewControllerDidExpand(c)
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        let a = try node(session, "row-a")
        until("row a is carried into its cell") { list.cell(a.id).map { a.isDescendant(of: $0) } ?? false }
        let touch = RowTouch(a, at: CGPoint(x: 20, y: 10))
        a.touchesBegan([touch], with: nil)
        a.touchesEnded([touch], with: nil)
        until("book a shows beside the list") { session.presenter.views.values.contains { $0.props["testId"] == "reading-a" && $0.window != nil } && !agent.nativeInFlight() }
    }

    /// A sidebar's selected row keeps its tinted ground, and its light content on it, while the window is
    /// inactive (the app in the background), where UIKit's list cell would turn the ground to a gray fill.
    func testASidebarsSelectedRowStaysTintedWhileTheWindowIsInactive() throws {
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        let row = try node(session, "row-a")
        until("row a is carried into its cell") { list.cell(row.id).map { row.isDescendant(of: $0) } ?? false }
        let touch = RowTouch(row, at: CGPoint(x: 20, y: 10))
        row.touchesBegan([touch], with: nil)
        row.touchesEnded([touch], with: nil)
        until("row a stays selected beside the book") { list.cell(row.id)?.isSelected == true && !agent.nativeInFlight() }
        let cell = try XCTUnwrap(list.cell(row.id))
        let ground = { () -> UIColor? in
            cell.layoutIfNeeded()
            return cell.backgroundConfiguration?.resolvedBackgroundColor(for: cell.tintColor).resolvedColor(with: cell.traitCollection)
        }
        let tinted = try XCTUnwrap(ground())
        XCTAssertEqual(tinted, cell.tintColor.resolvedColor(with: cell.traitCollection), "the selected row's ground is the tint")
        XCTAssertEqual(cell.contentView.overrideUserInterfaceStyle, .dark, "its content is light on the tint")
        try XCTUnwrap(window).traitOverrides.activeAppearance = .inactive
        until("the window is inactive") { cell.traitCollection.activeAppearance == .inactive }
        until("the ground stays the tint") { ground() == tinted }
        XCTAssertEqual(cell.contentView.overrideUserInterfaceStyle, .dark, "its content stays light")
    }

    /// LLP 1127 D7: a sidebar's selected row draws its label and secondary label in the colours UIKit's own cell
    /// content takes on the accent (white, and white at 0.8, on iOS 27.1), not the dark appearance's secondary
    /// label, a gray at 0.6; a row not selected keeps the label colours.
    func testASidebarsSelectedRowsTextIsInTheInkOfUIKitsCellContent() throws {
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        let a = try node(session, "row-a"), b = try node(session, "row-b")
        until("both rows are carried into their cells") { [a, b].allSatisfy { row in list.cell(row.id).map { row.isDescendant(of: $0) } ?? false } }
        let touch = RowTouch(a, at: CGPoint(x: 20, y: 10))
        a.touchesBegan([touch], with: nil)
        a.touchesEnded([touch], with: nil)
        until("row a is selected beside its book") { list.cell(a.id)?.isSelected == true && !agent.nativeInFlight() }
        let cell = try XCTUnwrap(list.cell(a.id) as? UICollectionViewListCell)
        let content = cell.defaultContentConfiguration().updated(for: cell.configurationState)
        let ink = { (testId: String) -> [Double] in
            guard let spec = (try? self.node(session, testId))?.paragraphSpec() else { return [] }
            return spec.runs.first?.color ?? spec.color
        }
        let text = channels(content.textProperties.resolvedColor(), cell.traitCollection)
        let secondary = channels(content.secondaryTextProperties.resolvedColor(), cell.traitCollection)
        until("row a's name is in the cell content's text ink \(text): \(ink("name-a"))") { near(ink("name-a"), text) }
        until("row a's preview is in its secondary text ink \(secondary): \(ink("preview-a"))") { near(ink("preview-a"), secondary) }
        let other = try XCTUnwrap(list.cell(b.id)).traitCollection
        XCTAssertTrue(near(ink("preview-b"), channels(.secondaryLabel, other)), "row b's preview keeps the secondary label: \(ink("preview-b"))")
    }
    private func channels(_ color: UIColor, _ traits: UITraitCollection) -> [Double] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        color.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a].map { Double(min(max($0, 0), 1)) * 255 }
    }
    private func near(_ a: [Double], _ b: [Double]) -> Bool { a.count == b.count && zip(a, b).allSatisfy { abs($0 - $1) < 1 } }

    /// LLP 1127: a row's preview, committed, opens its book as the row's press does, and the sidebar selects the
    /// row beside it in place of the one selected before. The row lifts as UIKit lifts a sidebar's cell, and the
    /// book that takes the shown one's place, keeping its key, is still the commit's to grow into. A later peek's
    /// text paints as its preview appears, though UIKit shows the preview through a transparent container.
    func testACommittedPreviewSelectsItsRowInTheSidebar() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted") }
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        let has = { (testId: String) in session.presenter.views.values.contains { $0.props["testId"] == testId && $0.window != nil } }
        let a = try node(session, "row-a"), b = try node(session, "row-b")
        until("both rows are carried into their cells") { [a, b].allSatisfy { row in list.cell(row.id).map { row.isDescendant(of: $0) } ?? false } }
        let touch = RowTouch(a, at: CGPoint(x: 20, y: 10))
        a.touchesBegan([touch], with: nil)
        a.touchesEnded([touch], with: nil)
        until("row a is selected beside its book") { has("reading-a") && list.cell(a.id)?.isSelected == true && !agent.nativeInFlight() }

        let menus = session.presenter.menus.context
        let interaction = try XCTUnwrap(b.interactions.compactMap { $0 as? UIContextMenuInteraction }.first, "the row's peek is UIKit's context menu")
        let configuration = try XCTUnwrap(menus.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: 20, y: 10)))
        let lifted = try XCTUnwrap(menus.contextMenuInteraction(interaction, previewForHighlightingMenuWithConfiguration: configuration))
        let cell = try XCTUnwrap(list.cell(b.id)), background = try XCTUnwrap(cell.backgroundConfiguration)
        XCTAssertTrue(lifted.view === cell, "the row lifts with its cell")
        XCTAssertGreaterThan(background.cornerRadius, 0, "a sidebar's cell has a rounded ground")
        let shape = lifted.parameters.visiblePath?.bounds ?? .null
        XCTAssertTrue(shape.insetBy(dx: -0.01, dy: -0.01).contains(cell.bounds) && cell.bounds.insetBy(dx: -0.01, dy: -0.01).contains(shape), "in its ground's shape: \(shape)")
        let owner = try XCTUnwrap(menus.open)
        _ = try XCTUnwrap(menus.lift(owner), "the preview")
        let commit = Commit()
        menus.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(commit.preferredCommitStyle, .pop, "the preview grows into book b")
        menus.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        until("book b shows beside the list") { has("reading-b") && !has("reading-a") && !agent.nativeInFlight() }
        until("row b is selected beside it") { list.cell(b.id)?.isSelected == true }
        XCTAssertNotEqual(list.cell(a.id)?.isSelected, true, "row a is not")

        // A later peek, of the other row: UIKit shows its preview through a container it keeps transparent, where
        // no text is found showing, so the preview's text paints as it appears, in its first frame
        let other = try XCTUnwrap(a.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let again = try XCTUnwrap(menus.contextMenuInteraction(other, configurationForMenuAtLocation: CGPoint(x: 20, y: 10)))
        _ = try XCTUnwrap(menus.contextMenuInteraction(other, previewForHighlightingMenuWithConfiguration: again))
        let preview = try XCTUnwrap(menus.open.flatMap { menus.lift($0) }, "row a's preview")
        let text = try XCTUnwrap(session.presenter.views.values.first { $0.isParagraph && $0.isDescendant(of: preview.view) }, "its text")
        let shown = UIView(frame: preview.view.frame), root = try XCTUnwrap(window?.rootViewController)
        shown.alpha = 0
        root.view.addSubview(shown)
        root.addChild(preview)
        shown.addSubview(preview.view)
        preview.didMove(toParent: root)
        root.view.layoutIfNeeded()
        XCTAssertNotNil(text.textRaster, "its text paints as it appears")
        menus.contextMenuInteraction(other, willEndFor: again, animator: nil)
        preview.willMove(toParent: nil)
        shown.removeFromSuperview()
        preview.removeFromParent()
    }
    /// LLP 1127: a sidebar's row lifts on the sidebar's ground, UIKit's material, as UIKit's own sidebar cell does,
    /// in light and dark, where the platter's default is the system background, white or black.
    func testASidebarsRowLiftsOnTheSidebarsGround() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted") }
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "the ground is read off the screen, which draws only a scene's window")
        }
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        let a = try node(session, "row-a")
        until("row a is carried into its cell") { list.cell(a.id).map { a.isDescendant(of: $0) } ?? false }
        let menus = session.presenter.menus.context
        let interaction = try XCTUnwrap(a.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        // The sidebar's material over the page, as in an app's window
        window?.rootViewController?.view.backgroundColor = .systemBackground
        for style in [UIUserInterfaceStyle.light, .dark] {
            window?.overrideUserInterfaceStyle = style
            // Each peek reads the ground off the screen, which shows the style once it has drawn
            until("in \(style == .dark ? "dark" : "light"), row a lifts on the sidebar's ground") {
                guard let cell = list.cell(a.id),
                      let configuration = menus.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: 20, y: 10)) else { return false }
                defer { menus.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil) }
                guard let lifted = menus.contextMenuInteraction(interaction, previewForHighlightingMenuWithConfiguration: configuration) else { return false }
                let platter = lifted.parameters.backgroundColor.resolvedColor(with: cell.traitCollection)
                let page = UIColor.systemBackground.resolvedColor(with: cell.traitCollection)
                var white: CGFloat = 0, pageWhite: CGFloat = 0
                platter.getWhite(&white, alpha: nil)
                page.getWhite(&pageWhite, alpha: nil)
                // Gray, neither the page's white nor its black
                return abs(white - pageWhite) > 0.02 && platter.cgColor.alpha == 1
            }
        }
    }
    /// LLP 1127: in a list that is not a sidebar (an iPhone's, a closed iPhone Duo's) a row's peek lifts its cell,
    /// its whole width past a side safe area, as UIKit lifts its own cells. UIKit asks for the preview about 0.18 s
    /// into a press (measured on iOS 27.1). The press stays a tap until the menu takes the touch, so a finger lifted
    /// in between opens the row.
    func testACollapsedListsRowLiftsWithItsCellAndATapThroughItsHighlightOpensIt() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted") }
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS")
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        XCTAssertTrue(try XCTUnwrap(navigation.split).controller.isCollapsed, "an iPhone's width collapses it")
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        let a = try node(session, "row-a")
        until("row a is carried into its cell") { list.cell(a.id).map { a.isDescendant(of: $0) } ?? false }
        let cell = try XCTUnwrap(list.cell(a.id))
        let touch = RowTouch(a, at: CGPoint(x: 20, y: 10))
        a.touchesBegan([touch], with: nil)
        until("the pressed row's cell is highlighted") { cell.isHighlighted }
        let menus = session.presenter.menus.context
        let interaction = try XCTUnwrap(a.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let configuration = try XCTUnwrap(menus.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: 20, y: 10)))
        let lifted = try XCTUnwrap(menus.contextMenuInteraction(interaction, previewForHighlightingMenuWithConfiguration: configuration))
        XCTAssertTrue(lifted.view === cell, "the row lifts with its cell")
        let shape = lifted.parameters.visiblePath?.bounds ?? .null
        XCTAssertTrue(shape.insetBy(dx: -0.01, dy: -0.01).contains(cell.bounds) && cell.bounds.insetBy(dx: -0.01, dy: -0.01).contains(shape), "its whole width: \(shape)")
        XCTAssertTrue(cell.isHighlighted, "the press holds while UIKit highlights the preview")
        a.touchesEnded([touch], with: nil)
        menus.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        until("the row's book shows") { session.presenter.views.values.contains { $0.props["testId"] == "reading-a" && $0.window != nil } && !agent.nativeInFlight() }
    }
    /// UIKit's commit animator, as the delegate sees it.
    private final class Commit: NSObject, UIContextMenuInteractionCommitAnimating {
        var preferredCommitStyle: UIContextMenuInteractionCommitStyle = .pop
        var previewViewController: UIViewController? { nil }
        func addAnimations(_ animations: @escaping () -> Void) { animations() }
        func addCompletion(_ completion: @escaping () -> Void) { completion() }
    }

    /// LLP 1127 D7: a plain list beside the detail is UIKit's plain sidebar list (`.sidebarPlain`), with lines
    /// between its rows, where a grouped one is the sidebar list, which draws none.
    func testASidebarsPlainListHasLinesBetweenItsRows() throws {
        ExactGroupedLists.install()
        let session = try boot("SPLIT_ROWS", size: CGSize(width: 1100, height: 800))
        let navigation = session.presenter.navigation, agent = Agent(session: session)
        until("a split") { navigation.split != nil && !agent.nativeInFlight() }
        let split = try XCTUnwrap(navigation.split)
        split.controller.traitOverrides.horizontalSizeClass = .regular
        until("expanded") { !split.controller.isCollapsed && !agent.nativeInFlight() }
        let host = try XCTUnwrap(session.presenter.groupedLists as? GroupedListHost)
        let list = try XCTUnwrap(host.lists[try node(session, "books").id])
        until("the list is in the sidebar") { list.sidebar }
        func lines(_ v: UIView) -> [UIView] {
            (String(describing: type(of: v)).contains("Separator") && !v.isHidden && v.alpha > 0 ? [v] : []) + v.subviews.flatMap(lines)
        }
        until("a line between its rows") {
            list.collection.layoutIfNeeded()
            return !lines(list.collection).isEmpty
        }
    }

    private final class RowTouch: UITouch {
        let target: UIView, point: CGPoint
        init(_ target: UIView, at point: CGPoint) { self.target = target; self.point = point; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(point, to: view) }
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
