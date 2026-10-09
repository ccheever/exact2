#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// #139: a browser's hover follows layout and scrolling under a pointer that
/// does not move; AppKit's tracking areas report only a pointer that moves,
/// so the presenter hit-tests the resting pointer at the next display frame.
final class HoverFollowsLayoutMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    /// Rows a, b, c (ids 2–4), 60 points tall, in a column (id 1).
    private func rows() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["hover"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["hover"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["hover"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 60.0, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 120.0, "w": 300.0, "h": 60.0]
        ]))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return p
    }
    /// The pointer resting on row b's middle (the stub for the cursor), b hovered.
    private func restOnB(_ p: Presenter) -> NSPoint {
        let b = p.views[3]!
        let at = b.convert(NSPoint(x: b.bounds.midX, y: b.bounds.midY), to: nil)
        p.agentPointer = at
        p.hover(b, true)
        return at
    }
    /// Row a goes: b and c move up a row.
    private func removeA(_ p: Presenter) {
        p.apply(wireBatch([
            ["op": "children", "id": 1, "ids": [3, 4]],
            ["op": "destroy", "id": 2],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 60.0, "w": 300.0, "h": 60.0]
        ]))
    }

    func testViewportMovesHoverTheHitSubtreeIncludingOverflowWithoutTracking() {
        let p = rows()
        p.apply(wireBatch([
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "frame", "id": 2, "x": 20, "y": 20, "w": 120, "h": 20],
            ["op": "frame", "id": 3, "x": 0, "y": 20, "w": 120, "h": 80],
            ["op": "frame", "id": 4, "x": 20, "y": 30, "w": 80, "h": 24],
        ]))
        var log: [String] = []
        p.onHover = { log.append("\($0):\($1)") }
        func move(_ node: NodeView, _ local: NSPoint) {
            let point = node.convert(local, to: nil)
            let event = NSEvent.mouseEvent(with: .mouseMoved, location: point, modifierFlags: [], timestamp: 0,
                windowNumber: window!.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0)!
            p.viewport.mouseMoved(with: event)
        }
        move(p.views[2]!, NSPoint(x: 10, y: 10))
        move(p.views[3]!, NSPoint(x: 10, y: 10))
        move(p.views[4]!, NSPoint(x: 10, y: 10))
        move(p.views[3]!, NSPoint(x: 10, y: 10))
        XCTAssertEqual(log, ["2:true", "3:true", "4:true", "4:false"], "ancestors remain hovered over every descendant")
        move(p.views[1]!, NSPoint(x: 250, y: 250))
        XCTAssertEqual(Array(log.suffix(2)), ["3:false", "2:false"])
        // A card without its own listener still extends the parent's hover.
        p.views[3]!.handlers = []; p.views[4]!.handlers = []
        log.removeAll()
        move(p.views[2]!, NSPoint(x: 10, y: 10))
        move(p.views[4]!, NSPoint(x: 10, y: 10))
        XCTAssertEqual(log, ["2:true"])
        move(p.views[1]!, NSPoint(x: 250, y: 250))
        XCTAssertEqual(log, ["2:true", "2:false"])
    }

    func testOverlappingTrackingAreasOnlyHoverTheTopmostHitAndViewportExitLeaves() {
        let p = rows()
        p.apply(wireBatch([
            ["op": "frame", "id": 2, "x": 20, "y": 20, "w": 150, "h": 100],
            ["op": "frame", "id": 3, "x": 40, "y": 40, "w": 100, "h": 50],
            ["op": "frame", "id": 4, "x": 0, "y": 200, "w": 300, "h": 60],
        ]))
        var log: [String] = []
        p.onHover = { log.append("\($0):\($1)") }
        for x in [10.0, 12.0, 14.0, 16.0, 18.0] {
            let point = p.views[3]!.convert(NSPoint(x: x, y: 10), to: nil)
            let event = NSEvent.mouseEvent(with: .mouseMoved, location: point, modifierFlags: [], timestamp: 0,
                windowNumber: window!.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0)!
            p.views[2]!.mouseMoved(with: event) // occluded tracking owner
            p.views[3]!.mouseMoved(with: event)
            p.viewport.mouseMoved(with: event)
        }
        XCTAssertEqual(log, ["3:true"])
        let exit = NSEvent.enterExitEvent(with: .mouseExited, location: NSPoint(x: -20, y: -20), modifierFlags: [], timestamp: 0,
            windowNumber: window!.windowNumber, context: nil, eventNumber: 0, trackingNumber: 0, userData: nil)!
        p.viewport.mouseExited(with: exit)
        XCTAssertEqual(log, ["3:true", "3:false"])
    }

    func testAncestorEnterCannotPublishAStaleDescendantAfterReentrantHover() {
        let p = rows()
        let parent = p.views[2]!, child = p.views[3]!
        parent.addSubview(child)
        var log: [String] = []
        p.onHover = { id, over in
            log.append("\(id):\(over)")
            if id == parent.id, over { p.setHoverPath([parent]) }
        }
        p.setHoverPath([child, parent])
        XCTAssertEqual(log, ["2:true"], "the unentered child neither enters nor leaves")
        XCTAssertEqual(p.hoveredNodes.map(\.id), [parent.id])
        p.setHoverPath([])
        XCTAssertEqual(log, ["2:true", "2:false"])
    }

    func testAncestorEnterThatHidesOrRemovesItsDescendantDoesNotEnterIt() {
        for remove in [false, true] {
            let p = rows()
            let parent = p.views[2]!, child = p.views[3]!
            parent.addSubview(child)
            var log: [String] = []
            p.onHover = { id, over in
                log.append("\(id):\(over)")
                if id == parent.id, over {
                    if remove { child.removeFromSuperview() } else { child.isHidden = true }
                }
            }
            p.setHoverPath([child, parent])
            XCTAssertEqual(log, ["2:true"])
            XCTAssertEqual(p.hoveredNodes.map(\.id), [parent.id])
            p.setHoverPath([])
            XCTAssertEqual(log, ["2:true", "2:false"])
        }
    }

    func testTheRowThatSlidUnderTheRestingPointerEntersAndTheOneThatLeftLeaves() {
        let p = rows()
        _ = restOnB(p)
        var log: [String] = []
        p.onHover = { id, over in log.append("\(id) \(over)") }
        removeA(p)
        XCTAssertTrue(p.followPending, "the batch moved boxes: a hit-test waits for the next frame")
        XCTAssertEqual(log, [], "nothing before the frame")
        p.hoverUnderPointer() // the frame
        XCTAssertFalse(p.followPending)
        XCTAssertEqual(log, ["3 false", "4 true"], "b leaves, then c enters, with no mouse event")
        XCTAssertTrue(p.hovered === p.views[4])
        p.hoverUnderPointer()
        XCTAssertEqual(log.count, 2, "the hovered row still under the pointer hears nothing more")
        // The next real move: b's tracking area exits and c's enters and
        // moves; each was already told, so nothing is sent twice.
        let move = NSEvent.mouseEvent(with: .mouseMoved, location: p.agentPointer!, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                      windowNumber: window!.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0)!
        p.views[3]!.mouseExited(with: move)
        p.views[4]!.mouseEntered(with: move)
        p.views[4]!.mouseMoved(with: move)
        XCTAssertEqual(log, ["3 false", "4 true"], "one leave and one enter, the web's mouseleave/mouseenter")
    }

    func testNothingFollowsWhileAButtonIsDownOrThePointerIsOutside() {
        let p = rows()
        _ = restOnB(p)
        var log: [String] = []
        p.onHover = { id, over in log.append("\(id) \(over)") }
        p.pointerHeld = 3
        removeA(p)
        XCTAssertFalse(p.followPending, "a button is down: no hit-test is scheduled")
        p.hoverUnderPointer()
        XCTAssertEqual(log, [], "nor does one hover")
        p.pointerHeld = nil
        p.agentPointer = NSPoint(x: -20, y: -20)
        p.apply(wireBatch([["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0]]))
        XCTAssertFalse(p.followPending, "outside the window: none either")
        p.hoverUnderPointer()
        XCTAssertEqual(log, [])
    }

    /// A fling moves the page every tick; the hit-test is the frame's, one,
    /// at where the content ended up.
    func testAScrollFlingIsHitTestedOnceAFrame() {
        let p = rows()
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 900.0]]))
        let at = restOnB(p)
        var log: [String] = []
        p.onHover = { id, over in log.append("\(id) \(over)") }
        p.hoverUnderPointer() // the last batch's frame
        XCTAssertFalse(p.followPending)
        p.viewport.contentView.scroll(to: NSPoint(x: 0, y: 20))
        let link = p.followLink
        XCTAssertTrue(p.followPending, "the scroll moved the content under the pointer")
        for y in [30.0, 45.0, 60.0] {
            p.viewport.contentView.scroll(to: NSPoint(x: 0, y: y))
            XCTAssertTrue(p.followLink === link && p.followPending, "one link and one hit-test a frame, however many ticks")
        }
        XCTAssertEqual(log, [], "nothing until the frame")
        p.hoverUnderPointer()
        XCTAssertEqual(log, ["3 false", "4 true"], "the row the fling brought under the pointer")
        XCTAssertEqual(p.restingPointer(), at)
    }
}
#endif
