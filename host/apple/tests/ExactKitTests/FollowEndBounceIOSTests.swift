#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A `scrollFollowEnd` scroll view keeps the end or the reader's place after
/// every batch, and leaves the rubber band alone while it is out of range
/// and moving (a chat's bottom bounce jittered under a spinner's batches).
final class FollowEndBounceIOSTests: XCTestCase {
    func testAtRestTheEndAndThePlaceAreKeptInRange() {
        XCTAssertEqual(NodeView.followedTop(current: 400, minimum: 0, maximum: 500, end: true, top: 400, moving: false), 500)
        XCTAssertEqual(NodeView.followedTop(current: 200, minimum: 0, maximum: 500, end: false, top: 260, moving: false), 260)
        XCTAssertEqual(NodeView.followedTop(current: 200, minimum: 0, maximum: 500, end: false, top: 900, moving: false), 500)
    }

    func testAPullPastTheEndIsLeftToTheRubberBand() {
        XCTAssertEqual(NodeView.followedTop(current: 560, minimum: 0, maximum: 500, end: true, top: 560, moving: true), 560)
        XCTAssertEqual(NodeView.followedTop(current: -40, minimum: 0, maximum: 500, end: false, top: -40, moving: true), -40)
        // A row above changing height still moves the reader's place.
        XCTAssertEqual(NodeView.followedTop(current: -40, minimum: 0, maximum: 500, end: false, top: -10, moving: true), -10)
    }

    func testAScrollInRangeStillFollows() {
        XCTAssertEqual(NodeView.followedTop(current: 499.8, minimum: 0, maximum: 500, end: true, top: 499.8, moving: true), 500)
    }
}
#endif
