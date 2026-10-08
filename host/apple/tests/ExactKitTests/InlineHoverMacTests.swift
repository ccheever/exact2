#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class InlineHoverMacTests: XCTestCase {
    func testRTLEllipsisHitsTheVisibleActionRunAndNeverTheHiddenRun() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "rtl-action")
        defer { session.destroy() }
        let p = session.presenter
        for (prefix, suffix, expected) in [("Describe the vowel counter (edited) ", "MMMMMMMM", 4),
                                          ("אבגדה ", "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 3)] {
            p.apply(wireBatch([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "create", "id": 2, "kind": "text", "style": ["font_size": 20.0, "direction": "rtl", "white_space": "nowrap", "text_align": "center", "text_overflow": "ellipsis", "overflow_x": "hidden"]],
                ["op": "paragraph", "id": 2, "runs": [
                    ["id": 3, "parent": 2, "paint": true, "props": ["text": prefix], "style": ["font_size": 20.0], "handlers": ["press"]],
                    ["id": 4, "parent": 2, "paint": true, "props": ["text": suffix], "style": ["font_size": 20.0], "handlers": ["press"]],
                ]],
                ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 100],
                ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 100, "h": 30],
            ]))
            let node = try XCTUnwrap(p.views[2]), action = try XCTUnwrap(p.inlineText(UInt32(expected)))
            let rect = try XCTUnwrap(node.inlineRects(action).first)
            XCTAssertEqual(node.inlineTarget(at: CGPoint(x: rect.midX, y: rect.midY), handler: "press")?.id, UInt32(expected))
            if expected == 4 { XCTAssertTrue(node.inlineRects(try XCTUnwrap(p.inlineText(3))).isEmpty) }
            p.apply(wireBatch([["op": "destroy", "id": 2], ["op": "destroy", "id": 1]]))
        }
    }

    func testAgentAndMouseEnterLeaveAndReenterWrappedLink() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "inline-hover")
        defer { session.destroy() }
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 600, height: 400)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        func run(_ id: Int, _ text: String, hover: Bool = false) -> [String: Any] {
            ["id": id, "parent": 2, "paint": true, "props": ["text": text],
             "style": ["font_size": 16.0], "handlers": hover ? ["hover"] : []]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "style": ["font_size": 16.0]],
            ["op": "paragraph", "id": 2, "runs": [run(3, "Before this, please read "),
                run(4, "see the earlier fix in pull request twelve", hover: true), run(5, " after it.")]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Outside"]],
            ["op": "children", "id": 1, "ids": [2, 6]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 600, "h": 400],
            ["op": "frame", "id": 2, "x": 24, "y": 24, "w": 260, "h": 80],
            ["op": "frame", "id": 6, "x": 24, "y": 120, "w": 260, "h": 24],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let node = try XCTUnwrap(p.views[2]), link = try XCTUnwrap(p.inlineText(4))
        let rects = node.inlineRects(link)
        XCTAssertGreaterThanOrEqual(rects.count, 2)
        let first = try XCTUnwrap(rects.first), last = try XCTUnwrap(rects.last)
        var events: [String] = []
        p.onHover = { events.append("\($0):\($1)") }
        let agent = Agent(session: session)
        func tap(_ request: [String: Any]) -> [String: Any] {
            let wire = try! JSONSerialization.data(withJSONObject: request)
            return agent.tap(try! JSONSerialization.jsonObject(with: wire) as! [String: Any])
        }
        let entered = tap(["id": 4, "hover": true])
        XCTAssertNil(entered["error"])
        XCTAssertEqual(entered["tapped"] as? Int, 4)
        let at = try XCTUnwrap(entered["at"] as? [Double])
        let expected = agent.box(node, region: first)
        XCTAssertEqual(at[0], expected.midX, accuracy: 0.01)
        XCTAssertEqual(at[1], expected.midY, accuracy: 0.01)
        XCTAssertNil(tap(["id": 6, "hover": true])["error"])
        let second = agent.box(node, region: last)
        XCTAssertNil(tap(["id": 2, "hover": true, "x": second.midX, "y": second.midY])["error"])
        XCTAssertEqual(events, ["4:true", "4:false", "4:true"])
        p.hoverInline(nil)
        events.removeAll()
        func move(_ rect: CGRect) {
            let point = node.convert(CGPoint(x: rect.midX, y: rect.midY), to: nil)
            let event = NSEvent.mouseEvent(with: .mouseMoved, location: point, modifierFlags: [],
                timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                context: nil, eventNumber: 0, clickCount: 0, pressure: 0)!
            node.mouseMoved(with: event)
        }
        move(first)
        move(try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(3))).first))
        move(last)
        XCTAssertEqual(events, ["4:true", "4:false", "4:true"])
        p.hoverInline(nil)
        events.removeAll()
        // Inline listeners do not evict their paragraph's ancestor hover.
        node.handlers = ["hover"]
        move(first)
        move(last)
        XCTAssertEqual(events, ["2:true", "4:true"])
        XCTAssertNil(tap(["id": 6, "hover": true])["error"])
        XCTAssertEqual(events, ["2:true", "4:true", "4:false", "2:false"])
        events.removeAll()
        move(first)
        events.removeAll()
        var leaves = 0
        p.onHover = { id, over in
            events.append("\(id):\(over)")
            if id == 4, !over {
                leaves += 1
                if leaves < 3 { p.setHoverPath([node]) }
            }
        }
        move(try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(3))).first))
        XCTAssertEqual(events, ["4:false"], "inline leave is admitted before a reentrant boundary callback")
        XCTAssertNil(p.hoveredInline)
        XCTAssertEqual(p.hoveredNodes.map(\.id), [2])
        p.setHoverPath([])
        p.onHover = nil
        p.hoverInline(4)
        events.removeAll()
        p.onHover = { id, over in
            events.append("\(id):\(over)")
            if id == 4, !over { p.setHoverPath([]) }
        }
        p.hoverInline(3)
        XCTAssertEqual(events, ["4:false"], "a superseded inline enter cannot send an unmatched leave")
        XCTAssertNil(p.hoveredInline)
        events.removeAll()
        p.onHover = { events.append("\($0):\($1)") }
        // An overlay over the run is the hit target, even when named by ID.
        let overlay = NodeView(id: 7, kind: "view", presenter: p)
        overlay.frame = node.frame
        node.superview?.addSubview(overlay)
        p.views[7] = overlay
        XCTAssertNil(tap(["id": 4, "hover": true])["error"])
        XCTAssertTrue(events.isEmpty, "an obscured run must not enter")
    }
}
#endif
