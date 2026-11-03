#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1070.000 §2.5 under UIKit: an input counts against the request under
/// way when it began, by its own timestamp against the time the request
/// reached the host. And §2.6's selected tab tapped again: its scroll to the
/// top gives way to a request the tab's press made, told by the list's latest
/// request before and after the press.
final class CollectionInputIOSTests: XCTestCase {
    /// A list whose request waits.
    private func list() -> Presenter {
        let p = Presenter()
        p.apply(wireBatch([["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
                "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": NSNull(), "waiting": true, "request": 1, "lastRequest": 1]]],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"],
            ["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        return p
    }

    func testAnInputCountsAgainstTheRequestUnderWayWhenItBegan() throws {
        let p = list()
        defer { p.collections.reset() }
        let requestAt = try XCTUnwrap(p.collections.entries[1]).requestAt
        p.collections.gestureBegan(1, at: requestAt - 0.01)
        XCTAssertFalse(try XCTUnwrap(p.collections.entries[1]).gesture, "a touch that began before the request")
        p.collections.gestureBegan(1, at: requestAt + 0.01)
        XCTAssertTrue(try XCTUnwrap(p.collections.entries[1]).gesture, "one that began after it")
    }

    /// The selected tab tapped again scrolls its screen's list to the top:
    /// the reader's move, which ends a request made before the tap, and
    /// gives way to any the tab's press made in answer, under way or ended.
    func testAScrollToTheTopGivesWayToARequestTheTabsPressMade() throws {
        let p = list()
        defer { p.collections.reset() }
        let entry = try XCTUnwrap(p.collections.entries[1]), scroll = try XCTUnwrap(p.views[1]?.scroll)
        let route = RouteController(try XCTUnwrap(p.views[1]))
        p.navigation.scrollsToTopAnimated = false
        scroll.contentOffset.y = 600
        p.navigation.scrollToTop(route, since: [1: 0])
        XCTAssertFalse(entry.gesture, "a request the press made, under way, is its answer to the tap")
        XCTAssertEqual(scroll.contentOffset.y, 600, accuracy: 0.5, "which the scroll to the top gives way to")
        p.navigation.scrollToTop(route, since: [:])
        XCTAssertEqual(scroll.contentOffset.y, 600, accuracy: 0.5, "as to one in a list the press made")
        // The next press's request was `done` at once: the port already showed its row.
        p.apply(wireBatch([["op": "collections", "items": [["view": 1, "revision": 2, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": NSNull(), "lastRequest": 2]]]]))
        p.navigation.scrollToTop(route, since: [1: 1])
        XCTAssertEqual(scroll.contentOffset.y, 600, accuracy: 0.5, "and so is one done at once, its row already shown")
        // A press that asked for nothing: the request under way is from before the tap.
        p.apply(wireBatch([["op": "collections", "items": [["view": 1, "revision": 3, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": [["view": 10, "root": 10, "epoch": 1]], "correction": NSNull(), "waiting": true, "request": 3, "lastRequest": 3]]]]))
        p.navigation.scrollToTop(route, since: [1: 3])
        XCTAssertTrue(entry.gesture, "a request made before the tap ends")
        XCTAssertEqual(scroll.contentOffset.y, -scroll.adjustedContentInset.top, accuracy: 0.5, "and the list goes to its top")
    }

    /// A finger coming down on a list while its request is under way is the
    /// reader's gesture against it, heard by the host's contact recognizer
    /// (LLP 1070.000 §2.6); one beside the list is not.
    func testAFingerComingDownOnTheListIsTheReadersGesture() throws {
        let p = list()
        defer { p.collections.reset() }
        let contact = p.collections.watchContacts(in: p.viewport)
        defer { p.viewport.removeGestureRecognizer(contact) }
        let after = try XCTUnwrap(p.collections.entries[1]).requestAt + 0.01
        let beside = Touch(p.viewport, at: after), onRow = Touch(try XCTUnwrap(p.views[10]), at: after)
        contact.touchesBegan([beside], with: UIEvent())
        XCTAssertFalse(try XCTUnwrap(p.collections.entries[1]).gesture, "beside the list")
        contact.touchesEnded([beside], with: UIEvent())
        contact.touchesBegan([onRow], with: UIEvent())
        XCTAssertTrue(try XCTUnwrap(p.collections.entries[1]).gesture, "on its row")
    }

    private final class Touch: UITouch {
        let target: UIView, time: TimeInterval
        init(_ target: UIView, at time: TimeInterval) { self.target = target; self.time = time; super.init() }
        override var view: UIView? { target }
        override var timestamp: TimeInterval { time }
    }
}
#endif
