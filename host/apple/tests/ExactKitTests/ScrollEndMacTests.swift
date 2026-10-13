#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// DOM's `scrollend` on AppKit (`ScrollEndMac.swift`): a live scroll ends
/// with its notification, on the next turn; a move outside one ends once the offset
/// has been still for `ScrollEnd.quiet`; each scroll ends once.
final class ScrollEndMacTests: XCTestCase {
    private func scroller() throws -> (Presenter, NodeView, NSScrollView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["scrollend"], "style": ["overflow_x": "hidden", "overflow_y": "auto"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
        ]))
        let n = try XCTUnwrap(p.views[1]), sv = try XCTUnwrap(n.scroll)
        sv.documentView?.setFrameSize(NSSize(width: 100, height: 1000))
        return (p, n, sv)
    }
    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
    private func move(_ sv: NSScrollView, to y: CGFloat) { sv.contentView.scroll(to: NSPoint(x: 0, y: y)); sv.reflectScrolledClipView(sv.contentView) }

    func testAMoveOutsideALiveScrollEndsOnceStill() throws {
        let (p, _, sv) = try scroller()
        var ends: [UInt32] = []
        p.onScrollend = { ends.append($0) }
        move(sv, to: 40); move(sv, to: 80)
        XCTAssertEqual(ends, [], "not before it has been still")
        spin(ScrollEnd.quiet + 0.1)
        XCTAssertEqual(ends, [1], "once, after the last move")
        spin(ScrollEnd.quiet + 0.05)
        XCTAssertEqual(ends, [1], "nothing more without another move")
    }

    func testAResizeThatLeavesTheOffsetEndsNothing() throws {
        let (p, _, sv) = try scroller()
        var ends: [UInt32] = []
        p.onScrollend = { ends.append($0) }
        sv.setFrameSize(NSSize(width: 100, height: 60))
        sv.contentView.setFrameSize(NSSize(width: 100, height: 60))
        spin(ScrollEnd.quiet + 0.1)
        XCTAssertEqual(ends, [], "the clip view's bounds changed size, not origin")
    }

    func testALiveScrollEndsWithItsNotificationAtOnce() throws {
        let (p, _, sv) = try scroller()
        var ends: [UInt32] = []
        p.onScrollend = { ends.append($0) }
        NotificationCenter.default.post(name: NSScrollView.willStartLiveScrollNotification, object: sv)
        move(sv, to: 60)
        spin(ScrollEnd.quiet + 0.1)
        XCTAssertEqual(ends, [], "a live scroll holds its end, its momentum included")
        NotificationCenter.default.post(name: NSScrollView.didEndLiveScrollNotification, object: sv)
        spin(0.02)
        XCTAssertEqual(ends, [1], "on the next turn, after the end's other observers")
        // A live scroll that moved nothing ends nothing.
        NotificationCenter.default.post(name: NSScrollView.willStartLiveScrollNotification, object: sv)
        NotificationCenter.default.post(name: NSScrollView.didEndLiveScrollNotification, object: sv)
        spin(0.02)
        XCTAssertEqual(ends, [1])
    }
}
#endif
