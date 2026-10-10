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
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
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
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
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

}
#endif
