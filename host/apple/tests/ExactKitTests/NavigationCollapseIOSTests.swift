#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1075.003 Stage 3: a large title collapses with its route's content
/// scroll view (the fixture's Second tab). CSS `scrollTop` counts from the
/// scrollport's top, which under the bar is the bar's bottom whatever its
/// height — UIKit keeps the offset plus that inset fixed as the title
/// collapses — so an authored offset lands where the browser's does (0, 80,
/// the middle, the end of what shows). At rest, CSS 0, the title is
/// expanded; an authored 0 after a collapse lands at 0 under the bar as it
/// is, and the title expands when the user pulls.
/// Gestures (a finger's collapse, pull to top, a held sheet) need real
/// touches, which a unit test has none of.
final class NavigationCollapseIOSTests: XCTestCase {
    private var window: UIWindow?
    private var exactView: ExactView?
    private var sessions: [ExactSession] = []
    private var animationsWereEnabled = true

    override func setUp() {
        super.setUp()
        // Hostless UIKit does not finish an animated tab selection. Keep
        // normal controller containment and make that transition synchronous.
        animationsWereEnabled = UIView.areAnimationsEnabled
        UIView.setAnimationsEnabled(false)
    }

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        exactView = nil
        UIView.setAnimationsEnabled(animationsWereEnabled)
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { spin(0.02) }
        XCTAssertTrue(done(), what)
    }

    private func fixture(_ label: String, planKey: String = "EXACT_FIXTURE_PLAN") throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[planKey])))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        exactView = view
        let host = UIViewController()
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
        host.beginAppearanceTransition(true, animated: false)
        host.endAppearanceTransition()
        spin(0.3)
        return session
    }

    private func second(_ session: ExactSession) throws -> (UINavigationController, RouteController, NodeView) {
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        _ = tabs.delegate?.tabBarController?(tabs, shouldSelect: tabs.viewControllers![1])
        until("Second selected") { tabs.selectedIndex == 1 }
        tabs.view.setNeedsLayout()
        tabs.view.layoutIfNeeded()
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let node = try XCTUnwrap(session.presenter.views.values.first {
            $0.props["testId"] == "list-second" && $0.isDescendant(of: route.node)
        })
        until("Second content is mounted in the selected route") { node.window === self.window }
        spin(0.3)
        return (nav, route, node)
    }

    /// iOS 26 puts the large title in the scroll content; the bar itself
    /// keeps its inline height. Inspect public label/geometry properties,
    /// without depending on UIKit's private view names.
    private func largeTitle(in nav: UINavigationController, title: String?) throws -> UILabel {
        var candidates: [UILabel] = []
        func walk(_ view: UIView) {
            if let label = view as? UILabel, label.text == title { candidates.append(label) }
            for child in view.subviews { walk(child) }
        }
        walk(nav.view)
        let label = try XCTUnwrap(candidates.max { $0.font.pointSize < $1.font.pointSize }, "the navigation controller contains its native large title")
        XCTAssertGreaterThan(label.font.pointSize, UIFont.preferredFont(forTextStyle: .headline).pointSize, "the inline title is not the large title")
        return label
    }

    private func visibleTitleHeight(_ label: UILabel, in nav: UINavigationController) -> CGFloat {
        guard let window = label.window else { return 0 }
        var rect = label.convert(label.bounds, to: window)
        var current: UIView? = label
        while let view = current {
            if view.isHidden || view.alpha < 0.01 { return 0 }
            if view.clipsToBounds { rect = rect.intersection(view.convert(view.bounds, to: window)) }
            current = view.superview
        }
        // A large title may belong to the expanded navigation bar itself.
        // UIKit's ancestor visibility/clipping is the authority on both designs.
        let visible = rect.intersection(window.bounds)
        return visible.isNull ? 0 : visible.height
    }

    private final class NativeRoute: UIViewController {
        let scroll: UIScrollView
        init(scroll: UIScrollView = UIScrollView()) {
            self.scroll = scroll
            super.init(nibName: nil, bundle: nil)
        }
        required init?(coder: NSCoder) { nil }
        override func loadView() {
            view = UIView()
            scroll.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            view.addSubview(scroll)
        }
        override func viewDidLayoutSubviews() {
            super.viewDidLayoutSubviews()
            scroll.frame = view.bounds
        }
    }

    func testNativeUIKitInitialLargeTitleGeometry() throws {
        guard #available(iOS 26, *) else { return }
        for adjustment in [UIScrollView.ContentInsetAdjustmentBehavior.automatic, .always] {
            let route = NativeRoute()
            route.title = "Native Second"
            route.navigationItem.largeTitleDisplayMode = .always
            let nav = UINavigationController(rootViewController: route)
            nav.navigationBar.prefersLargeTitles = true
            route.loadViewIfNeeded()
            route.scroll.contentSize = CGSize(width: 402, height: 2400)
            route.scroll.contentInsetAdjustmentBehavior = adjustment
            route.setContentScrollView(route.scroll, for: .top)
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
            self.window = window
            window.rootViewController = nav
            window.makeKeyAndVisible()
            nav.beginAppearanceTransition(true, animated: false)
            nav.endAppearanceTransition()
            nav.view.layoutIfNeeded()
            spin(0.3)
            let label = try largeTitle(in: nav, title: route.title)
            XCTAssertGreaterThan(visibleTitleHeight(label, in: nav), 0, "UIKit starts with its large title visible")
            route.scroll.setContentOffset(CGPoint(x: 0, y: -route.scroll.adjustedContentInset.top), animated: false)
            spin(0.3)
            window.isHidden = true
            self.window = nil
        }
    }

    func testNativeUIKitCollectionScrollWriteSurvivesItsNextLayout() throws {
        guard #available(iOS 26, *) else { return }
        for adjustment in [UIScrollView.ContentInsetAdjustmentBehavior.automatic, .always] {
            let layout = UICollectionViewCompositionalLayout.list(using: UICollectionLayoutListConfiguration(appearance: .insetGrouped))
            let collection = UICollectionView(frame: .zero, collectionViewLayout: layout)
            let cells = UICollectionView.CellRegistration<UICollectionViewListCell, Int> { cell, _, item in
                var config = cell.defaultContentConfiguration()
                config.text = "Item \(item + 1)"
                cell.contentConfiguration = config
                cell.insetsLayoutMarginsFromSafeArea = false
                cell.contentView.insetsLayoutMarginsFromSafeArea = false
                for content in cell.contentView.subviews { content.insetsLayoutMarginsFromSafeArea = false }
            }
            let source = UICollectionViewDiffableDataSource<Int, Int>(collectionView: collection) { view, path, item in
                view.dequeueConfiguredReusableCell(using: cells, for: path, item: item)
            }
            defer { withExtendedLifetime(source) {} }
            var snapshot = NSDiffableDataSourceSnapshot<Int, Int>()
            snapshot.appendSections([0])
            snapshot.appendItems(Array(0..<40))
            source.apply(snapshot, animatingDifferences: false)
            let route = NativeRoute(scroll: collection)
            route.title = "Native Collection"
            route.navigationItem.largeTitleDisplayMode = .always
            let nav = UINavigationController(rootViewController: route)
            nav.navigationBar.prefersLargeTitles = true
            collection.contentInsetAdjustmentBehavior = adjustment
            route.setContentScrollView(collection, for: .top)
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
            self.window = window
            window.rootViewController = nav
            window.makeKeyAndVisible()
            nav.beginAppearanceTransition(true, animated: false)
            nav.endAppearanceTransition()
            nav.view.layoutIfNeeded()
            collection.layoutIfNeeded()
            spin(0.3)
            let label = try largeTitle(in: nav, title: route.title)
            XCTAssertGreaterThan(visibleTitleHeight(label, in: nav), 0)
            let initialInset = collection.adjustedContentInset.top
            collection.setContentOffset(CGPoint(x: 0, y: 80 - initialInset), animated: false)
            if collection.adjustedContentInset.top != initialInset {
                collection.layoutIfNeeded()
                collection.setContentOffset(CGPoint(x: 0, y: 80 - collection.adjustedContentInset.top), animated: false)
            }
            collection.layoutIfNeeded()
            spin(0.3)
            XCTAssertEqual(collection.contentOffset.y + collection.adjustedContentInset.top, 80, accuracy: 0.5)
            window.isHidden = true
            self.window = nil
        }
    }

    func testAnAuthoredScrollTopLandsAsTheBrowsersWhileTheTitleCollapses() throws {
        let session = try fixture("collapse")
        let (nav, route, node) = try second(session)
        let sv = try XCTUnwrap(node.scrollView)
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("collapses its title with its scroller"))
        XCTAssertGreaterThan(node.scrollOrigin, 0, "the expanded inset is CSS 0")
        let css = { sv.contentOffset.y + sv.adjustedContentInset.top }
        // This baseline plain-scroll fixture starts inline on iOS 26.
        // Native-list cold title geometry is covered separately below;
        // this test checks the authored offset basis on both UIKit designs.
        let expanded = nav.navigationBar.frame.height
        XCTAssertEqual(css(), 0, accuracy: 0.5, "at rest, CSS 0")
        func assign(_ top: Double) {
            node.pendingScrollTop = top
            node.applyPendingScroll()
            spin(0.3)
        }
        assign(80)
        XCTAssertEqual(css(), 80, accuracy: 0.5, "80 lands at 80")
        if #unavailable(iOS 26) { XCTAssertLessThan(nav.navigationBar.frame.height, expanded, "and the title collapsed") }
        assign(300)
        XCTAssertEqual(css(), 300, accuracy: 0.5, "the middle lands")
        assign(0)
        XCTAssertEqual(css(), 0, accuracy: 0.5, "0 lands at 0 (the title expands when the user pulls)")
        assign(100_000)
        let end = sv.contentSize.height + sv.adjustedContentInset.bottom - sv.bounds.height + sv.adjustedContentInset.top
        XCTAssertEqual(css(), end, accuracy: 0.5, "the end clamps to the scroller's own end")
        // A user's offset reads back in CSS terms, as `layout` reports it.
        sv.setContentOffset(CGPoint(x: 0, y: 30 - sv.adjustedContentInset.top), animated: false)
        spin(0.2)
        let layout = Agent(session: session).layout([:])
        let row = (layout["nodes"] as? [[String: Any]])?.first { ($0["id"] as? Int) == Int(node.id) }
        XCTAssertEqual(row?["sy"] as? Double ?? -1, 30, accuracy: 0.5)
    }
    private final class NativeDelegate: NSObject, UICollectionViewDelegate {}

    func testReplacingAScrollersBackendRebindsTheNavigationAndHatchObjects() throws {
        let session = try fixture("collapse-native-backend")
        let (nav, route, node) = try second(session)
        let navigation = session.presenter.navigation
        let plain = try XCTUnwrap(node.scrollView)
        XCTAssertTrue(route.contentScrollView(for: .top) === plain)
        node.groupedOwner = true
        node.syncScroll()
        XCTAssertNil(node.scroll, "the native owner retires its plain scroll container")
        let delegate = NativeDelegate()
        func mount() -> UICollectionView {
            let collection = UICollectionView(frame: node.bounds, collectionViewLayout: UICollectionViewFlowLayout())
            collection.delegate = delegate
            node.addSubview(collection)
            node.mountNativeScrollView(collection)
            navigation.prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
            return collection
        }
        let collection = mount()
        XCTAssertTrue(navigation.contentScroll(of: route) === collection, "route hatches receive the actual collection")
        XCTAssertTrue(ElementHatches.platform(of: node, session.presenter) === collection, "element hatches use that backend too")
        XCTAssertTrue(route.contentScrollView(for: .top) === collection, "UIKit follows the actual collection")
        XCTAssertTrue(route.collapseScroll === node)
        XCTAssertTrue(route.collapseScrollView === collection)
        XCTAssertEqual(plain.contentInsetAdjustmentBehavior, .never)
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .always)
        collection.contentInset.bottom = 40
        XCTAssertTrue(NavigationHost.ownedChanges(collection, of: node, collapsing: true).isEmpty, "the collection retains its Exact-owned delegate and native footer insets")
        let changedDelegate = NativeDelegate()
        collection.delegate = changedDelegate
        XCTAssertTrue(NavigationHost.ownedChanges(collection, of: node, collapsing: true).contains("delegate"), "a hatch replacing that delegate is detected")
        collection.delegate = delegate
        node.scrollOrigin = 123
        node.scrollCollapsed = 45
        collection.removeFromSuperview()
        let replacement = mount()
        XCTAssertTrue(route.collapseScroll === node, "the logical owner did not change")
        XCTAssertTrue(route.contentScrollView(for: .top) === replacement, "a new backend on the same node is rebound")
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .never)
        XCTAssertEqual(replacement.contentInsetAdjustmentBehavior, .always)
        XCTAssertEqual(node.scrollOrigin, 0, "the previous backend's title inset is not carried over")
        XCTAssertEqual(node.scrollCollapsed, 0)
        XCTAssertTrue(navigation.contentScroll(of: route) === replacement)
    }

    private func nativeList(initialTop: Double? = 0, paddingTop: Double = 0, laterAuthoredTop: Double? = nil, customHero: Bool = false, heroHeight: Double = 80, followEnd: Bool = false) throws -> (ExactSession, UINavigationController, RouteController, NodeView) {
        ExactGroupedLists.install()
        let session = try fixture("collapse-native-list")
        let (nav, route, original) = try second(session)
        let p = session.presenter
        // Exercise host projection against a real route, with model/geometry
        // supplied by this test rather than the fixture's running kernel.
        p.onCovers = nil
        p.onScrolled = nil
        let host = try XCTUnwrap(p.groupedLists as? GroupedListHost)
        let listID: UInt32 = 900_000
        let sectionID = listID + 1
        let rowIDs = (0..<40).map { listID + 2 + UInt32($0) }
        let heroID = listID + 100
        var rows = rowIDs.enumerated().map { GroupedListModel.Row(view: $0.element, title: "Item \($0.offset + 1)", pressable: true) }
        if customHero { rows.insert(.init(view: heroID, custom: true), at: 0) }
        host.model = { id in
            id == listID ? GroupedListModel(sections: [.init(view: sectionID, rows: rows)]) : nil
        }
        let children = route.node.container.subviews.compactMap { ($0 as? NodeView).map { $0 === original ? listID : $0.id } }
        let frame = original.frame
        var listProps = ["id": "list-second", "listStyle": "inset-grouped"]
        if let initialTop { listProps["scrollTop"] = String(initialTop) }
        if followEnd { listProps["scrollFollowEnd"] = "true" }
        var ops: [[String: Any]] = [
            ["op": "props", "id": original.id, "clear": ["id"]],
            ["op": "create", "id": listID, "kind": "list", "props": listProps, "style": ["overflow_y": "scroll", "padding_top": paddingTop]],
            ["op": "create", "id": sectionID, "kind": "view", "props": ["semanticTag": "section"]],
        ]
        ops += rowIDs.map { ["op": "create", "id": $0, "kind": "button", "handlers": ["press"]] }
        if customHero {
            ops.append(["op": "create", "id": heroID, "kind": "view", "props": ["testId": "native-nav-hero"]])
            ops.append(["op": "frame", "id": heroID, "x": 0.0, "y": 0.0, "w": 370.0, "h": heroHeight])
        }
        ops += [
            ["op": "children", "id": listID, "ids": [sectionID]],
            ["op": "children", "id": sectionID, "ids": customHero ? [heroID] + rowIDs : rowIDs],
        ]
        ops += [
            ["op": "children", "id": route.node.id, "ids": children],
            ["op": "frame", "id": listID, "x": Double(frame.minX), "y": Double(frame.minY), "w": Double(frame.width), "h": Double(frame.height)],
        ]
        if let laterAuthoredTop {
            ops.append(["op": "props", "id": listID, "set": ["scrollTop": String(laterAuthoredTop)]])
        }
        p.apply(wireBatch(ops))
        let node = try XCTUnwrap(p.views[listID])
        let list = try XCTUnwrap(host.lists[listID])
        let collection = list.collection
        nav.view.layoutIfNeeded()
        collection.layoutIfNeeded()
        spin(0.3)
        return (session, nav, route, node)
    }

    private func showBottomToolbar(_ nav: UINavigationController, _ route: RouteController) {
        route.toolbarItems = [UIBarButtonItem(title: "Action", style: .plain, target: nil, action: nil)]
        nav.setToolbarHidden(false, animated: false)
        nav.view.setNeedsLayout()
        nav.view.layoutIfNeeded()
        route.host?.reportCovers()
        spin(0.3)
    }

    func testFullHeightPlainScrollOwnsTheNativeBottomInsetAndReachesItsEnd() throws {
        let session = try fixture("bottom-plain")
        let (nav, route, node) = try second(session)
        showBottomToolbar(nav, route)
        let scroll = try XCTUnwrap(node.scrollView)
        until("the plain port reaches the route bottom") {
            abs(node.frame.maxY - route.node.contentBox().maxY) <= 0.5
        }
        XCTAssertTrue(route.contentScrollView(for: .bottom) === scroll)
        XCTAssertGreaterThan(scroll.adjustedContentInset.bottom - scroll.contentInset.bottom, 0)
        let frame = node.convert(node.bounds, to: route.view)
        XCTAssertGreaterThan(frame.maxY, route.view.bounds.maxY - route.view.safeAreaInsets.bottom,
                             "the physical port extends behind the native bar")
        var metrics: [Double]?
        session.presenter.onScroll = { id, values in if id == node.id { metrics = values } }
        node.handlers.insert("scroll")
        node.pendingScrollTop = 100_000
        node.applyPendingScroll()
        spin(0.1)
        let event = try XCTUnwrap(metrics)
        let room = node.scrollPortInsets(scroll)
        XCTAssertEqual(event[5], Double(scroll.bounds.height - room.top - room.bottom), accuracy: 0.5)
        XCTAssertEqual(event[3] - event[1] - event[5], 0, accuracy: 0.5, "logical scroll metrics reach the same visible end")
        let end = scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height
        XCTAssertEqual(scroll.contentOffset.y, end, accuracy: 0.5)
        let bottom = scroll.convert(CGPoint(x: 0, y: scroll.contentSize.height), to: route.view).y
        XCTAssertEqual(bottom, frame.maxY - scroll.adjustedContentInset.bottom, accuracy: 0.5,
                       "content reaches the visible end with one native obstruction inset")
    }

    func testFullHeightGroupedScrollUsesTheSameBottomOwnerAndVisibleEnd() throws {
        let (session, nav, route, node) = try nativeList(initialTop: nil)
        showBottomToolbar(nav, route)
        let scroll = try XCTUnwrap(node.scrollView)
        XCTAssertTrue(route.host?.bottomScroll(of: route) === scroll)
        XCTAssertTrue(route.contentScrollView(for: .bottom) === scroll)
        XCTAssertGreaterThan(scroll.adjustedContentInset.bottom - scroll.contentInset.bottom, 0)
        var metrics: [Double]?
        session.presenter.onScroll = { id, values in if id == node.id { metrics = values } }
        node.handlers.insert("scroll")
        node.pendingScrollTop = 100_000
        node.applyPendingScroll()
        scroll.layoutIfNeeded()
        spin(0.1)
        let event = try XCTUnwrap(metrics)
        let room = node.scrollPortInsets(scroll)
        XCTAssertEqual(event[5], Double(scroll.bounds.height - room.top - room.bottom), accuracy: 0.5)
        XCTAssertEqual(event[3] - event[1] - event[5], 0, accuracy: 0.5, "grouped scroll metrics use one native bottom inset")
        let end = scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height
        XCTAssertEqual(scroll.contentOffset.y, end, accuracy: 0.5)
        let bottom = scroll.convert(CGPoint(x: 0, y: scroll.contentSize.height), to: route.view).y
        let port = node.convert(node.bounds, to: route.view)
        XCTAssertEqual(bottom, port.maxY - scroll.adjustedContentInset.bottom, accuracy: 0.5)
    }

    func testAnAppBottomScrollOverrideSurvivesUnchangedRouteProjection() throws {
        let session = try fixture("bottom-hatch-owner")
        let (nav, route, node) = try second(session)
        showBottomToolbar(nav, route)
        XCTAssertTrue(route.bottomScrollView === node.scrollView)
        let appScroll = UIScrollView()
        route.setContentScrollView(appScroll, for: .bottom)
        session.presenter.navigation.prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
        session.presenter.navigation.reportCovers()
        XCTAssertTrue(route.contentScrollView(for: .bottom) === appScroll,
                      "an unchanged Exact binding does not replace the app's native override")
        XCTAssertTrue(route.bottomScrollView === node.scrollView)
    }

    func testAFlowFooterKeepsItsBottomCoverAndChangingFlowRebindsTheOwner() throws {
        let session = try fixture("bottom-footer")
        let (nav, route, node) = try second(session)
        let p = session.presenter
        showBottomToolbar(nav, route)
        p.onScrolled = nil
        var bottomCover: CGFloat?
        p.onCovers = { changes in
            for (id, cover) in changes where id == route.node.id {
                if case .edges(let edges)? = cover { bottomCover = edges.bottom }
            }
        }
        let children = route.node.container.subviews.compactMap { ($0 as? NodeView)?.id }
        let footer: UInt32 = 950_000
        let visibleEnd = route.view.bounds.maxY - route.view.safeAreaInsets.bottom
        let footerEnd = route.node.container.convert(CGPoint(x: 0, y: visibleEnd), from: route.view).y
        let footerY = footerEnd - 40
        p.apply(wireBatch([
            ["op": "create", "id": footer, "kind": "view"],
            ["op": "children", "id": route.node.id, "ids": children + [footer]],
            ["op": "frame", "id": node.id, "x": Double(node.frame.minX), "y": Double(node.frame.minY),
             "w": Double(node.frame.width), "h": Double(max(0, footerY - node.frame.minY))],
            ["op": "frame", "id": footer, "x": 0.0, "y": Double(footerY), "w": Double(node.frame.width), "h": 40.0],
        ]))
        XCTAssertTrue(p.flats.isFlat(footer), "an inert footer may be drawn as a layer without a NodeView")
        XCTAssertNil(p.navigation.bottomScroll(of: route))
        XCTAssertNil(route.bottomScrollView, "Exact releases its explicit binding even if UIKit finds a heuristic owner")
        p.navigation.reportCovers()
        nav.view.layoutIfNeeded()
        node.scrollView?.layoutIfNeeded()
        spin(0.1)
        XCTAssertGreaterThan(bottomCover ?? 0, 0, "a fixed flow footer keeps the route's kernel cover")
        let footerFrame = route.node.container.convert(try XCTUnwrap(p.flats.leaves[footer]).frame, to: route.view)
        XCTAssertLessThanOrEqual(footerFrame.maxY, visibleEnd + 0.5, "the covered layout keeps the footer above the native bar")
        XCTAssertLessThanOrEqual(node.convert(node.bounds, to: route.view).maxY, footerFrame.minY + 0.5)
        let scroll = try XCTUnwrap(node.scrollView)
        XCTAssertEqual(scroll.adjustedContentInset.bottom - scroll.contentInset.bottom, 0, accuracy: 0.5,
                       "the covered scroll port ending above its footer gets no duplicate native bottom inset")
        p.apply(wireBatch([["op": "style", "id": footer, "style": ["display": "none"]]]))
        XCTAssertTrue(route.contentScrollView(for: .bottom) === node.scrollView,
                      "the same backend is rebound when its footer leaves flow")
        p.apply(wireBatch([["op": "style", "id": footer, "style": ["display": "block", "visibility": "hidden"]]]))
        XCTAssertNil(p.navigation.bottomScroll(of: route), "visibility:hidden still takes flow space")
        for position in ["absolute", "fixed"] {
            p.apply(wireBatch([["op": "style", "id": footer, "style": ["position_type": position]]]))
            XCTAssertTrue(route.contentScrollView(for: .bottom) === node.scrollView, "out-of-flow overlays do not reserve a footer")
        }
        p.apply(wireBatch([["op": "style", "id": footer, "style": [:]],
                          ["op": "props", "id": footer, "set": ["popover": "auto"]]]))
        XCTAssertTrue(route.contentScrollView(for: .bottom) === node.scrollView, "a top-layer popover is not a flow footer")
        p.apply(wireBatch([["op": "props", "id": footer, "clear": ["popover"]]]))
        let ghost = try XCTUnwrap(p.views[footer])
        XCTAssertNil(p.navigation.bottomScroll(of: route))
        XCTAssertNil(route.bottomScrollView)
        p.apply(wireBatch([["op": "exit", "id": footer],
                          ["op": "children", "id": route.node.id, "ids": children]]))
        XCTAssertTrue(ghost.superview === route.node.container, "the exit still paints in its old slot")
        XCTAssertNil(p.views[footer], "the exiting footer already left live flow")
        XCTAssertTrue(route.contentScrollView(for: .bottom) === node.scrollView,
                      "an exit ghost does not reserve the route's bottom cover")
    }

    func testShortFixedPlainPortKeepsItsFrameWithoutAFictitiousBottomInset() throws {
        let session = try fixture("bottom-short-port")
        let (nav, route, node) = try second(session)
        showBottomToolbar(nav, route)
        let p = session.presenter
        p.onCovers = nil
        p.onScrolled = nil
        p.apply(wireBatch([["op": "frame", "id": node.id, "x": Double(node.frame.minX),
                            "y": Double(node.frame.minY), "w": Double(node.frame.width), "h": 120.0]]))
        let scroll = try XCTUnwrap(node.scroll)
        scroll.layoutIfNeeded()
        XCTAssertTrue(route.contentScrollView(for: .bottom) === scroll)
        XCTAssertEqual(node.bounds.height, 120, "the native edge owner does not enlarge a fixed authored port")
        XCTAssertEqual(scroll.bounds.height, 120)
        XCTAssertEqual(scroll.adjustedContentInset.bottom - scroll.contentInset.bottom, 0, accuracy: 0.5,
                       "UIKit adds no bottom inset to a port ending above the bar")
        node.content = CGSize(width: node.bounds.width, height: 40)
        node.fitScroll()
        XCTAssertEqual(scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height, 0, accuracy: 0.5,
                       "a short port gains no empty bottom scroll range")
    }

    func testFixedPortMatchingTheFormerCoveredHeightUsesItsActualOverlap() throws {
        let session = try fixture("bottom-fixed-covered-height")
        let (nav, route, node) = try second(session)
        showBottomToolbar(nav, route)
        let p = session.presenter
        p.onCovers = nil
        p.onScrolled = nil
        let formerCover = max(0, route.view.safeAreaInsets.bottom - p.insets.bottom)
        let fixedHeight = route.node.contentBox().height - formerCover
        p.apply(wireBatch([["op": "frame", "id": node.id, "x": Double(node.frame.minX),
                            "y": Double(node.frame.minY), "w": Double(node.frame.width), "h": Double(fixedHeight)]]))
        let scroll = try XCTUnwrap(node.scroll)
        nav.view.layoutIfNeeded()
        scroll.layoutIfNeeded()
        XCTAssertTrue(route.contentScrollView(for: .bottom) === scroll)
        XCTAssertEqual(node.bounds.height, fixedHeight, accuracy: 0.5, "ownership preserves the fixed frame")
        let port = node.convert(node.bounds, to: route.view)
        let visibleEnd = route.view.bounds.maxY - route.view.safeAreaInsets.bottom
        let overlap = max(0, port.maxY - visibleEnd)
        XCTAssertEqual(scroll.adjustedContentInset.bottom - scroll.contentInset.bottom, overlap, accuracy: 0.5,
                       "only actual overlap is inset, rather than the full native bar height")
        p.navigation.reportCovers()
        p.navigation.prepareRoutes(nav.viewControllers.compactMap { $0 as? RouteController }, in: nav)
        XCTAssertTrue(route.contentScrollView(for: .bottom) === scroll, "cover reports do not oscillate fixed-port ownership")
        XCTAssertEqual(node.bounds.height, fixedHeight, accuracy: 0.5)
    }

    func testShortContentDoesNotGainABlankBottomToolbarRange() throws {
        let session = try fixture("bottom-short-content")
        let (nav, route, node) = try second(session)
        showBottomToolbar(nav, route)
        let scroll = try XCTUnwrap(node.scroll)
        node.content = CGSize(width: node.bounds.width, height: 40)
        node.fitScroll()
        let automaticBottom = max(0, scroll.adjustedContentInset.bottom - scroll.contentInset.bottom)
        XCTAssertGreaterThan(automaticBottom, 0)
        XCTAssertEqual(scroll.contentSize.height, max(40, scroll.bounds.height - automaticBottom), accuracy: 0.5,
                       "the native automatic bottom inset must not create empty scroll content")
    }

    func testPlainShortContentRefitsWhenTheToolbarChangesAndKeepsManualInsetRoom() throws {
        let session = try fixture("bottom-inset-changes")
        let (nav, route, node) = try second(session)
        let scroll = try XCTUnwrap(node.scroll)
        let automaticBottom = { max(0, scroll.adjustedContentInset.bottom - scroll.contentInset.bottom) }
        let tabBarOnly = automaticBottom()
        showBottomToolbar(nav, route)
        session.presenter.onCovers = nil
        session.presenter.onScrolled = nil
        node.content = CGSize(width: node.bounds.width, height: 40)
        node.fitScroll()
        let floor = { max(40, scroll.bounds.height - automaticBottom()) }
        let shown = automaticBottom()
        XCTAssertGreaterThan(shown, 0)
        // UIKit 27 (the iOS 27.2 simulator) lays a tab's toolbar in the tab
        // bar's own room: showing or hiding it changes no inset (83 points
        // either way, 86 and 83 on 26.5), and only the refit is left to check.
        let toolbarRoom = shown > tabBarOnly + 0.5
        let manual = scroll.contentInset.bottom
        let beforeRange = scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height
        scroll.contentInset.bottom += 180 // the same manual inset basis used by the keyboard toolbar
        scroll.layoutIfNeeded()
        spin(0.1)
        XCTAssertEqual(scroll.contentInset.bottom, manual + 180, accuracy: 0.5)
        XCTAssertEqual(scroll.contentSize.height, floor(), accuracy: 0.5)
        XCTAssertEqual(scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height,
                       beforeRange + 180, accuracy: 0.5, "manual keyboard room remains reachable")
        scroll.contentInset.bottom = manual
        nav.setToolbarHidden(true, animated: false)
        nav.view.layoutIfNeeded()
        until("hiding the toolbar refits short content without an authored batch") {
            (!toolbarRoom || automaticBottom() < shown - 0.5) && abs(scroll.contentSize.height - floor()) <= 0.5
        }
        let hidden = automaticBottom()
        nav.setToolbarHidden(false, animated: false)
        nav.view.layoutIfNeeded()
        until("showing the toolbar refits short content without an authored batch") {
            (!toolbarRoom || automaticBottom() > hidden + 0.5) && abs(scroll.contentSize.height - floor()) <= 0.5
        }
        XCTAssertEqual(scroll.contentInset.bottom, manual, accuracy: 0.5, "UIKit refitting preserves authored insets")
    }

    func testARealGroupedListIsTheRoutesOnlyScrollerAndCollapsesItsTitle() throws {
        let (session, nav, route, node) = try nativeList()
        let p = session.presenter
        let collection = try XCTUnwrap((p.groupedLists as? GroupedListHost)?.lists[node.id]?.collection)
        let listID = node.id
        XCTAssertNil(node.scroll)
        XCTAssertEqual(node.subviews.filter { $0 is UIScrollView }.count, 1)
        XCTAssertTrue(node.scrollView === collection)
        XCTAssertTrue(route.contentScrollView(for: .top) === collection)
        XCTAssertTrue(p.navigation.contentScroll(of: route) === collection)
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .always)
        XCTAssertFalse(collection.isHidden)
        XCTAssertGreaterThan(collection.contentSize.height, collection.bounds.height, "the native layout owns the row extent")
        XCTAssertGreaterThan(node.scrollOrigin, 0)
        XCTAssertEqual(collection.contentOffset.y + collection.adjustedContentInset.top, 0, accuracy: 0.5)
        let title: UILabel?
        if #available(iOS 26, *) { title = try largeTitle(in: nav, title: route.navigationItem.title) } else { title = nil }
        let expanded = title.map { visibleTitleHeight($0, in: nav) } ?? nav.navigationBar.bounds.height
        XCTAssertGreaterThan(expanded, 0, "the native large title is visible at CSS 0")
        var scrolled: [UInt32] = []
        p.onScrolled = { id, _, _ in
            if let id { scrolled.append(id) }
        }
        node.pendingScrollTop = 80
        node.applyPendingScroll()
        collection.layoutIfNeeded()
        spin(0.3)
        XCTAssertEqual(collection.contentOffset.y + collection.adjustedContentInset.top, 80, accuracy: 0.5)
        XCTAssertLessThan(title.map { visibleTitleHeight($0, in: nav) } ?? nav.navigationBar.bounds.height, expanded, "the actual content title scrolled under the bar")
        XCTAssertTrue(scrolled.contains(listID), "the collection forwards the logical owner's scroll callback")
        let observation = Agent(session: session).layout([:])
        let measured = (observation["nodes"] as? [[String: Any]])?.first { ($0["id"] as? Int) == Int(listID) }
        XCTAssertEqual(measured?["sy"] as? Double ?? -1, 80, accuracy: 0.5)
    }

    func testACustomLeadingRowKeepsItsNativeTitleAndReaderAfterUnrelatedBatches() throws {
        let (session, nav, route, node) = try nativeList(initialTop: nil, customHero: true)
        let p = session.presenter
        let list = try XCTUnwrap((p.groupedLists as? GroupedListHost)?.lists[node.id])
        let collection = list.collection
        let heroID = node.id + 100
        let heroCell = try XCTUnwrap(list.cell(heroID))
        let heroTop = heroCell.convert(heroCell.bounds, to: window).minY
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
        if #available(iOS 26, *) {
            XCTAssertGreaterThan(visibleTitleHeight(try largeTitle(in: nav, title: route.navigationItem.title), in: nav), 0)
        }
        XCTAssertGreaterThanOrEqual(heroTop, nav.navigationBar.convert(nav.navigationBar.bounds, to: window).maxY, "the custom profile is below the native title")
        p.apply(wireBatch([["op": "props", "id": heroID, "set": ["testId": "updated-native-nav-hero"]]]))
        spin(0.1)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
        XCTAssertEqual(heroCell.convert(heroCell.bounds, to: window).minY, heroTop, accuracy: 0.5)
        if #available(iOS 26, *) {
            XCTAssertGreaterThan(visibleTitleHeight(try largeTitle(in: nav, title: route.navigationItem.title), in: nav), 0)
        }
        p.apply(wireBatch([["op": "props", "id": node.id, "set": ["scrollTop": "140"]]]))
        spin(0.1)
        let row = try XCTUnwrap(p.views[node.id + 10])
        let position = try XCTUnwrap(p.groupedLists?.projectedRect(for: row, in: collection)).minY - collection.contentOffset.y
        p.apply(wireBatch([["op": "props", "id": heroID, "set": ["testId": "updated-again-native-nav-hero"]]]))
        spin(0.1)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 140, accuracy: 0.5)
        XCTAssertEqual(try XCTUnwrap(p.groupedLists?.projectedRect(for: row, in: collection)).minY - collection.contentOffset.y, position, accuracy: 0.5)
    }

    func testMeasuringALeadingCustomRowKeepsItsNativeTitleAtTheStart() throws {
        for provisionalHeight in [0.0, 52.0] {
            let (session, nav, route, node) = try nativeList(initialTop: nil, customHero: true, heroHeight: provisionalHeight)
            let p = session.presenter
            let list = try XCTUnwrap((p.groupedLists as? GroupedListHost)?.lists[node.id])
            let collection = list.collection
            let heroID = node.id + 100
            XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
            p.apply(wireBatch([["op": "frame", "id": heroID, "x": 0.0, "y": 0.0, "w": 370.0, "h": 80.0]]))
            XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5, "measuring the leading row does not scroll away from the start")
            collection.layoutIfNeeded()
            spin(0.2)
            XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
            if #available(iOS 26, *) {
                XCTAssertGreaterThan(visibleTitleHeight(try largeTitle(in: nav, title: route.navigationItem.title), in: nav), 0, "measuring content keeps the native large title visible")
            }
            let heroCell = try XCTUnwrap(list.cell(heroID))
            XCTAssertEqual(heroCell.bounds.height, 80, accuracy: 0.5)
            XCTAssertGreaterThanOrEqual(heroCell.convert(heroCell.bounds, to: window).minY, nav.navigationBar.convert(nav.navigationBar.bounds, to: window).maxY)
            p.apply(wireBatch([["op": "props", "id": heroID, "set": ["testId": "measured-native-nav-hero"]]]))
            spin(0.1)
            XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
        }
    }

    func testAFreshCompiledNativeListStartsWithItsLargeTitleAndKeepsItsStartWhenMeasured() throws {
        ExactGroupedLists.install()
        // Real kernel model, covers and scroll callbacks, with no former
        // navigation controller, plain backend or authored scrollTop.
        let session = try fixture("cold-native-list", planKey: "EXACT_NATIVE_SCROLL_PLAN")
        let p = session.presenter
        let nav = try shownStack(p)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let node = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "cold-native-list" })
        let collection = try XCTUnwrap(node.scrollView)
        let hero = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "cold-hero" })
        XCTAssertNil(node.scroll)
        XCTAssertNil(node.props["scrollTop"])
        XCTAssertTrue(route.contentScrollView(for: .top) === collection)
        func checkStart() throws {
            collection.layoutIfNeeded()
            spin(0.2)
            XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
            if #available(iOS 26, *) {
                XCTAssertGreaterThan(visibleTitleHeight(try largeTitle(in: nav, title: route.navigationItem.title), in: nav), 0)
            }
        }
        try checkStart()
        for (button, height) in [("cold-shrink", 0.0), ("cold-measure", 80.0)] {
            // UIKit's fractional rest can differ from its inset by a few
            // millionths of a point. This is not a reader's chosen anchor.
            collection.contentOffset.y = -collection.adjustedContentInset.top + 0.000001
            p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == button }).id)
            until("the compiled row height changes") { abs(hero.frame.height - height) < 0.5 }
            try checkStart()
        }
        let list = try XCTUnwrap((p.groupedLists as? GroupedListHost)?.lists[node.id])
        let cell = try XCTUnwrap(list.cell(hero.id))
        XCTAssertEqual(cell.bounds.height, 80, accuracy: 0.5)
        XCTAssertGreaterThanOrEqual(cell.convert(cell.bounds, to: window).minY, nav.navigationBar.convert(nav.navigationBar.bounds, to: window).maxY)
        p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "cold-One" }).id)
        try checkStart()
    }

    func testAnIdleFollowEndReaderAtTheStartUsesTheNewNativeTitleInset() throws {
        let (session, _, route, node) = try nativeList(initialTop: 0, followEnd: true)
        let collection = try XCTUnwrap(node.scrollView)
        let header = try XCTUnwrap(route.lifted)
        let heading = try XCTUnwrap(session.presenter.views.values.first { $0.props["accessibilityHeadingLevel"] != nil && $0.isDescendant(of: header) })
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
        collection.contentOffset.y = -collection.adjustedContentInset.top + 0.000001
        session.presenter.apply(wireBatch([["op": "props", "id": heading.id, "set": ["accessibilityHeadingLevel": "2"]]]))
        collection.layoutIfNeeded()
        spin(0.2)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5, "resting at the start does not replay the former title's raw offset")
        XCTAssertEqual(collection.contentOffset.y, -collection.adjustedContentInset.top, accuracy: 0.5)
    }

    func testChangingANativeTitleToInlineKeepsTheVisibleNativeReader() throws {
        let (session, _, route, node) = try nativeList(initialTop: 20)
        let p = session.presenter
        let collection = try XCTUnwrap(node.scrollView)
        let header = try XCTUnwrap(route.lifted)
        let heading = try XCTUnwrap(p.views.values.first { $0.props["accessibilityHeadingLevel"] != nil && $0.isDescendant(of: header) })
        let row = try XCTUnwrap(p.views[node.id + 10])
        let before = try XCTUnwrap(p.groupedLists?.projectedRect(for: row, in: collection)).minY - collection.contentOffset.y
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 20, accuracy: 0.5)
        p.apply(wireBatch([["op": "props", "id": heading.id, "set": ["accessibilityHeadingLevel": "2"]]]))
        spin(0.1)
        XCTAssertTrue(node.scrollView === collection)
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .never)
        let after = try XCTUnwrap(p.groupedLists?.projectedRect(for: row, in: collection)).minY - collection.contentOffset.y
        XCTAssertEqual(after, before, accuracy: 0.5, "changing the title preserves the same native row at the same window position")
        let observation = Agent(session: session).layout([:])
        let measured = (observation["nodes"] as? [[String: Any]])?.first { ($0["id"] as? Int) == Int(node.id) }
        XCTAssertEqual(measured?["sy"] as? Double ?? -1, Double(collection.contentOffset.y + node.scrollTopInset(collection)), accuracy: 0.5, "the new layout basis is reported as the current logical offset")
    }

    func testRevealingAnOffscreenNativeRowFromTheExpandedTitleSurvivesLayout() throws {
        let (session, _, _, node) = try nativeList()
        let p = session.presenter
        let collection = try XCTUnwrap(node.scrollView)
        let targetID = node.id + 30
        p.apply(wireBatch([["op": "props", "id": targetID, "set": ["id": "native-nav-reveal-target"]]]))
        p.scrollElementIntoView(["native-nav-reveal-target", "start", "nearest"])
        collection.layoutIfNeeded()
        spin(0.2)
        let target = try XCTUnwrap(p.views[targetID])
        let rect = try XCTUnwrap(p.groupedLists?.projectedRect(for: target, in: collection))
        XCTAssertEqual(rect.minY - collection.contentOffset.y - collection.adjustedContentInset.top, 0, accuracy: 0.5, "the requested native row remains aligned after the bar collapses")
        let list = try XCTUnwrap((p.groupedLists as? GroupedListHost)?.lists[node.id])
        XCTAssertNotNil(list.cell(targetID), "the row is actually realized in the visible native collection")
    }

    func testANativePaddedListsDefaultOffsetIsZeroWithoutAnAuthoredScrollTop() throws {
        let (_, _, route, node) = try nativeList(initialTop: nil, paddingTop: 24)
        let collection = try XCTUnwrap(node.scrollView)
        XCTAssertTrue(route.contentScrollView(for: .top) === collection)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 0, accuracy: 0.5)
    }

    func testALaterAuthoredWriteWinsBeforeTheNativeListsFirstSettledInset() throws {
        let (session, _, route, node) = try nativeList(laterAuthoredTop: 140)
        let collection = try XCTUnwrap(node.scrollView)
        XCTAssertTrue(route.contentScrollView(for: .top) === collection)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 140, accuracy: 0.5, "the later write wins over the backend's initial zero position")
        session.presenter.navigation.reportCovers()
        spin(0.1)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), 140, accuracy: 0.5, "settled inset reports do not replay the original position")
    }

    private func checkPaddedRelease(initialTop: Double) throws {
        let (session, _, route, node) = try nativeList(initialTop: initialTop, paddingTop: 24)
        let collection = try XCTUnwrap(node.scrollView)
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), initialTop, accuracy: 0.5)
        session.presenter.apply(wireBatch([["op": "props", "id": route.node.id, "clear": ["navigationScroll"]]]))
        spin(0.3)
        XCTAssertNil(route.collapseScroll)
        XCTAssertNil(route.collapseScrollView)
        XCTAssertTrue(node.scrollView === collection, "releasing the bar does not replace the native backend")
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .never)
        XCTAssertEqual(collection.contentInset.top, 24, accuracy: 0.5, "native content padding remains owned by the list")
        XCTAssertEqual(collection.contentOffset.y + node.scrollTopInset(collection), initialTop, accuracy: 0.5, "releasing navigation ownership preserves CSS offset including native padding")
    }

    func testReleasingANativePaddedScrollerPreservesZeroOffset() throws {
        try checkPaddedRelease(initialTop: 0)
    }

    func testReleasingANativePaddedScrollerPreservesNonzeroOffset() throws {
        try checkPaddedRelease(initialTop: 140)
    }

    func testANativeListsAuthoredOffsetSurvivesBackendChanges() throws {
        let (session, nav, route, node) = try nativeList(initialTop: 140)
        let p = session.presenter
        weak var originalCollection = node.scrollView
        func css() -> CGFloat { let scroll = node.scrollView!; return scroll.contentOffset.y + scroll.adjustedContentInset.top }
        XCTAssertEqual(css(), 140, accuracy: 0.5, "a nonzero authored offset is not changed to zero during inset adoption")
        if #available(iOS 26, *) {
            let title = try largeTitle(in: nav, title: route.navigationItem.title)
            XCTAssertEqual(visibleTitleHeight(title, in: nav), 0, accuracy: 0.5, "the title is already above the bar at that position")
        }
        p.apply(wireBatch([["op": "props", "id": node.id, "clear": ["listStyle"]],
                          ["op": "content", "id": node.id, "w": 402.0, "h": 2400.0]]))
        spin(0.3)
        let plain = try XCTUnwrap(node.scroll)
        XCTAssertTrue(route.contentScrollView(for: .top) === plain)
        XCTAssertEqual(css(), 140, accuracy: 0.5, "the replacement plain backend preserves the reader's CSS position")
        p.apply(wireBatch([["op": "props", "id": node.id, "set": ["listStyle": "inset-grouped"]]]))
        spin(0.3)
        let replacement = try XCTUnwrap((p.groupedLists as? GroupedListHost)?.lists[node.id]?.collection)
        XCTAssertTrue(route.contentScrollView(for: .top) === replacement)
        XCTAssertFalse(replacement === originalCollection)
        XCTAssertNil(node.scroll)
        XCTAssertEqual(css(), 140, accuracy: 0.5, "the replacement native backend preserves the reader's CSS position")
        p.navigation.reportCovers()
        spin(0.1)
        XCTAssertEqual(css(), 140, accuracy: 0.5, "later cover reports do not repeat the initial handoff")
    }

    // MARK: A large title's lifecycle over one physical scroller (LLP 1084 §6.5)
    // The compiled fixture's later tabs, its list arriving while hidden, the
    // node put between a header and its list, a replaced scroller and pops.

    private func lifecycle() throws -> Presenter {
        ExactGroupedLists.install()
        return try fixture("title-lifecycle", planKey: "EXACT_NATIVE_SCROLL_PLAN").presenter
    }

    private func node(_ p: Presenter, _ testId: String) -> NodeView? {
        p.views.values.first { $0.props["testId"] == testId }
    }

    private func press(_ p: Presenter, _ testId: String) throws {
        p.press(try XCTUnwrap(node(p, testId), testId).id)
    }

    /// The stack shown: the primary one, or the selected tab's.
    private func shownStack(_ p: Presenter) throws -> UINavigationController {
        if let nav = p.navigation.primaryNavigation { return nav }
        return try XCTUnwrap(p.navigation.tabController?.selectedViewController as? UINavigationController)
    }

    private func select(_ p: Presenter, tab index: Int) throws -> (UINavigationController, RouteController) {
        let tabs = try XCTUnwrap(p.navigation.tabController)
        _ = tabs.delegate?.tabBarController?(tabs, shouldSelect: tabs.viewControllers![index])
        until("tab \(index) is selected") { tabs.selectedIndex == index }
        tabs.view.setNeedsLayout()
        tabs.view.layoutIfNeeded()
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        until("the tab's route is in the window") { route.view.window === self.window }
        spin(0.3)
        return (nav, route)
    }

    /// The most of the route's title any label shows: the large title, in
    /// its scroller or the bar, or the inline one.
    private func shownTitle(_ nav: UINavigationController, _ route: RouteController) -> CGFloat {
        var most: CGFloat = 0
        func walk(_ view: UIView) {
            if let label = view as? UILabel, label.text == route.navigationItem.title { most = max(most, visibleTitleHeight(label, in: nav)) }
            for child in view.subviews { walk(child) }
        }
        walk(nav.view)
        return most
    }

    private func largeTitleShows(_ nav: UINavigationController, _ route: RouteController, _ what: String) throws {
        guard #available(iOS 26, *) else { return }
        XCTAssertGreaterThan(visibleTitleHeight(try largeTitle(in: nav, title: route.navigationItem.title), in: nav), 0, what)
    }

    private func fromStart(_ node: NodeView, _ scroll: UIScrollView) -> CGFloat { scroll.contentOffset.y + node.scrollTopInset(scroll) }

    /// Followed by the bar, its scroll view at its start and its large title shown.
    private func followed(_ testId: String, by route: RouteController, in nav: UINavigationController, _ p: Presenter) throws {
        until("the route's bar follows \(testId)") { self.node(p, testId)?.scrollView.map { route.contentScrollView(for: .top) === $0 } ?? false }
        spin(0.3)
        let n = try XCTUnwrap(node(p, testId)), scroll = try XCTUnwrap(n.scrollView)
        XCTAssertTrue(route.collapseScroll === n)
        XCTAssertEqual(fromStart(n, scroll), 0, accuracy: 0.5, "\(testId) rests at its start")
        try largeTitleShows(nav, route, "the large title shows over \(testId) at its start")
    }

    func testALaterTabRestsWithItsLargeTitleOverItsListsStart() throws {
        let p = try lifecycle()
        let (nav, route) = try select(p, tab: 1)
        try followed("later-list", by: route, in: nav, p)
    }

    func testAListThatArrivesWhileItsTabIsHiddenIsFollowedFromItsStart() throws {
        let p = try lifecycle()
        try press(p, "control-arrive")
        until("the hidden tab's list is built") { self.node(p, "arriving-list")?.scrollView != nil }
        spin(0.3)
        let (nav, route) = try select(p, tab: 2)
        try followed("arriving-list", by: route, in: nav, p)
    }

    func testANodePutBetweenTheHeaderAndTheListAndTakenOutKeepsTheTitle() throws {
        let p = try lifecycle()
        let (nav, route) = try select(p, tab: 1)
        try followed("later-list", by: route, in: nav, p)
        try press(p, "control-between")
        until("the bar stops following the list") { route.collapseScroll == nil && route.collapseScrollView == nil }
        spin(0.3)
        // UIKit's bar over no scroller shows its large title, still.
        try largeTitleShows(nav, route, "the title shows over a list the bar no longer follows")
        try press(p, "control-between")
        try followed("later-list", by: route, in: nav, p)
    }

    func testReplacingTheScrollerUnderTheTitleFollowsTheNewOneFromItsStart() throws {
        let p = try lifecycle()
        let (nav, route) = try select(p, tab: 1)
        try followed("later-list", by: route, in: nav, p)
        weak var collection = node(p, "later-list")?.scrollView
        try press(p, "control-plain")
        try followed("later-plain", by: route, in: nav, p)
        XCTAssertNil(node(p, "later-list"), "the list is gone, not hidden under the scroll")
        try press(p, "control-plain")
        try followed("later-list", by: route, in: nav, p)
        XCTAssertFalse(node(p, "later-list")?.scrollView === collection, "a new list's collection view")
    }

    /// Push the fixture's pushed route over the cold root, then pop it as
    /// UIKit's back button does, or as the app does when `byApp`.
    private func pushAndPop(_ p: Presenter, _ nav: UINavigationController, _ root: RouteController, byApp: Bool = false) throws {
        try press(p, "control-open")
        until("the pushed route is shown") { nav.topViewController !== root && nav.transitionCoordinator == nil }
        spin(0.3)
        let pushed = try XCTUnwrap(nav.topViewController as? RouteController)
        XCTAssertGreaterThan(shownTitle(nav, pushed), 0, "the pushed route shows a title")
        if byApp {
            // UIKit's own pop moves, as a user sees it.
            UIView.setAnimationsEnabled(true)
            defer { UIView.setAnimationsEnabled(false) }
            try press(p, "pushed-back")
            until("the pop starts") { nav.topViewController === root && nav.transitionCoordinator != nil }
            until("the pop ends") { nav.transitionCoordinator == nil }
        } else {
            nav.popViewController(animated: true)
        }
        until("the root is shown again") { nav.topViewController === root && nav.transitionCoordinator == nil }
        spin(0.3)
        XCTAssertEqual(nav.viewControllers, [root], "one route left, the root")
    }

    /// What shows of a leaving route's large title in its scroller: UIKit
    /// moves it out as a copy of its title view above the scroller's content
    /// (the label itself goes clear), the copies a reader sees.
    private func leavingTitleCopies(in scroll: UIScrollView) -> Int {
        func opacity(_ view: UIView) -> Float {
            var shown: Float = 1, current: UIView? = view
            while let v = current { shown *= v.isHidden ? 0 : (v.layer.presentation()?.opacity ?? v.layer.opacity); current = v.superview }
            return shown
        }
        return scroll.subviews.filter {
            $0.frame.maxY <= 0.5 && $0.frame.height > 30 && abs($0.frame.width - scroll.bounds.width) < 0.5 && opacity($0) > 0.01
        }.count
    }

    /// The root's large title leaves with the root as a push starts. On iOS
    /// 27.2 a push made by `setViewControllers` takes it off at once: no copy
    /// of it moves out with the root (UIKit's own push moves one).
    func testARootsLargeTitleSlidesOutWithItAsAPushStarts() throws {
        guard #available(iOS 26, *) else { return }
        let p = try lifecycle()
        let nav = try shownStack(p)
        let root = try XCTUnwrap(nav.topViewController as? RouteController)
        try followed("cold-native-list", by: root, in: nav, p)
        // UIKit's own push moves, as a user sees it.
        UIView.setAnimationsEnabled(true)
        try press(p, "control-open")
        until("the push starts") { nav.topViewController !== root && nav.transitionCoordinator != nil }
        spin(0.1)
        XCTAssertNotNil(nav.transitionCoordinator, "the push still moves")
        let collection = try XCTUnwrap(node(p, "cold-native-list")?.scrollView)
        XCTAssertGreaterThan(leavingTitleCopies(in: collection), 0, "the root's large title moves out with the root")
        until("the push ends") { nav.transitionCoordinator == nil }
    }

    func testAnAppsOwnPopBackToARootAtRestShowsItsLargeTitleAtItsStart() throws {
        let p = try lifecycle()
        let nav = try shownStack(p)
        let root = try XCTUnwrap(nav.topViewController as? RouteController)
        try followed("cold-native-list", by: root, in: nav, p)
        try pushAndPop(p, nav, root, byApp: true)
        try followed("cold-native-list", by: root, in: nav, p)
    }

    func testAPopBackToARootAtRestShowsItsLargeTitleAtItsStart() throws {
        let p = try lifecycle()
        let nav = try shownStack(p)
        let root = try XCTUnwrap(nav.topViewController as? RouteController)
        try followed("cold-native-list", by: root, in: nav, p)
        try pushAndPop(p, nav, root)
        try followed("cold-native-list", by: root, in: nav, p)
    }

    func testAPopBackToAScrolledRootKeepsItsReaderAndShowsATitle() throws {
        let p = try lifecycle()
        let nav = try shownStack(p)
        let root = try XCTUnwrap(nav.topViewController as? RouteController)
        try followed("cold-native-list", by: root, in: nav, p)
        let list = try XCTUnwrap(node(p, "cold-native-list")), collection = try XCTUnwrap(list.scrollView)
        list.pendingScrollTop = 400
        list.applyPendingScroll()
        nav.navigationBar.setNeedsLayout()
        nav.navigationBar.layoutIfNeeded()
        spin(0.3)
        let before = fromStart(list, collection)
        XCTAssertGreaterThan(before, 100, "the root is scrolled past its large title")
        try pushAndPop(p, nav, root)
        XCTAssertTrue(root.contentScrollView(for: .top) === collection)
        XCTAssertEqual(fromStart(list, collection), before, accuracy: 0.5, "the reader's place survives the push and the pop")
        XCTAssertGreaterThan(shownTitle(nav, root), 0, "a title shows over the scrolled root")
    }

    /// A tab selected again over its scrolled list: on the 26.5 and 27.2
    /// simulators a UIKit app's bar over a plain scroll view comes back as
    /// tall as a large title, with no title in it and the reader 52 points
    /// lower; over a collection view, inline with its title (LLP 1084 §6.5).
    /// Exact keeps the reader's place and shows a title over both.
    func testATabSelectedAgainOverItsScrolledListKeepsItsReaderAndShowsATitle() throws {
        let p = try lifecycle()
        let (nav, route) = try select(p, tab: 1)
        try followed("later-list", by: route, in: nav, p)
        // A grouped list, then the plain `scroll` the control swaps in for it.
        for testId in ["later-list", "later-plain"] {
            if testId == "later-plain" {
                try press(p, "control-plain")
                try followed(testId, by: route, in: nav, p)
            }
            let list = try XCTUnwrap(node(p, testId)), scroll = try XCTUnwrap(list.scrollView)
            list.pendingScrollTop = 400
            list.applyPendingScroll()
            nav.navigationBar.setNeedsLayout()
            nav.navigationBar.layoutIfNeeded()
            spin(0.3)
            let before = fromStart(list, scroll), bar = nav.navigationBar.frame
            XCTAssertGreaterThan(before, 100, "\(testId) is scrolled past its large title")
            _ = try select(p, tab: 0)
            _ = try select(p, tab: 1)
            XCTAssertTrue(route.contentScrollView(for: .top) === scroll)
            XCTAssertEqual(fromStart(list, scroll), before, accuracy: 0.5, "the reader's place over \(testId) survives")
            XCTAssertEqual(nav.navigationBar.frame.maxY, bar.maxY, accuracy: 0.5, "the bar over \(testId) keeps its collapsed height")
            XCTAssertGreaterThan(shownTitle(nav, route), 0, "a title shows over \(testId)")
        }
    }

    /// UIKit infers a view's content margins, which an inset-grouped list's
    /// side insets come from, from its superview when the view's own
    /// geometry changes. A route's node sized for an iPad card while its
    /// controller's view was still 820 points wide took a trailing margin of
    /// 0, and kept it when that view shrank to the card's 580: the list's
    /// trailing inset was 8 points where UIKit's is 16 (iOS 27.2, LLP 1084
    /// §6.5). The controller has the node's margins inferred again as its
    /// view's size changes. The margins are UIKit's own (`_contentMargins`,
    /// read by key): here the window's, 20 points a side.
    func testARouteNodeTakesItsControllersContentMarginsAgainAsItsViewResizes() throws {
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        let route = RouteController(node), container = UIViewController()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 820, height: 660))
        window.rootViewController = container
        window.isHidden = false
        defer { window.isHidden = true }
        container.addChild(route)
        route.view.frame = container.view.bounds
        route.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        container.view.addSubview(route.view)
        route.didMove(toParent: container)
        guard route.view.responds(to: Selector(("_contentMargins"))) else { throw XCTSkip("no content margins in this UIKit (26.5)") }
        func margins(_ object: NSObject) -> UIEdgeInsets { (object.value(forKey: "contentMargins") as? NSValue)?.uiEdgeInsetsValue ?? .zero }
        window.layoutIfNeeded()
        // The node laid out for the card, its controller's view not yet.
        node.frame = CGRect(x: 0, y: 0, width: 580, height: 660)
        window.layoutIfNeeded()
        let side = margins(route.view).right
        XCTAssertGreaterThan(side, 0, "the controller's view has UIKit's margins")
        XCTAssertEqual(margins(node).right, 0, accuracy: 0.5, "a node narrower than its superview takes none of its trailing margin")
        // The view takes the card's size; the node's frame does not change.
        window.frame = CGRect(x: 0, y: 0, width: 580, height: 660)
        window.layoutIfNeeded()
        XCTAssertEqual(route.view.bounds.width, 580)
        XCTAssertEqual(margins(route.view).right, side)
        XCTAssertEqual(margins(node).left, side, accuracy: 0.5)
        XCTAssertEqual(margins(node).right, side, accuracy: 0.5, "the node's margins are inferred again")
        XCTAssertTrue(node.insetsLayoutMarginsFromSafeArea, "and the setter used for it is left as it was")
    }

}
#endif
