#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A textarea's `resize` grip on the Mac: its vertical scroller ends above
/// the grip's corner, as Chrome's scrollbar ends above its scroll corner, and
/// a drag's moves reach the runner once a display frame, the last at once
/// when the button comes up.
final class ResizeGripMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func presenter(resize: String) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        let lines = (1...40).map { "Line \($0)" }.joined(separator: "\n")
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "textarea", "props": ["value": lines], "style": ["resize": resize]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "frame", "id": 2, "x": 20, "y": 20, "w": 300, "h": 80],
        ]))
        return p
    }

    /// The vertical scroller's bottom in the textarea's own space, once its
    /// text is laid out and overflows: an overlay scroller has no frame
    /// until there is something to scroll.
    private func scrollerBottom(_ node: NodeView) throws -> CGFloat {
        let scroll = try XCTUnwrap(node.textAreaScroll)
        let text = try XCTUnwrap(node.textArea)
        text.layoutManager?.ensureLayout(for: try XCTUnwrap(text.textContainer))
        text.sizeToFit()
        scroll.tile()
        XCTAssertGreaterThan(text.frame.height, scroll.contentSize.height, "the text overflows")
        let vertical = try XCTUnwrap(scroll.verticalScroller)
        return node.convert(vertical.frame, from: scroll).maxY
    }

    func testTheVerticalScrollerEndsAboveTheGrip() throws {
        let p = presenter(resize: "vertical")
        let node = try XCTUnwrap(p.views[2])
        let grip = try XCTUnwrap(node.subviews.first { $0 is ResizeGrip })
        XCTAssertEqual(try scrollerBottom(node), grip.frame.minY, accuracy: 0.01, "the scroller stops at the grip's top")
        XCTAssertGreaterThan(try scrollerBottom(node), 0)
        // A taller box: the scroller follows its new corner.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 20, "y": 20, "w": 300, "h": 140]]))
        XCTAssertEqual(try scrollerBottom(node), grip.frame.minY, accuracy: 0.01)
        XCTAssertEqual(grip.frame, node.resizeGripFrame)
        // No grip: the scroller reaches the scroll view's bottom again.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["resize": "none"]]]))
        // This presenter has no text engine, so its restyle stops before the
        // textarea's layout; lay it out as the next pass would.
        node.layoutTextArea()
        XCTAssertFalse(node.subviews.contains { $0 is ResizeGrip })
        let scroll = try XCTUnwrap(node.textAreaScroll)
        XCTAssertEqual(try scrollerBottom(node), node.convert(scroll.bounds, from: scroll).maxY, accuracy: 1.5,
                       "the scroller's own end, AppKit's, within its border")
    }

    func testANoneTextareaKeepsAppKitsScroller() throws {
        let p = presenter(resize: "none")
        let node = try XCTUnwrap(p.views[2])
        XCTAssertFalse(node.subviews.contains { $0 is ResizeGrip })
        let scroll = try XCTUnwrap(node.textAreaScroll)
        XCTAssertEqual(try scrollerBottom(node), node.convert(scroll.bounds, from: scroll).maxY, accuracy: 1.5)
    }

    func testADragsMovesGoOnceAFrameAndTheLastAtTheButtonsRise() throws {
        let p = presenter(resize: "both")
        let node = try XCTUnwrap(p.views[2])
        let grip = try XCTUnwrap(node.subviews.first { $0 is ResizeGrip } as? ResizeGrip)
        var sent: [(UInt32, CGFloat?, CGFloat?)] = []
        p.onResizeBox = { sent.append(($0, $1, $2)) }
        let corner = NSPoint(x: grip.frame.midX, y: grip.frame.midY)
        let event = { (type: NSEvent.EventType, dx: CGFloat, dy: CGFloat) in
            NSEvent.mouseEvent(with: type, location: node.convert(NSPoint(x: corner.x + dx, y: corner.y + dy), to: nil), modifierFlags: [],
                               timestamp: 1, windowNumber: self.window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
        }
        grip.mouseDown(with: event(.leftMouseDown, 0, 0))
        grip.mouseDragged(with: event(.leftMouseDragged, 10, 5))
        grip.mouseDragged(with: event(.leftMouseDragged, 40, 30))
        XCTAssertTrue(sent.isEmpty, "no send before the display's frame")
        grip.mouseUp(with: event(.leftMouseUp, 40, 30))
        XCTAssertEqual(sent.count, 1, "the moves between two frames are one send")
        let last = try XCTUnwrap(sent.first)
        XCTAssertEqual(last.0, 2)
        XCTAssertEqual(try XCTUnwrap(last.1), 340, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(last.2), 110, accuracy: 0.01)
        grip.mouseUp(with: event(.leftMouseUp, 40, 30))
        XCTAssertEqual(sent.count, 1, "nothing is left to send")
    }
}
#endif
