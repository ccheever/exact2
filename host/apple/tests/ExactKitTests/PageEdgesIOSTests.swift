#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
import ExactGroupedLists

/// LLP 1130: a page with no native container keeps the area it is laid out
/// in, the view less its safe area, and is drawn in a viewport that takes
/// the whole view, so it scrolls under the status bar and the home
/// indicator; a scroller at the area's top and foot reaches into those bands
/// with its content where it was and its `scrollTop` unchanged.
final class PageEdgesIOSTests: XCTestCase {
    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// `scripts/fixtures/page-edges.contract` in a window of the test host's
    /// scene, the screen's size, so its safe area is the device's.
    private func fixture(_ key: String = "PAGE_EDGES") throws -> (ExactSession, ExactView) {
        let plan = try fixturePlan(key)
        let session = ExactApp.shared.makeSession(label: "page-edges")
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the page is laid out") { (try? node(session, "list"))?.scrollView != nil }
        view.fit()
        return (session, view)
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    func testThePageKeepsItsAreaAndIsDrawnPastIt() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        let p = session.presenter, safe = view.safeAreaInsets
        XCTAssertGreaterThan(safe.top + safe.bottom, 0, "the test host's window has a status bar or a home indicator")
        XCTAssertEqual(p.viewport.frame, view.bounds, "the viewport takes the whole view")
        XCTAssertEqual(p.viewportSize, view.bounds.inset(by: safe).size, "the kernel lays the page out in its area, as before")
        XCTAssertEqual(p.pageMargins.top, safe.top)
        XCTAssertEqual(p.pageMargins.bottom, safe.bottom)
        XCTAssertEqual(p.root.frame.minY, safe.top, "the document starts at the area's top")
        XCTAssertEqual(p.viewport.contentSize.height, view.bounds.height, accuracy: 0.5, "a page that fits its area does not scroll")
        XCTAssertEqual(p.viewport.contentOffset, .zero, "and rests at offset zero, its scrollTop")
        let top = try node(session, "list-top")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, safe.top, accuracy: 0.5, "the content shows where it did")
    }

    func testTheScrollerAtTheEdgesReachesIntoTheBands() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        let safe = view.safeAreaInsets, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        XCTAssertEqual(list.pageBand, UIEdgeInsets(top: safe.top, left: 0, bottom: safe.bottom, right: 0))
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).minY, 0, accuracy: 0.5, "its scroll view starts at the screen's top")
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).maxY, view.bounds.maxY, accuracy: 0.5, "and ends at its foot")
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 0, accuracy: 0.5, "scrollTop 0 is the rest under the band")
        let top = try node(session, "list-top")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, safe.top, accuracy: 0.5, "its content shows where it did")
        // The CSS port is the box: the band is neither port nor content.
        XCTAssertEqual(sv.bounds.height - list.scrollPortInsets(sv).top - list.scrollPortInsets(sv).bottom, list.bounds.height, accuracy: 0.5)
    }

    func testATouchInABandReachesTheScrollerThere() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        let list = try node(session, "list"), sv = try XCTUnwrap(list.scroll), window = try XCTUnwrap(view.window)
        // A finger under the status bar or on the home indicator's band, as on a hand-built table view.
        for y in [view.safeAreaInsets.top / 2, view.bounds.maxY - view.safeAreaInsets.bottom / 2] {
            let hit = try XCTUnwrap(window.hitTest(CGPoint(x: view.bounds.midX + 60, y: y), with: nil), "nothing at y \(y)")
            XCTAssertTrue(hit.isDescendant(of: sv), "y \(y) reaches the scroller, not \(type(of: hit))")
        }
    }

    func testARouteWithNoBarRunsItsScrollerUnderTheStatusBar() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture("BARLESS_ROUTE")
        guard view.safeAreaInsets.top > 0 else {
            throw XCTSkip("no status bar above the route for its scroller to run under: the iPhone Duo's stands in a side column")
        }
        let list = try node(session, "list"), route = try node(session, "route"), top = try node(session, "list-top")
        until("the route's scroller is UIKit's to inset") { list.scrollView?.adjustedContentInset.top ?? 0 > 0 }
        let sv = try XCTUnwrap(list.scrollView), safe = view.safeAreaInsets
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).minY, 0, accuracy: 0.5, "its scroll view starts at the screen's top")
        XCTAssertEqual(route.convert(route.bounds, to: nil).minY, 0, accuracy: 0.5)
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, safe.top, accuracy: 0.5, "its content starts below the status bar")
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 0, accuracy: 0.5, "at scrollTop 0")
    }

    func testAGroupedListAtTheEdgesReachesIntoTheBands() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        ExactGroupedLists.install()
        let (session, view) = try fixture("PAGE_EDGES_GROUPED")
        let list = try node(session, "list"), safe = view.safeAreaInsets
        // The bands at the top and the foot, or only the foot's on the iPhone Duo, whose status bar stands in a side column
        until("UIKit draws the list") { list.scrollView is UICollectionView && list.pageBand.top + list.pageBand.bottom > 0 }
        let cv = try XCTUnwrap(list.scrollView as? UICollectionView)
        XCTAssertEqual(list.pageBand, UIEdgeInsets(top: safe.top, left: 0, bottom: safe.bottom, right: 0))
        XCTAssertEqual(cv.convert(cv.bounds, to: nil).minY, 0, accuracy: 0.5, "its collection view starts at the screen's top")
        XCTAssertEqual(cv.convert(cv.bounds, to: nil).maxY, view.bounds.maxY, accuracy: 0.5, "and ends at its foot")
        XCTAssertEqual(cv.contentOffset.y + list.scrollTopInset(cv), 0, accuracy: 0.5, "scrollTop 0 is the rest under the band")
        cv.layoutIfNeeded()
        let first = try XCTUnwrap(cv.visibleCells.min { $0.frame.minY < $1.frame.minY })
        XCTAssertGreaterThanOrEqual(first.convert(first.bounds, to: nil).minY, safe.top - 0.5, "its first card starts below the status bar")
        if safe.top > 0 {
            let hit = try XCTUnwrap(view.window?.hitTest(CGPoint(x: view.bounds.midX, y: safe.top / 2), with: nil))
            XCTAssertTrue(hit.isDescendant(of: cv), "a finger under the status bar reaches the list, not \(type(of: hit))")
        }
    }

    /// A scroller at the edges shown again inside an animation UIKit has
    /// open (`display: none` lifted): it takes its band, its insets and its
    /// offset together and at once, so its content does not slide
    /// (`NodeView.setPageBand` through `Presenter.moveOrPlace`).
    func testAScrollerShownAgainInsideAnAnimationTakesItsBandAtOnce() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        let p = session.presenter, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        let band = list.pageBand
        XCTAssertGreaterThan(band.top + band.bottom, 0, "a band to take")
        list.isHidden = true
        p.reachPageEdges()
        XCTAssertEqual(list.pageBand, .zero, "a hidden scroller has no band")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05)) // a frame shows it hidden
        var open = false
        UIView.animate(withDuration: 0.5) {
            open = UIView.inheritedAnimationDuration > 0
            list.isHidden = false
            p.reachPageEdges()
        }
        XCTAssertTrue(open, "UIKit had an animation open")
        XCTAssertEqual(list.pageBand, band)
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 0, accuracy: 0.5, "at its rest under the band")
        XCTAssertEqual(sv.layer.animationKeys() ?? [], [], "its frame and its offset at once, together")
        _ = view
    }

    /// The offset of a grouped list's collection view, which UIKit sets as
    /// it insets it, follows the same rule as a `ScrollView`'s: set inside an
    /// animation on a list no frame has shown, it is there at once.
    func testAGroupedListsOffsetInsideAnAnimationMovesOnlyIfItShowed() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        ExactGroupedLists.install()
        let (session, _) = try fixture("PAGE_EDGES_GROUPED")
        let list = try node(session, "list")
        until("UIKit draws the list") { list.scrollView is UICollectionView && list.pageBand.top + list.pageBand.bottom > 0 }
        let cv = try XCTUnwrap(list.scrollView as? UICollectionView)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05)) // a frame shows the list
        let rest = cv.contentOffset
        UIView.animate(withDuration: 0.5) { cv.contentOffset.y = rest.y + 20 }
        XCTAssertNotNil(cv.layer.animation(forKey: "bounds.origin") ?? cv.layer.animation(forKey: "bounds"), "a list that showed moves with the animation")
        cv.layer.removeAllAnimations()
        list.isHidden = true
        RunLoop.main.run(until: Date().addingTimeInterval(0.05)) // a frame shows it hidden
        list.isHidden = false
        UIView.animate(withDuration: 0.5) { cv.contentOffset.y = rest.y }
        XCTAssertEqual(cv.layer.animationKeys() ?? [], [], "one shown again since the last frame is at its offset at once")
    }

    /// LLP 1131: a route's list under its control bars, drawn through them and the route's cover.
    func testAListRunsUnderItsControlBarsAndKeepsItsPlace() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the bars and the safe area are a scene window's") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("control bars are iOS 26's, where UIKit draws the edge effect over them") }
        let (session, _) = try fixture("CONTROL_BARS")
        let p = session.presenter, list = try node(session, "list"), filter = try node(session, "filter"), top = try node(session, "list-top")
        until("the switcher is a bar over the list") { list.pageBand.top > 0 }
        let sv = try XCTUnwrap(list.scrollView), box = list.convert(list.bounds, to: nil)
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).minY, 0, accuracy: 0.5, "its scroll view runs up under the switcher and the bar")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, box.minY, accuracy: 0.5, "its content rests where it was laid out")
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 0, accuracy: 0.5, "at scrollTop 0")
        XCTAssertGreaterThan(filter.layer.zPosition, list.layer.zPosition, "the switcher draws over the list")
        let hit = try XCTUnwrap(filter.window?.hitTest(filter.convert(CGPoint(x: filter.bounds.midX, y: filter.bounds.midY), to: nil), with: nil))
        XCTAssertTrue(hit.isDescendant(of: filter), "a tap on the switcher is the switcher's, not \(type(of: hit))")
        XCTAssertTrue(filter.interactions.contains { $0 is UIScrollEdgeElementContainerInteraction }, "UIKit's edge effect over the list")
        sv.contentOffset.y = 100 - list.scrollTopInset(sv)
        p.press(try node(session, "pick-chat").id)
        until("the switcher goes and a composer comes") { list.pageBand.top == 0 && list.pageBand.bottom > 0 }
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 100, accuracy: 1, "scrollTop is kept as the bands change")
        XCTAssertNil(filter.window, "the switcher is gone")
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).maxY, try XCTUnwrap(list.window).bounds.maxY, accuracy: 0.5, "the transcript runs under the composer to the screen's foot")
        XCTAssertGreaterThan(try node(session, "composer").layer.zPosition, list.layer.zPosition, "the composer draws over it")
        for shape in ["caption", "painted", "large"] {
            p.press(try node(session, "pick-\(shape)").id)
            until("\(shape) is no bar over the list") { list.pageBand == .zero }
        }
    }

    /// A transcript in a box over a composer its author filled: the composer is no bar, so
    /// the rows stop above it, and the top still runs under the inline bar, as a hand-built
    /// table under its navigation bar does whatever is pinned under it (bake-off r28's chat).
    func testAListOverAFilledComposerStillRunsUnderTheBar() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the bars and the safe area are a scene window's") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("control bars are iOS 26's, where UIKit draws the edge effect over them") }
        let (session, _) = try fixture("CONTROL_BARS")
        session.presenter.press(try node(session, "pick-filled").id)
        until("the transcript runs under the bar") { (try? self.node(session, "wrapped"))?.pageBand.top ?? 0 > 0 }
        let list = try node(session, "wrapped"), sv = try XCTUnwrap(list.scrollView), composer = try node(session, "filled-composer")
        XCTAssertEqual(list.pageBand.bottom, 0, "the filled composer keeps the rows above it")
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).minY, 0, accuracy: 0.5, "its scroll view runs up under the bar")
        XCTAssertEqual(sv.convert(sv.bounds, to: nil).maxY, composer.convert(composer.bounds, to: nil).minY, accuracy: 0.5, "and stops at the composer")
        let route = try XCTUnwrap(session.presenter.navigation.activeRoute)
        XCTAssertTrue(route.contentScrollView(for: .top) === sv, "the bar's edge effect follows it")
    }

    /// A virtualized transcript over its composer (the chat recipe), shown when its messages
    /// arrive mid-push, opens with its newest row above the composer. As the push lands its
    /// box grows under the bar and its top band goes to UIKit's inset, which leaves its port
    /// the same size: keeping its scrollTop put the last two rows under the composer, and the
    /// runner, which saw only the offset move, took it for the reader leaving the end (round
    /// 32's chat-rooms).
    func testAPushedTranscriptOpensWithItsNewestRowAboveItsComposer() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the bars and the safe area are a scene window's") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("control bars are iOS 26's, where UIKit draws the edge effect over them") }
        let (session, _) = try fixture("CONTROL_BARS")
        session.presenter.press(try node(session, "pick-thread").id)
        until("the push lands, the transcript under the bar and its composer") {
            // Not `node`: its XCTUnwrap records a failure while the messages are on their way.
            guard let list = session.presenter.views.values.first(where: { $0.props["testId"] == "transcript" }), list.window != nil else { return false }
            return list.pageBand.top == 0 && list.pageBand.bottom > 0
        }
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        let composer = try node(session, "thread-composer"), last = try node(session, "message-30")
        XCTAssertEqual(last.convert(last.bounds, to: nil).maxY, composer.convert(composer.bounds, to: nil).minY - 6, accuracy: 1,
                       "the newest message ends at the composer's margin")
    }

    func testScrollTopIsKeptWhenTheBandGoes() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, view) = try fixture()
        guard view.safeAreaInsets.top > 0 else {
            throw XCTSkip("no status bar above the page for its band to leave: the iPhone Duo's stands in a side column")
        }
        let p = session.presenter, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        sv.contentOffset.y = 100 - list.scrollTopInset(sv)
        p.press(try node(session, "head").id)
        until("a line above the scroller takes it off the top edge") { list.pageBand.top == 0 && (try? node(session, "header")) != nil }
        XCTAssertGreaterThan(list.pageBand.bottom, 0, "it keeps the bottom band")
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 100, accuracy: 0.5, "the logical scrollTop is kept")
        p.press(try node(session, "head").id)
        until("the band comes back") { list.pageBand.top > 0 }
        XCTAssertEqual(sv.contentOffset.y + list.scrollTopInset(sv), 100, accuracy: 0.5, "and kept again")
    }

    func testAScrollerThatFitsDoesNotScrollByItsBands() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the status bar and the safe area are a scene window's") }
        let (session, _) = try fixture()
        let p = session.presenter, list = try node(session, "list"), sv = try XCTUnwrap(list.scroll)
        p.press(try node(session, "shorten").id)
        until("the content is short") { (try? node(session, "filler"))?.bounds.height == 40 }
        let i = sv.adjustedContentInset
        XCTAssertLessThanOrEqual(sv.contentSize.height + i.top + i.bottom, sv.bounds.height + 0.5, "no range from the bands, as a table that fits")
    }
    func testALargeTitleCollapsesOverAFixedControlBar() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the title and safe area belong to a scene window") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("control bars require iOS 26") }
        let (session, _) = try fixture("CONTROL_BARS")
        let p = session.presenter
        p.press(try node(session, "pick-large").id)
        let filter = try node(session, "filter"), list = try node(session, "list")
        p.apply(wireBatch([["op": "props", "id": Int(filter.id), "set": ["accessibilityRole": "group"]]]))
        let route = try XCTUnwrap(p.navigation.activeRoute)
        until("the filter is page content and its bar has settled") { route.scoped == nil && !Agent(session: session).nativeInFlight() }
        let nav = try XCTUnwrap(route.navigationController), sv = try XCTUnwrap(list.scrollView)
        nav.view.layoutIfNeeded()
        let expanded = nav.navigationBar.bounds.height
        XCTAssertGreaterThan(expanded, 80)
        XCTAssertTrue(route.collapseScroll === list)
        XCTAssertGreaterThan(list.pageBand.top, 0)
        let top = try node(session, "list-top")
        XCTAssertEqual(top.convert(top.bounds, to: nil).minY, filter.convert(filter.bounds, to: nil).maxY + 8, accuracy: 1)
        let beforeDrag = sv.contentOffset.y + list.scrollTopInset(sv)
        Agent.rest(sv, at: CGPoint(x: 0, y: sv.contentOffset.y + 30), x: false, y: true)
        nav.view.layoutIfNeeded()
        let turn = expectation(description: "bar covers delivered")
        DispatchQueue.main.async { turn.fulfill() }
        wait(for: [turn], timeout: 5)
        let shortDragHeight = nav.navigationBar.bounds.height
        let shortDragTop = sv.contentOffset.y + list.scrollTopInset(sv)
        XCTAssertGreaterThan(shortDragTop, beforeDrag, "the released drag must move content")
        Agent.rest(sv, at: CGPoint(x: 0, y: 300 - sv.adjustedContentInset.top), x: false, y: true)
        nav.view.layoutIfNeeded()
        XCTAssertLessThan(nav.navigationBar.bounds.height, 65)
        let collapsed = nav.navigationBar.bounds.height
        XCTAssertEqual(min(abs(shortDragHeight - expanded), abs(shortDragHeight - collapsed)), 0, accuracy: 0.5,
                       "the short drag rests at an exact native title height")
        XCTAssertEqual(shortDragTop, beforeDrag + expanded - collapsed, accuracy: 0.5,
                       "the offset follows the same native snap instead of returning to the top")
        XCTAssertEqual(filter.convert(filter.bounds, to: route.view).minY, route.view.safeAreaInsets.top + 8, accuracy: 1)
        // A route that clips to its own box (`overflow="hidden"` at `inset=0`) keeps its bars: no band passes its edge.
        route.node.clipsToBounds = true
        defer { route.node.clipsToBounds = false }
        XCTAssertEqual(ControlBars(p).bars(of: list, in: route.node, top: true)?.isEmpty, false)
    }

    func testLargeTitleRidersRequireBothEdgesAndTheBarsPaintOrder() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the title belongs to a scene window") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("control bars require iOS 26") }
        let (session, _) = try fixture("CONTROL_BARS")
        let p = session.presenter
        p.press(try node(session, "pick-large").id)
        let filter = try node(session, "filter"), list = try node(session, "list")
        p.apply(wireBatch([["op": "props", "id": Int(filter.id), "set": ["accessibilityRole": "group"]]]))
        let route = try XCTUnwrap(p.navigation.activeRoute)
        until("the filter is page content and its bar has settled") { route.scoped == nil && !Agent(session: session).nativeInFlight() }
        let nav = try XCTUnwrap(route.navigationController)
        XCTAssertTrue(route.collapseScroll === list)
        p.onCovers = nil
        let originalStyle = list.style, originalFrame = list.frame
        list.style["written"] = .array([.string("z_index")])
        list.style["z_index"] = .number(100)
        list.setRank(filter.paintRank + 100)
        for _ in 0..<2 {
            p.navigation.prepareRoutes([route], in: nav)
            p.reachPageEdges()
            XCTAssertNil(route.collapseScroll, "a scroller above its bar cannot drive collapse")
            XCTAssertTrue(route.ridingBars.isEmpty)
            XCTAssertEqual(list.pageBand.top, 0)
            XCTAssertEqual(filter.controlRide, 0)
        }
        list.style = originalStyle
        list.setRank(filter.paintRank - 1)
        list.frame = CGRect(x: originalFrame.minX, y: originalFrame.minY, width: originalFrame.width, height: originalFrame.height - 40)
        for _ in 0..<2 {
            p.navigation.prepareRoutes([route], in: nav)
            p.reachPageEdges()
            XCTAssertNil(route.collapseScroll, "an open opposite edge must reach the route's edge")
            XCTAssertTrue(route.ridingBars.isEmpty)
            XCTAssertEqual(list.pageBand.top, 0)
        }
    }

    func testAControlBarMayIncludeACaptionButACaptionAloneIsNotABar() {
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea"],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Messages are saved locally"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Signed in as Ada"]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "roots", "ids": [1, 4]]
        ]))
        XCTAssertTrue(ControlBars(p).isBar(p.views[1]!))
        XCTAssertFalse(ControlBars(p).isBar(p.views[4]!))
        p.views[1]?.style["written"] = .array([.string("background_color")])
        XCTAssertFalse(ControlBars(p).isBar(p.views[1]!))
    }

}
#endif
