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
final class NativeListNavigationIOSTests: XCTestCase {
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

    private func settle(_ p: Presenter) {
        window?.layoutIfNeeded()
        let turned = expectation(description: "queued title and layout work ran")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 5)
        guard let session = p.session else { return }
        let agent = Agent(session: session)
        until("native work settled") { !agent.nativeInFlight() }
        window?.layoutIfNeeded()
    }

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
        settle(session.presenter)
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
        XCTAssertTrue(navigation.shownScroll(navigation.contentScroller(of: route)) === collection, "route hatches receive the actual collection")
        XCTAssertTrue(ElementHatches.platform(of: node, session.presenter) === collection, "element hatches use that backend too")
        XCTAssertTrue(route.contentScrollView(for: .top) === collection, "UIKit follows the actual collection")
        XCTAssertTrue(route.collapseScroll === node)
        XCTAssertTrue(route.collapseScrollView === collection)
        XCTAssertEqual(plain.contentInsetAdjustmentBehavior, .never)
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .always)
        collection.contentInset.bottom = 40
        XCTAssertTrue(NavigationHost.ownedChanges(collection, of: node, insetByUIKit: true).isEmpty, "the collection retains its Exact-owned delegate and native footer insets")
        let changedDelegate = NativeDelegate()
        collection.delegate = changedDelegate
        XCTAssertTrue(NavigationHost.ownedChanges(collection, of: node, insetByUIKit: true).contains("delegate"), "a hatch replacing that delegate is detected")
        collection.delegate = delegate
        node.scrollOrigin = 123
        node.scrollCollapsed = 45
        collection.removeFromSuperview()
        let replacement = mount()
        XCTAssertTrue(route.collapseScroll === node, "the logical owner did not change")
        XCTAssertTrue(route.contentScrollView(for: .top) === replacement, "a new backend on the same node is rebound")
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .never)
        XCTAssertTrue(route.contentScrollView(for: .bottom) === replacement, "the bottom edge follows the replacement too")
        XCTAssertEqual(replacement.contentInsetAdjustmentBehavior, .always)
        XCTAssertEqual(node.scrollOrigin, replacement.adjustedContentInset.top, "the replacement uses its own title inset")
        XCTAssertEqual(node.scrollCollapsed, replacement.adjustedContentInset.top)
        XCTAssertEqual(replacement.contentOffset.y + node.scrollTopInset(replacement), 0, accuracy: 0.5, "the replacement keeps the logical start")
        XCTAssertTrue(navigation.shownScroll(navigation.contentScroller(of: route)) === replacement)
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
        XCTAssertTrue(p.navigation.shownScroll(p.navigation.contentScroller(of: route)) === collection)
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

    func testTheFirstNativeRouteBindsBothScrollEdgesDuringBoot() throws {
        ExactGroupedLists.install()
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_NATIVE_SCROLL_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "initial-native-scroll-edges")
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
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        // No deferred turn or appearance callback may be needed to attach
        // either edge: UIKit receives this route during the boot transaction.
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "cold-native-list" })
        let native = try XCTUnwrap(node.scrollView as? UICollectionView)
        XCTAssertTrue(route.collapseScroll === node)
        XCTAssertTrue(route.collapseScrollView === native)
        XCTAssertTrue(route.contentScrollView(for: .top) === native)
        XCTAssertTrue(route.edgeScroll === node)
        XCTAssertTrue(route.contentScrollView(for: .bottom) === native)
        XCTAssertNil(node.scroll, "the visible collection is the only physical backend")
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
        session.presenter.apply(wireBatch([["op": "props", "id": route.node.id, "set": ["navigationScroll": "missing"]]]))
        spin(0.3)
        XCTAssertNil(route.collapseScroll)
        XCTAssertNil(route.collapseScrollView)
        XCTAssertTrue(node.scrollView === collection, "releasing the bar does not replace the native backend")
        XCTAssertEqual(collection.contentInsetAdjustmentBehavior, .always)
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
        settle(p)
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
        settle(p)
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
        settle(p)
        let (nav, route) = try select(p, tab: 2)
        try followed("arriving-list", by: route, in: nav, p)
    }

    func testANodePutBetweenTheHeaderAndTheListAndTakenOutKeepsTheTitle() throws {
        let p = try lifecycle()
        let (nav, route) = try select(p, tab: 1)
        try followed("later-list", by: route, in: nav, p)
        try press(p, "control-between")
        until("the bar stops following the list") { route.collapseScroll == nil && route.collapseScrollView == nil }
        settle(p)
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
        settle(p)
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
        settle(p)
        XCTAssertEqual(nav.viewControllers, [root], "one route left, the root")
    }

    private final class RecordingNavigationController: UINavigationController {
        enum Change: Equatable {
            case push(UIViewController, Bool)
            case pop(Bool)
            case replace([UIViewController], Bool)
        }
        var changes: [Change] = []

        func start(_ controllers: [UIViewController]) {
            super.setViewControllers(controllers, animated: false)
            changes = []
        }
        override func pushViewController(_ controller: UIViewController, animated: Bool) { changes.append(.push(controller, animated)) }
        override func popViewController(animated: Bool) -> UIViewController? { changes.append(.pop(animated)); return viewControllers.last }
        override func setViewControllers(_ controllers: [UIViewController], animated: Bool) { changes.append(.replace(controllers, animated)) }
    }

    /// Record the public UIKit operation, not the private snapshot views
    /// UIKit uses to animate the leaving title. Real-touch comparisons cover
    /// the animation's pixels (LLP 1084 §6.5).
    func testSingleRouteChangesUseUIKitsPushAndPop() {
        let host = Presenter().navigation
        let nav = RecordingNavigationController()
        let root = UIViewController(), pushed = UIViewController(), next = UIViewController()
        let cases: [([UIViewController], [UIViewController], Bool, RecordingNavigationController.Change)] = [
            ([root], [root, pushed], true, .push(pushed, true)),
            ([root, pushed], [root], true, .pop(true)),
            ([root], [root, pushed], false, .replace([root, pushed], false)),
            ([root, pushed], [root], false, .replace([root], false)),
            ([root], [root, pushed, next], true, .replace([root, pushed, next], true)),
            ([root, pushed], [root, next], true, .replace([root, next], true)),
            ([root], [pushed, next], true, .replace([pushed, next], true)),
            ([root, pushed], [next], true, .replace([next], true)),
        ]
        for (shown, wanted, animated, change) in cases {
            nav.start(shown)
            host.setStack(wanted, in: nav, animated: animated)
            XCTAssertEqual(nav.changes, [change])
        }
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
        settle(p)
        let before = fromStart(list, collection)
        XCTAssertGreaterThan(before, 100, "the root is scrolled past its large title")
        try pushAndPop(p, nav, root)
        XCTAssertTrue(root.contentScrollView(for: .top) === collection)
        XCTAssertEqual(fromStart(list, collection), before, accuracy: 0.5, "the reader's place survives the push and the pop")
        XCTAssertGreaterThan(shownTitle(nav, root), 0, "a title shows over the scrolled root")
    }

    /// A tab selected again over its scrolled list: on the 26.5 and 27.2
    /// simulators a UIKit app's bar comes back inline with its title and the
    /// reader where it was, over a plain scroll view as over a collection
    /// view (LLP 1084 §6.5). Exact does the same over both.
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
            settle(p)
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

    /// UIKit sizes a scroller's scroll bar from insets it caches when the
    /// scroller's safe area changes, the large title it holds left out. A
    /// list given its bar after its last change kept a bar that counted the
    /// title: listed to VoiceOver 52 points below UIKit's at rest (iOS 26.5).
    /// The cached insets (`_effectiveVerticalScrollIndicatorInsets`, read by
    /// key) are the ones UIKit computes again when an indicator inset changes.
    func testAListAtRestUnderItsBarHasItsScrollBarMeasuredForItsTitle() throws {
        let p = try lifecycle()
        let nav = try shownStack(p)
        let root = try XCTUnwrap(nav.topViewController as? RouteController)
        try followed("cold-native-list", by: root, in: nav, p)
        let scroll = try XCTUnwrap(node(p, "cold-native-list")?.scrollView)
        guard scroll.responds(to: Selector(("_effectiveVerticalScrollIndicatorInsets"))) else { throw XCTSkip("no cached indicator insets in this UIKit") }
        func effective() -> UIEdgeInsets { (scroll.value(forKey: "effectiveVerticalScrollIndicatorInsets") as? NSValue)?.uiEdgeInsetsValue ?? .zero }
        func measureAgain() {
            let insets = scroll.verticalScrollIndicatorInsets
            var changed = insets
            changed.top += 1
            scroll.verticalScrollIndicatorInsets = changed
            scroll.verticalScrollIndicatorInsets = insets
        }
        let followedTop = effective().top
        // The bar stops following the list and the bar is measured then, as
        // it is before a route first gives its list (the title not in it).
        root.track(nil)
        nav.navigationBar.layoutIfNeeded()
        settle(p)
        measureAgain()
        // The list given back: UIKit moves the title into it, its safe area unchanged.
        root.track(scroll)
        until("the bar follows the list again") { root.contentScrollView(for: .top) === scroll }
        settle(p)
        let cached = effective().top
        measureAgain()
        XCTAssertEqual(effective().top, followedTop, accuracy: 0.5)
        XCTAssertEqual(cached, effective().top, accuracy: 0.5, "the scroll bar at rest is measured for the title the list holds")
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
