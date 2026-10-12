#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// X80: Chrome hit-tests a held mouse as it does a free one. A drag moves
/// the hover across the `hover` nodes it crosses (each leaves and enters with
/// the button down), and the node under the pointer at the release stays
/// hovered, whether or not a node holds the pointer. A pointer a gesture
/// captured (the web's recognizers call `setPointerCapture`) keeps the hover
/// on its node until the button comes up, when the node under it enters.
final class HoverDuringDragMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    /// Rows 2–6 (the repro's 1–5), 40 points tall, in a column (id 1).
    private func rows(_ handlers: [Int: [String]]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        let ids = Array(2...6)
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"]]
            + ids.map { ["op": "create", "id": $0, "kind": "view", "handlers": handlers[$0] ?? ["hover"]] }
            + [["op": "children", "id": 1, "ids": ids], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0]]
            + ids.map { ["op": "frame", "id": $0, "x": 0.0, "y": Double($0 - 2) * 40, "w": 200.0, "h": 40.0] }))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return p
    }
    /// A mouse event at (x, y) from the column's top left.
    private func event(_ type: NSEvent.EventType, _ p: Presenter, x: CGFloat = 100, y: CGFloat) -> NSEvent {
        let down = type != .mouseMoved
        return NSEvent.mouseEvent(with: type, location: p.views[1]!.convert(NSPoint(x: x, y: y), to: nil), modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window!.windowNumber, context: nil,
            eventNumber: 1, clickCount: down ? 1 : 0, pressure: down ? 1 : 0)!
    }
    /// The repro's drive: rest on row 1's middle, press, drag 120 points down
    /// in 4-point steps to row 4's middle; AppKit sends every drag to the
    /// view the button went down on.
    private func drag(_ p: Presenter, release: Bool = true) -> NodeView {
        let a1 = p.views[2]!
        a1.mouseMoved(with: event(.mouseMoved, p, y: 20))
        a1.mouseDown(with: event(.leftMouseDown, p, y: 20))
        for y in stride(from: 24.0, through: 140, by: 4) { a1.mouseDragged(with: event(.leftMouseDragged, p, y: y)) }
        if release { a1.mouseUp(with: event(.leftMouseUp, p, y: 140)) }
        return a1
    }

    func testADragMovesTheHoverAcrossTheRowsAndTheReleaseLeavesItUnderThePointer() {
        let p = rows([:])
        var log: [String] = []
        p.onHover = { log.append("\($0)\($1 ? "+" : "-")") }
        let a1 = drag(p, release: false)
        XCTAssertEqual(log, ["2+", "2-", "3+", "3-", "4+", "4-", "5+"], "held over row 4: the web's order")
        XCTAssertTrue(p.hovered === p.views[5])
        a1.mouseUp(with: event(.leftMouseUp, p, y: 140))
        XCTAssertEqual(log.count, 7, "the release over row 4 changes nothing")
        XCTAssertTrue(p.hovered === p.views[5], "row 4 stays hovered with no further move")
        p.views[5]!.mouseMoved(with: event(.mouseMoved, p, x: 300, y: 140))
        XCTAssertEqual(log.last, "5-", "a free move off the rows leaves it")
    }

    func testAHeldPointerKeepsItsMovesAndUpWhileTheHoverFollows() {
        let p = rows(Dictionary(uniqueKeysWithValues: (2...6).map { ($0, ["hover", "pointerdown", "pointermove", "pointerup"]) }))
        var log: [String] = []
        p.onHover = { log.append("\($0)\($1 ? "+" : "-")") }
        p.onPointer = { id, kind, _ in
            switch kind {
            case .down: log.append("\(id)v")
            case .up: log.append("\(id)^")
            case .move: if log.last != "\(id)m" { log.append("\(id)m") }
            }
        }
        let a1 = drag(p, release: false)
        p.flushHoverMove()
        XCTAssertEqual(p.pointerHeld, 2, "the pressed row holds the pointer")
        XCTAssertEqual(log.filter { !$0.hasSuffix("m") }, ["2+", "2v", "2-", "3+", "3-", "4+", "4-", "5+"])
        XCTAssertEqual(Set(log.filter { $0.hasSuffix("m") }), ["2m"], "every move is the held row's")
        a1.mouseUp(with: event(.leftMouseUp, p, y: 140))
        XCTAssertEqual(log.last, "2^", "the up is the held row's")
        XCTAssertNil(p.pointerHeld)
        XCTAssertTrue(p.hovered === p.views[5])
    }

    func testAGestureThatTookThePointerKeepsTheHoverUntilTheUp() {
        let p = rows([2: ["hover", "pan"]])
        var log: [String] = []
        p.onHover = { log.append("\($0)\($1 ? "+" : "-")") }
        var panned = 0.0
        p.onPan = { _, _, dy in panned += dy }
        _ = drag(p, release: false)
        XCTAssertGreaterThan(panned, 100, "the pan took the drag")
        XCTAssertEqual(log, ["2+"], "captured: the panned row stays hovered")
        p.views[2]!.mouseUp(with: event(.leftMouseUp, p, y: 140))
        XCTAssertEqual(log, ["2+", "2-", "5+"], "the release lets it go: the row under the pointer enters, with no move")
        XCTAssertTrue(p.hovered === p.views[5])
    }
}
#endif
