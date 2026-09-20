// @ref LLP 1043.000 §3 D8 — native samples commit viewport deltas while held.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
final class LayoutPanTests: XCTestCase {
    func testHeldPanUsesIncrementalDownwardViewportCoordinatesAndRetiresDisabledOwner() {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = p.viewport
        defer { window.close() }
        let node = NodeView(id: 7, kind: "view", presenter: p)
        node.handlers = ["pan"]; p.views[7] = node; p.root.addSubview(node)
        let child = NodeView(id: 8, kind: "view", presenter: p)
        node.addSubview(child); p.views[8] = child
        var samples: [CGPoint] = []
        p.onPan = { id, dx, dy in XCTAssertEqual(id,7); samples.append(CGPoint(x: dx,y: dy)) }
        func event(_ x: Double, _ y: Double, _ kind: NSEvent.EventType = .leftMouseDragged) -> NSEvent {
            NSEvent.mouseEvent(with: kind, location: NSPoint(x: x,y: y),modifierFlags: [],timestamp: 0,windowNumber: window.windowNumber,context: nil,eventNumber: 0,clickCount: 1,pressure: 1)!
        }
        XCTAssertTrue(p.mouseLayoutPan.down(child,event: event(100,300,.leftMouseDown)))
        XCTAssertTrue(p.mouseLayoutPan.drag(event(103,298)))
        XCTAssertTrue(samples.isEmpty)
        XCTAssertTrue(p.mouseLayoutPan.drag(event(120,260)))
        XCTAssertEqual(samples,[CGPoint(x:20,y:40)])
        // Layout can move the handler; viewport deltas must not feed back its position.
        node.frame.origin = CGPoint(x: 20,y: 40)
        XCTAssertTrue(p.mouseLayoutPan.drag(event(130,250)))
        XCTAssertEqual(samples.last,CGPoint(x:10,y:10))
        node.props["disabled"] = "true"
        XCTAssertFalse(p.mouseLayoutPan.drag(event(140,240)))
        XCTAssertEqual(samples.count,2)
        XCTAssertFalse(p.mouseLayoutPan.up(event(140,240,.leftMouseUp)))
    }
}
#endif
