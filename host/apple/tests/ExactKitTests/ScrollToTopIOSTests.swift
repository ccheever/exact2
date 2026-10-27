#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A scroll to the top under a collapsed large title heads past the
/// scroller's top, to the open title's inset, and UIKit puts an offset there
/// back at the top when the content size changes (`NodeView.holdsSize`). A
/// new content size waits while the offset is there, for as long as the
/// scroll runs: until its animation ends, or a drag begins. Its second-long
/// bound sets nothing while the offset is still past the top. The tree is the
/// test's own, a scroller and a list in a window, and the scroll to the top
/// is begun and ended through the scroller's delegate, as UIKit's are; the
/// whole of it under a large title is
/// `NavigationTabsIOSTests.testTheSelectedTabTappedAgainAtItsRootScrollsToTheTop`.
final class ScrollToTopIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    /// A scroller (2) and a list (4), each 400 by 800 over 1,200 points of content.
    private func mounted() throws -> Presenter {
        let session = ExactApp.shared.makeSession(label: "scroll-to-top")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 800, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 800, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 4]], ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 800.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 1200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 1200.0],
            ["op": "frame", "id": 4, "x": 400.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "content", "id": 4, "w": 400.0, "h": 1200.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 400.0, "h": 1200.0],
        ]))
        p.viewport.layoutIfNeeded()
        let sv = try XCTUnwrap(p.views[2]?.scroll)
        XCTAssertTrue(sv.delegate === p.views[2], "the scroller hears its own scroll's end")
        XCTAssertEqual(sv.contentSize.height, 1200)
        return p
    }

    /// The scroller's content grows to `height`, as a `cqh` length grows it while the title opens.
    private func grow(_ p: Presenter, to height: Double) {
        p.apply(wireBatch([["op": "content", "id": 2, "w": 400.0, "h": height], ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": height]]))
    }

    /// The offset `by` points past the scroller's top, where the scroll to the top heads.
    private func past(_ sv: UIScrollView, by: CGFloat) {
        sv.contentOffset = CGPoint(x: 0, y: -sv.adjustedContentInset.top - by)
    }

    /// Waits out the wall clock to `deadline`: what is checked is what a
    /// bound measured on it, the scroll's fail-safe, does or does not set.
    private func wait(until deadline: Date) {
        while Date() < deadline { RunLoop.main.run(mode: .default, before: deadline) }
    }

    private func until(_ what: String, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(5)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(done(), what)
    }

    /// A scroll to the top whose animation outlasts its second-long bound:
    /// at the bound the offset is still past the top, so the new size still
    /// waits, and it is set once the offset is back in range.
    func testANewSizeWaitsPastTheBoundWhileTheOffsetIsPastTheTop() throws {
        let p = try mounted(), node = try XCTUnwrap(p.views[2]), sv = try XCTUnwrap(node.scroll)
        let began = Date()
        node.scrollsToTop()
        past(sv, by: 40)
        grow(p, to: 1300)
        XCTAssertEqual(sv.contentSize.height, 1200, "the new size waits while the offset is past the top")
        wait(until: began.addingTimeInterval(1.3))
        XCTAssertEqual(sv.contentSize.height, 1200, "past the bound, the offset still past the top: the size still waits")
        XCTAssertEqual(sv.contentOffset.y, -sv.adjustedContentInset.top - 40, accuracy: 0.5, "and the offset stays where the scroll put it")
        sv.contentOffset.y = -sv.adjustedContentInset.top
        XCTAssertEqual(sv.contentSize.height, 1300, "the offset back in range: the size is set")
        node.scrollViewDidEndScrollingAnimation(sv)
        past(sv, by: 40)
        grow(p, to: 1400)
        XCTAssertEqual(sv.contentSize.height, 1400, "the scroll over, a size is set at once")
    }

    /// The scroll to the top ends with its animation, the tab's
    /// (`scrollViewDidEndScrollingAnimation`) or the status bar's
    /// (`scrollViewDidScrollToTop`), well inside its bound: an offset past the
    /// top after that, as a released pull's, holds nothing back.
    func testTheScrollsEndEndsTheWait() throws {
        let p = try mounted(), node = try XCTUnwrap(p.views[2]), sv = try XCTUnwrap(node.scroll)
        node.scrollsToTop()
        past(sv, by: 40)
        grow(p, to: 1300)
        XCTAssertEqual(sv.contentSize.height, 1200, "the new size waits")
        sv.contentOffset.y = -sv.adjustedContentInset.top
        XCTAssertEqual(sv.contentSize.height, 1300, "the offset back in range: the size is set")
        node.scrollViewDidEndScrollingAnimation(sv)
        past(sv, by: 40)
        grow(p, to: 1400)
        XCTAssertEqual(sv.contentSize.height, 1400, "the tab's scroll over, a size is set at once")
        sv.contentOffset.y = 300
        XCTAssertTrue(node.scrollViewShouldScrollToTop(sv), "the status bar's tap scrolls to the top")
        past(sv, by: 40)
        grow(p, to: 1500)
        XCTAssertEqual(sv.contentSize.height, 1400, "the new size waits")
        sv.contentOffset.y = -sv.adjustedContentInset.top
        sv.delegate?.scrollViewDidScrollToTop?(sv)
        past(sv, by: 40)
        grow(p, to: 1600)
        XCTAssertEqual(sv.contentSize.height, 1600, "the status bar's scroll over, a size is set at once")
    }

    /// A drag begun during the scroll to the top ends it: the waiting size is
    /// set as the finger takes the content, and the pull it releases holds
    /// nothing back.
    func testADragEndsTheWait() throws {
        let p = try mounted(), node = try XCTUnwrap(p.views[2]), sv = try XCTUnwrap(node.scroll)
        node.scrollsToTop()
        past(sv, by: 40)
        grow(p, to: 1300)
        XCTAssertEqual(sv.contentSize.height, 1200, "the new size waits")
        node.scrollViewWillBeginDragging(sv)
        XCTAssertEqual(sv.contentSize.height, 1300, "a drag begins: the size is set")
        past(sv, by: 60)
        grow(p, to: 1400)
        XCTAssertEqual(sv.contentSize.height, 1400, "a released pull holds nothing back")
    }

    /// A second scroll to the top inside the first's bound has a bound of its
    /// own: past the first's, the new size still waits, and once the offset
    /// is in range with no scroll to say so (the inset grew under it), the
    /// second's bound sets the size.
    func testASecondScrollToTheTopHasItsOwnBound() throws {
        let p = try mounted(), node = try XCTUnwrap(p.views[2]), sv = try XCTUnwrap(node.scroll)
        let began = Date()
        node.scrollsToTop()
        past(sv, by: 40)
        grow(p, to: 1300)
        XCTAssertEqual(sv.contentSize.height, 1200, "the new size waits")
        wait(until: began.addingTimeInterval(0.5))
        node.scrollsToTop()
        wait(until: began.addingTimeInterval(1.15))
        XCTAssertEqual(sv.contentSize.height, 1200, "past the first scroll's bound, the second's runs: the size still waits")
        sv.contentInset.top += 40
        XCTAssertGreaterThanOrEqual(sv.contentOffset.y, -sv.adjustedContentInset.top - 0.5, "the offset in range with the inset grown under it")
        XCTAssertEqual(sv.contentSize.height, 1200, "which no scroll reported")
        until("the second scroll's bound sets the size") { sv.contentSize.height == 1300 }
    }

    /// A list sets its own content size, from its rows' extent
    /// (`CollectionHost.fit`): it waits as a scroller's does.
    func testAListsNewSizeWaitsAsAScrollersDoes() throws {
        let p = try mounted(), node = try XCTUnwrap(p.views[4]), sv = try XCTUnwrap(node.scroll)
        node.scrollsToTop()
        past(sv, by: 40)
        p.collections.shift(4, by: 0, extent: 1300, from: nil)
        XCTAssertEqual(sv.contentSize.height, 1200, "the list's new extent waits while the offset is past the top")
        sv.contentOffset.y = -sv.adjustedContentInset.top
        XCTAssertEqual(sv.contentSize.height, 1300, "the offset back in range: the list's size is set")
    }
}
#endif
