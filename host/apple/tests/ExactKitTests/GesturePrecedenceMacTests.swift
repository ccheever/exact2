#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactDrag

/// LLP 1057.001 §1 on AppKit's own mouse methods: candidates innermost first,
/// then reorder > transform > height > pan > swipe; the web's `dblclick` order.
final class GesturePrecedenceMacTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactDrag.install() } // LLP 1047.001 D4
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

    /// The down and the up bubble, innermost first (Charlie, 2026-10-15):
    /// a box hearing only `pointermove` holds the drags and takes nothing
    /// from its ancestors' down and up (#20261010, X71). The up still
    /// reaches an ancestor after the held node is destroyed under the hold.
    func testPointerDownAndUpBubbleWhileTheInnermostHoldsTheMoves() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["pointermove"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "create", "id": 4, "kind": "view", "handlers": ["pointerup"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 200.0, "h": 150.0],
            ["op": "frame", "id": 3, "x": 10.0, "y": 10.0, "w": 50.0, "h": 50.0],
            ["op": "frame", "id": 4, "x": 100.0, "y": 10.0, "w": 50.0, "h": 50.0]
        ])
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        let inner = p.views[3]!, moving = p.views[2]!
        inner.mouseDown(with: event(.leftMouseDown, inner))
        XCTAssertEqual(p.pointerHeld, 3)
        inner.mouseDragged(with: event(.leftMouseDragged, inner, right: 10))
        inner.mouseUp(with: event(.leftMouseUp, inner))
        XCTAssertEqual(log, ["down 3", "down 1", "up 3", "up 1"], "the innermost takes no move it does not hear")
        log = []
        moving.mouseDown(with: event(.leftMouseDown, moving, down: 50))
        XCTAssertEqual(p.pointerHeld, 2, "the pointermove box holds the moves")
        moving.mouseDragged(with: event(.leftMouseDragged, moving, down: 50, right: 10))
        moving.mouseUp(with: event(.leftMouseUp, moving, down: 50))
        XCTAssertEqual(log, ["down 1", "move 2", "up 1"])
        log = []
        let up = p.views[4]!
        up.mouseDown(with: event(.leftMouseDown, up))
        p.apply(wireBatch([["op": "destroy", "id": 4], ["op": "children", "id": 2, "ids": [3]]]))
        up.mouseUp(with: event(.leftMouseUp, up))
        XCTAssertEqual(log, ["down 1", "up 1"], "the ancestor's up outlives the destroyed held node")
        XCTAssertNil(p.pointerHeld); XCTAssertTrue(p.pointerPath.isEmpty)
    }

    /// A pointer's offsets are from the content box layout sent: a padding
    /// the style rows cannot resolve (5% of a 400-point containing block) is
    /// 20 points in, as the canvas or paragraph inside it is drawn.
    func testAPointersOffsetsAreFromTheContentBoxLayoutSent() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown"]],
            ["op": "style", "id": 1, "style": ["padding_left": ["pct": 5], "padding_top": 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "fieldContent", "id": 1, "rect": [20.0, 4.0, 180.0, 96.0]]
        ])
        var samples: [PointerSample] = []
        p.onPointer = { _, _, sample in samples.append(sample) }
        let node = p.views[1]!
        node.mouseDown(with: event(.leftMouseDown, node))
        XCTAssertEqual(samples.last?.x, 80); XCTAssertEqual(samples.last?.y, 46)
        node.mouseUp(with: event(.leftMouseUp, node))
    }

    /// Any button holds the pointer, as in a browser (review b5-b 1): the
    /// secondary button's down, moves and up and the middle button's are the
    /// node's pointer events with `buttons` 2 and 4; the secondary's down is
    /// then its `contextmenu` at that point. A disabled box still hears
    /// `wheel` (it means nothing on a div); a disabled button does not
    /// (review b5-b 4).
    func testEveryButtonHoldsThePointerAndADisabledBoxHearsTheWheel() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "pointermove", "contextmenu"]],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["wheel"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["wheel"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["wheel"]],
            ["op": "roots", "ids": [1, 2, 3, 4]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 150.0, "w": 200.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 220.0, "w": 200.0, "h": 50.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 280.0, "w": 200.0, "h": 20.0]
        ])
        p.views[2]!.props["disabled"] = "true"; p.views[3]!.props["disabled"] = "true"
        // A disabled link is the web's `<a>`, which `disabled` does not touch (review b5-delta).
        p.views[4]!.props["disabled"] = "true"; p.views[4]!.props["href"] = "/docs"
        var log: [String] = []
        p.onPointer = { id, kind, sample in
            log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id) \(sample.buttons)")
        }
        p.onClipboard = { id, kind, _ in log.append("event \(kind) \(id)") }
        let node = p.views[1]!
        node.rightMouseDown(with: event(.rightMouseDown, node))
        node.rightMouseDragged(with: event(.rightMouseDragged, node, right: 5))
        node.rightMouseUp(with: event(.rightMouseUp, node))
        XCTAssertEqual(log, ["down 1 2", "event 10 1", "move 1 2", "up 1 0"])
        log = []
        node.otherMouseDown(with: event(.otherMouseDown, node))
        node.otherMouseDragged(with: event(.otherMouseDragged, node, right: 5))
        node.otherMouseUp(with: event(.otherMouseUp, node))
        XCTAssertEqual(log, ["down 1 4", "move 1 4", "up 1 0"])
        XCTAssertNil(p.pointerHeld)
        log = []
        let wheel = { NSEvent(cgEvent: CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: -3, wheel2: 0, wheel3: 0)!)! }
        _ = p.views[3]!.wheel(wheel())
        _ = p.views[2]!.wheel(wheel())
        _ = p.views[4]!.wheel(wheel())
        XCTAssertEqual(log, ["event 37 3", "event 37 4"], "the disabled box and link hear the wheel, the disabled button does not")
    }

    /// The hold is the node's: a batch that removes the child the button
    /// went down on (a board square's piece, fix/syntax6's board drag) leaves
    /// that view in the window, transparent, since AppKit sends the drags and
    /// the up only to it while it is there; it goes when the button comes up.
    func testAHoldOutlivesTheChildItWasPressedOn() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "pointermove"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 50.0, "h": 50.0]
        ])
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        let child = p.views[2]!
        child.mouseDown(with: event(.leftMouseDown, child))
        p.apply(wireBatch([["op": "children", "id": 1, "ids": []], ["op": "destroy", "id": 2]]))
        XCTAssertNil(p.views[2])
        XCTAssertNotNil(child.window, "still where AppKit sends the hold's events")
        XCTAssertEqual(child.alphaValue, 0)
        child.mouseDragged(with: event(.leftMouseDragged, child, right: 10))
        child.mouseUp(with: event(.leftMouseUp, child))
        XCTAssertEqual(log, ["down 1", "move 1", "up 1"])
        XCTAssertNil(p.pointerHeld)
        XCTAssertNil(child.superview, "gone with the button")
    }

    /// b6 review B8: a batch that moves the pressed view to another parent
    /// under the hold moves it, shown; it keeps hearing the hold there.
    func testAHeldViewMovedToAnotherParentGoesThere() {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "pointermove"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1, 3]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 120.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 50.0, "h": 50.0]
        ])
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        let child = p.views[2]!, old = p.views[1]!, new = p.views[3]!
        child.mouseDown(with: event(.leftMouseDown, child))
        p.apply(wireBatch([["op": "children", "id": 1, "ids": []], ["op": "children", "id": 3, "ids": [2]]]))
        XCTAssertTrue(child.superview === new.container, "moved")
        XCTAssertEqual(child.alphaValue, 1, "shown")
        child.mouseDragged(with: event(.leftMouseDragged, child, right: 10))
        child.mouseUp(with: event(.leftMouseUp, child))
        XCTAssertEqual(log, ["down 1", "move 1", "up 1"])
        XCTAssertTrue(child.superview === new.container, "still there after the button")
        XCTAssertFalse(child.isDescendant(of: old))
    }

    /// The T3 Code clone's crash on a click after "Archive thread" or a
    /// thread's Delete confirm (realinput-1011c G4-14, LSF-6): a context
    /// menu took a right-click's up, so the hold lasts, and a batch removes
    /// and frees the row it went down on. The view pressed goes with its
    /// row rather than staying a subview of a freed one, and the next click's
    /// up ends the hold without reaching it.
    func testARowFreedUnderAHoldTakesThePressedViewWithIt() throws {
        var p: Presenter!
        weak var row: NodeView?
        autoreleasepool {
            p = host([
                ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "pointermove"]],
                ["op": "create", "id": 2, "kind": "view"],
                ["op": "create", "id": 3, "kind": "view"],
                ["op": "create", "id": 4, "kind": "button", "handlers": ["press"]],
                ["op": "children", "id": 2, "ids": [3]],
                ["op": "children", "id": 1, "ids": [2, 4]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
                ["op": "frame", "id": 3, "x": 10.0, "y": 10.0, "w": 100.0, "h": 20.0],
                ["op": "frame", "id": 4, "x": 0.0, "y": 250.0, "w": 100.0, "h": 30.0]
            ])
            row = p.views[2]
        }
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        p.onPress = { log.append("press \($0)") }
        let label = try XCTUnwrap(p.views[3])
        autoreleasepool { label.rightMouseDown(with: event(.rightMouseDown, label)) }
        // No up: the context menu's tracking took it. The pressed view is
        // destroyed while its row is in the window, then the row goes.
        autoreleasepool {
            p.apply(wireBatch([["op": "destroy", "id": 3], ["op": "destroy", "id": 2], ["op": "children", "id": 1, "ids": [4]]]))
        }
        XCTAssertNil(row, "the row is freed")
        let settings = try XCTUnwrap(p.views[4])
        settings.mouseDown(with: event(.leftMouseDown, settings))
        settings.mouseUp(with: event(.leftMouseUp, settings))
        XCTAssertNil(label.superview, "the pressed view left with its row")
        // The hold the menu left ends at this up, as before.
        XCTAssertEqual(log, ["down 1", "up 1", "press 4"])
        XCTAssertNil(p.pointerHeld)
        XCTAssertNil(p.pointerSource)
    }

    /// The T3 Code clone's crash on a click on Settings while a panel held
    /// the pointer (realinput-1011c R11-X79, R11-X51): a batch between the
    /// down and the up replaces the button's panel. AppKit sends that up to
    /// no view, the one pressed having left the window, so the hold lasts,
    /// and the next click, on the new button, ends it at its up.
    func testAPanelReplacedUnderAClickTakesThePressedViewWithIt() throws {
        var p: Presenter!
        weak var panel: NodeView?
        autoreleasepool {
            p = host([
                ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "pointermove"]],
                ["op": "create", "id": 2, "kind": "view"],
                ["op": "create", "id": 3, "kind": "button", "handlers": ["press"]],
                ["op": "children", "id": 2, "ids": [3]],
                ["op": "children", "id": 1, "ids": [2]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 200.0, "w": 200.0, "h": 100.0],
                ["op": "frame", "id": 3, "x": 10.0, "y": 10.0, "w": 100.0, "h": 30.0]
            ])
            panel = p.views[2]
        }
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        p.onPress = { log.append("press \($0)") }
        let settings = try XCTUnwrap(p.views[3])
        autoreleasepool { settings.mouseDown(with: event(.leftMouseDown, settings)) }
        // The panel leaves the window first, then its nodes are destroyed.
        autoreleasepool {
            p.apply(wireBatch([
                ["op": "create", "id": 5, "kind": "view"],
                ["op": "create", "id": 6, "kind": "button", "handlers": ["press"]],
                ["op": "children", "id": 5, "ids": [6]],
                ["op": "children", "id": 1, "ids": [5]],
                ["op": "destroy", "id": 3],
                ["op": "destroy", "id": 2],
                ["op": "frame", "id": 5, "x": 0.0, "y": 200.0, "w": 200.0, "h": 100.0],
                ["op": "frame", "id": 6, "x": 10.0, "y": 10.0, "w": 100.0, "h": 30.0]
            ]))
        }
        XCTAssertNil(panel, "the panel is freed")
        // No up for that click: AppKit sent it to no view. The next click
        // is on the new button.
        let again = try XCTUnwrap(p.views[6])
        again.mouseDown(with: event(.leftMouseDown, again))
        again.mouseUp(with: event(.leftMouseUp, again))
        XCTAssertNil(settings.superview, "the pressed view left with its panel")
        // The hold the first up missed ends at this up, as before.
        XCTAssertEqual(log, ["down 1", "up 1", "press 6"])
        XCTAssertNil(p.pointerHeld)
        XCTAssertNil(p.pointerSource)
    }

    /// LLP 1005: a held node's release clears the hold. The view the button
    /// went down on goes with the button even so, and a resting pointer's
    /// hover follows the layout again, as it does after any up.
    func testTheButtonLetsThePressedViewGoAfterTheHeldNodeLeft() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "pointermove"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["hover"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1, 3]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 50.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 150.0, "w": 200.0, "h": 100.0]
        ])
        var log: [String] = []
        p.onPointer = { id, kind, _ in log.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
        let child = try XCTUnwrap(p.views[2]), other = try XCTUnwrap(p.views[3])
        child.mouseDown(with: event(.leftMouseDown, child))
        p.apply(wireBatch([["op": "roots", "ids": [3]], ["op": "destroy", "id": 2], ["op": "destroy", "id": 1]]))
        XCTAssertNil(p.pointerHeld, "the held node's release cleared the hold")
        other.mouseUp(with: event(.leftMouseUp, other))
        XCTAssertNil(p.pointerSource, "the button let the pressed view go")
        p.agentPointer = other.convert(NSPoint(x: other.bounds.midX, y: other.bounds.midY), to: nil)
        XCTAssertNotNil(p.restingPointer(), "a resting pointer's hover follows again")
        XCTAssertEqual(log, ["down 1"])
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
        p.flushHoverMove() // the driver's display frame, independent of a window server tick
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

    /// A presenter released with a move pending leaves no display link
    /// running (Grok's batch 2 delta review).
    func testAReleasedPresenterStopsItsHoverLink() {
        weak var link: CADisplayLink?
        autoreleasepool {
            var p: Presenter? = Presenter()
            p!.viewport.frame = NSRect(x: 0, y: 0, width: 100, height: 100)
            p!.hoverMoved(1, PointerSample(x: 0, y: 0, buttons: 0, pressure: 0, type: "mouse", id: 1, clientX: 0, clientY: 0))
            link = p!.hoverLink
            XCTAssertNotNil(link)
            p = nil
        }
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))
        XCTAssertNil(link, "invalidated, the run loop lets it go")
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

    /// The same through AppKit's own button, whose tracking loop takes the
    /// mouse: its second click double-clicks from its node up after its press.
    func testADoubleClickOnANativeButtonBubblesAfterItsPress() throws {
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["dblclick"]],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"],
             "handlers": ["press"], "style": ["appearance": "auto"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 10.0, "w": 80.0, "h": 40.0]
        ])
        var log: [String] = []
        p.onPress = { log.append("press \($0)") }
        p.onDblclick = { log.append("dblclick \($0)") }
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        for clicks in [1, 2] {
            // The up is queued first, for the button's tracking loop to take.
            NSApp.postEvent(event(.leftMouseUp, button, clicks: clicks), atStart: true)
            button.mouseDown(with: event(.leftMouseDown, button, clicks: clicks))
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
