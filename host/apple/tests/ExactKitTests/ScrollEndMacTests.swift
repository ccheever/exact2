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
    func testNestedSmoothJumpUsesTheInnerDestination() throws {
        let (p, _, outer) = try scroller()
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "target"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 300.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 400.0, "w": 50.0, "h": 50.0],
        ]))
        let inner = try XCTUnwrap(p.views[2]?.scroll)
        let clips = [inner, outer].map { sv -> DeferredClip in
            let document = sv.documentView
            let clip = DeferredClip(frame: sv.contentView.frame)
            sv.contentView = clip; sv.documentView = document
            document?.setFrameSize(NSSize(width: 100, height: 1200))
            return clip
        }
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = p.viewport
        defer { withExtendedLifetime(window) {} }
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }
        p.scrollElementIntoView(["target", "start", "nearest", "smooth"])
        XCTAssertEqual(clips[0].destination?.y, 400)
        XCTAssertEqual(clips[1].destination?.y, 300)
        clips.forEach { $0.land() }
        let target = try XCTUnwrap(p.views[3])
        XCTAssertEqual(target.convert(target.bounds, to: outer).minY, 0)
    }

    private func interruptElementJump(wheel: Bool) throws {
        let (p, node, sv) = try scroller()
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "far"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "visible"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 400.0, "w": 50.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 50.0, "h": 1.0],
        ]))
        sv.documentView?.setFrameSize(NSSize(width: 100, height: 1000))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = p.viewport
        defer { withExtendedLifetime(window) {} }
        let prior = DisplayPreferences.agent
        DisplayPreferences.agent = (false, false)
        defer { DisplayPreferences.agent = prior }
        p.scrollElementIntoView(["far", "start", "nearest", "smooth"])
        XCTAssertNotNil(node.scrollEnd?.elementAnimation)
        // Deliver an intermediate animation frame without relying on the
        // window server's animation clock. The bounds observer owns the end.
        move(sv, to: 40)
        XCTAssertTrue(node.scrollEnd?.owed == true)
        let ended = expectation(description: "interrupted scroll ends")
        var ends = 0
        p.onScrollend = { _ in ends += 1; ended.fulfill() }
        if wheel {
            let event = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 2, wheel1: 0, wheel2: -1, wheel3: 0))
            sv.scrollWheel(with: try XCTUnwrap(NSEvent(cgEvent: event)))
        } else {
            p.scrollElementIntoView(["visible", "nearest", "nearest", "instant"])
        }
        XCTAssertNil(node.scrollEnd?.elementAnimation)
        wait(for: [ended], timeout: 1)
        XCTAssertEqual(sv.contentView.bounds.minY, 40)
        XCTAssertEqual(ends, 1)
    }

    func testVisibleRequestCancelsAnElementAnimation() throws { try interruptElementJump(wheel: false) }
    func testChainedWheelStillEndsTheInterruptedAnimation() throws { try interruptElementJump(wheel: true) }

}

/// Records where the presenter asks the animator to land while its bounds
/// stay at the current frame, as during an asynchronous AppKit animation.
private final class DeferredClip: NSClipView {
    var destination: NSPoint?
    func land() { if let destination { super.setBoundsOrigin(destination) } }
    override func animator() -> Self { self }
    override func setBoundsOrigin(_ newOrigin: NSPoint) { destination = newOrigin }
}
#endif
