#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1057.001 §1 on AppKit's own mouse methods: candidates innermost first,
/// then reorder > transform > height > pan > swipe; the web's `dblclick` order.
final class GesturePrecedenceMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    private func host(_ ops: [[String: Any]], calls: ReorderRecorder? = nil) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        p.reorderCalls = calls
        p.apply(wireBatch(ops))
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        return p
    }
    private func event(_ type: NSEvent.EventType, _ view: NSView, down: CGFloat = 0, right: CGFloat = 0,
                       clicks: Int = 1) -> NSEvent {
        let at = view.convert(NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil)
        return NSEvent.mouseEvent(with: type, location: NSPoint(x: at.x + right, y: at.y - down), modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window!.windowNumber, context: nil,
            eventNumber: clicks, clickCount: clicks, pressure: 1)!
    }

    func testCandidatesAreInnermostFirstThenRanked() {
        // Depths per rank (reorder, transform, height, pan, swipe).
        XCTAssertEqual(MouseChain.ordered([nil, 2, 0, 0, 1]), [2, 3, 4, 1])
        XCTAssertEqual(MouseChain.ordered([1, nil, nil, 1, 0]), [4, 0, 3])
        XCTAssertEqual(MouseChain.ordered([nil, nil, nil, nil, nil]), [])
    }

    /// LLP 1005 §3: the button down on a child reaches the nearest node
    /// hearing `pointerdown`/`pointerup`, down before the press, up when it
    /// lifts, as the web's order has it.
    func testPointerDownAndUpReachTheNearestPointerNodeAroundThePress() {
        let p = host([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press", "pointerdown", "pointerup", "pointermove"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 50.0, "h": 50.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        var samples: [PointerSample] = []
        p.onPointer = { id, kind, sample in
            log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)"); samples.append(sample)
        }
        let child = p.views[2]!
        child.mouseDown(with: event(.leftMouseDown, child))
        XCTAssertEqual(log, ["down 1"], "before any press")
        // LLP 1056 §3 stage 3: the held pointer's drag is the node's move,
        // its point from the node's content box (the child's middle, 35 in).
        child.mouseDragged(with: event(.leftMouseDragged, child, down: 5, right: 10))
        XCTAssertEqual(log, ["down 1", "move 1"])
        XCTAssertEqual(samples.last?.x, 45); XCTAssertEqual(samples.last?.y, 40)
        XCTAssertEqual(samples.last?.type, "mouse"); XCTAssertEqual(samples.last?.id, 1)
        child.mouseUp(with: event(.leftMouseUp, child))
        XCTAssertEqual(log, ["down 1", "move 1", "up 1", "press 1"])
        XCTAssertEqual(samples.last?.buttons, 0)
        XCTAssertNil(p.pointerHeld)
    }

    /// LLP 1056 §3: a free pointer's moves are one a display frame, the
    /// latest, as the web host sends them; a pending one goes before a down
    /// (Grok's batch 2 review). AppKit can report several `mouseMoved` a frame.
    func testFreeMovesAreOneAFrameAndGoBeforeADown() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointermove"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        window!.orderFront(nil)
        var log: [String] = []
        var xs: [Double] = []
        p.onPointer = { id, kind, sample in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)"); xs.append(sample.x) }
        let node = p.views[1]!
        for right in [0.0, 5.0, 10.0] { node.mouseMoved(with: event(.mouseMoved, node, right: right)) }
        XCTAssertEqual(log, [], "nothing until the frame")
        let frame = expectation(description: "a display frame")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { frame.fulfill() }
        wait(for: [frame], timeout: 2)
        XCTAssertEqual(log, ["move 1"], "one move a frame")
        XCTAssertEqual(xs.last, 110, "the latest")
        node.mouseMoved(with: event(.mouseMoved, node, right: 20))
        node.mouseDown(with: event(.leftMouseDown, node))
        XCTAssertEqual(log, ["move 1", "move 1", "down 1"], "the pending move before the down")
        node.mouseUp(with: event(.leftMouseUp, node))
    }

    /// Two `pointermove` nodes crossed in one frame each hear their own
    /// last move (Grok's batch 2 delta review).
    func testEachNodeCrossedInAFrameHearsItsLastMove() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointermove"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["pointermove"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "children", "id": 3, "ids": [1, 2]],
            ["op": "roots", "ids": [3]],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 200.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPointer = { id, _, sample in log.append("move \(id) \(Int(sample.x))") }
        let (a, b) = (p.views[1]!, p.views[2]!)
        a.mouseMoved(with: event(.mouseMoved, a))
        a.mouseMoved(with: event(.mouseMoved, a, right: 5))
        b.mouseMoved(with: event(.mouseMoved, b))
        p.flushHoverMove()
        XCTAssertEqual(log, ["move 1 105", "move 2 100"])
    }

    func testDoubleClickPressesTwiceThenDoubleClicks() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press", "dblclick"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onDblclick = { log.append("dblclick \($0)") }
        let node = p.views[1]!
        for clicks in [1, 2] {
            node.mouseDown(with: event(.leftMouseDown, node, clicks: clicks))
            node.mouseUp(with: event(.leftMouseUp, node, clicks: clicks))
        }
        XCTAssertEqual(log, ["press 1", "press 1", "dblclick 1"], "the web's click, click, dblclick")
    }

    func testADoubleClickBubblesToTheNearestDblclickAfterThePress() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["dblclick"]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 80.0, "h": 40.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onDblclick = { log.append("dblclick \($0)") }
        let button = p.views[2]!
        for clicks in [1, 2] {
            button.mouseDown(with: event(.leftMouseDown, button, clicks: clicks))
            button.mouseUp(with: event(.leftMouseUp, button, clicks: clicks))
        }
        XCTAssertEqual(log, ["press 2", "press 2", "dblclick 1"])
    }

    /// A reorder grip (3) inside a row (2) that pans: the inner reorder wins a
    /// vertical drag; it refuses a horizontal one, which the outer pan takes.
    private func gripInsidePan(_ calls: ReorderRecorder) -> Presenter {
        host([
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50,
                "totalExtent": 2000, "rows": [["view": 2, "root": 3, "epoch": 7]], "correction": NSNull()]]],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["pan"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["reorderFor": "items"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "grip"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 10.0, "y": 10.0, "w": 100.0, "h": 20.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]
        ], calls: calls)
    }

    func testTheInnermostDragBindingBeatsAnAncestorPan() {
        let calls = ReorderRecorder()
        let p = gripInsidePan(calls)
        var pans: [Double] = []
        p.onPan = { _, _, dy in pans.append(dy) }
        let text = p.views[4]!
        text.mouseDown(with: event(.leftMouseDown, text))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 10))
        text.mouseDragged(with: event(.leftMouseDragged, text, down: 30))
        text.mouseUp(with: event(.leftMouseUp, text, down: 30))
        XCTAssertEqual(calls.log.first, "begin 3 top=0.0", "reorder, the inner candidate, takes the vertical drag")
        XCTAssertEqual(pans, [], "the ancestor's pan does not also fire")
    }

    func testARefusedInnerCandidateFallsToTheAncestorPan() {
        let calls = ReorderRecorder()
        let p = gripInsidePan(calls)
        var pans: [Double] = []
        p.onPan = { _, dx, _ in pans.append(dx) }
        let text = p.views[4]!
        text.mouseDown(with: event(.leftMouseDown, text))
        text.mouseDragged(with: event(.leftMouseDragged, text, right: 10))
        text.mouseDragged(with: event(.leftMouseDragged, text, right: 30))
        text.mouseUp(with: event(.leftMouseUp, text, right: 30))
        XCTAssertEqual(calls.log, [], "reorder refuses a horizontal drag")
        XCTAssertEqual(pans, [10, 20])
    }

    /// Rule 4 on AppKit: a node with `pan` and `press` presses when the pan
    /// never leaves the slop, and a pan that begins cancels the press.
    func testAPanNodePressesUnlessThePanBegins() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press", "pan"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onPan = { _, dx, _ in log.append("pan \(dx)") }
        let node = p.views[1]!
        node.mouseDown(with: event(.leftMouseDown, node))
        node.mouseDragged(with: event(.leftMouseDragged, node, right: 2))
        node.mouseUp(with: event(.leftMouseUp, node, right: 2))
        XCTAssertEqual(log, ["press 1"], "inside the slop it is a press")
        log = []
        node.mouseDown(with: event(.leftMouseDown, node))
        node.mouseDragged(with: event(.leftMouseDragged, node, right: 10))
        node.mouseUp(with: event(.leftMouseUp, node, right: 10))
        XCTAssertEqual(log, ["pan 10.0"], "a pan that begins cancels the press")
    }

    /// Rule 3 for a `pan` (kanban F6): a press between keeps the contact only
    /// within the slop — a drag that starts on a card's button pans the card,
    /// and a tap on the button still presses it.
    func testAnAncestorPanTakesADragThatStartsOnAButton() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pan"]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 100.0, "h": 40.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onPan = { id, dx, _ in log.append("pan \(id) \(dx)") }
        let button = p.views[2]!
        button.mouseDown(with: event(.leftMouseDown, button))
        button.mouseDragged(with: event(.leftMouseDragged, button, right: 2))
        button.mouseUp(with: event(.leftMouseUp, button, right: 2))
        XCTAssertEqual(log, ["press 2"], "inside the slop it is the button's press")
        log = []
        button.mouseDown(with: event(.leftMouseDown, button))
        button.mouseDragged(with: event(.leftMouseDragged, button, right: 10))
        button.mouseDragged(with: event(.leftMouseDragged, button, right: 30))
        button.mouseUp(with: event(.leftMouseUp, button, right: 30))
        XCTAssertEqual(log, ["pan 1 10.0", "pan 1 20.0"], "past it the card pans and the press is cancelled")
    }
}
#endif
