// Where a wheel event goes.
//
// This target exists for one class of defect: a host routing decision that
// no other check in this repository can see. The six checks compile and run
// Rust; the agent drives the app through the eight operations and its wheel
// carries a delta and no gesture phase; a screenshot shows a stretched rubber
// band and a scrolled pane as the same picture. So a lift dropped before
// AppKit could see it — the zero-delta `.ended` that ends every trackpad
// scroll — left a band stretched forever, passed every check, drove
// perfectly under the agent, and was found by a person scrolling.
//
// `ChainingScrollView.routing` is a pure function of the event and the
// geometry, so these are assertions about a decision: no window, no run
// loop, no animation, no clock. Nothing here can flake on timing.
//
// @ref LLP 1033 D4a, LLP 1010 (nested chaining), `rules/RULES.md` §Loop shape
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class ScrollRoutingTests: XCTestCase {
    func testEmptyScrollPortDeliversTheHeldPointerAndPressThroughTheWindow() throws {
        _ = NSApplication.shared
        clicksArriveAsInTheKeyWindow()
        for (kind, axis, width, height) in [("view", "y", 300, 60), ("view", "x", 60, 240), ("view", "y", 300, 240), ("view", "y", 300, 480), ("list", "y", 0, 0)] {
            let session = ExactApp.shared.makeSession(label: "empty-scroll")
            defer { session.destroy() }
            let p = session.presenter
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = p.viewport
            defer { window.close() }
            p.apply(wireBatch([
                ["op": "collections", "items": kind == "list" ? [["view": 2, "revision": 1, "scrollSequence": 0, "count": 0, "totalExtent": 0, "rows": []]] : []],
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "create", "id": 2, "kind": kind, "style": ["overflow_" + axis: "scroll"]],
                ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 240],
                ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 240],
                ["op": "content", "id": 2, "w": width, "h": height]]))
            let outer = try XCTUnwrap(p.views[1]), port = try XCTUnwrap(p.views[2])
            if kind == "list" { XCTAssertTrue(p.collections.owns(2)) }
            window.orderFrontRegardless()
            var heard: [String] = []
            p.onPointer = { id, kind, _ in heard.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
            p.onPress = { heard.append("press \($0)") }
            for target in [outer, port] {
                outer.handlers = []; port.handlers = []
                target.handlers = ["pointerdown", "pointermove", "pointerup", "press"]
                heard = []
                for (type, y) in [(NSEvent.EventType.leftMouseDown, 150.0), (.leftMouseDragged, 151.0), (.leftMouseUp, 151.0)] {
                    let point = port.convert(NSPoint(x: 100, y: y), to: nil)
                    window.sendEvent(try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                        windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)))
                }
                XCTAssertEqual(heard, ["down \(target.id)", "move \(target.id)", "up \(target.id)", "press \(target.id)"], "\(kind) \(axis) \(width)x\(height)")
                XCTAssertNil(p.pointerHeld)
            }
            if height > 240 {
                let hit = try XCTUnwrap(port.hitTest(port.superview!.convert(NSPoint(x: 100, y: 150), from: port)))
                let wheel = try XCTUnwrap(NSEvent(cgEvent: try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: -1, wheel2: 0, wheel3: 0))))
                atOnce { hit.scrollWheel(with: wheel) }
                XCTAssertEqual(port.scroll?.contentView.bounds.minY, 10, "the same empty area still routes the wheel through its scroller")
            }
        }
    }

    func testEmptyScrollPortKeepsTheMouseTargetWhenPointerDownHidesOverflow() throws {
        _ = NSApplication.shared
        clicksArriveAsInTheKeyWindow()
        let session = ExactApp.shared.makeSession(label: "removed-scroll")
        defer { session.destroy() }
        let p = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointermove", "pointerup"], "style": ["overflow_y": "scroll"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 240],
            ["op": "content", "id": 1, "w": 300, "h": 60]]))
        let port = try XCTUnwrap(p.views[1])
        weak var removed = port.scroll
        window.orderFrontRegardless()
        var heard: [String] = []
        p.onPointer = { _, kind, _ in
            heard.append(kind == .down ? "down" : kind == .up ? "up" : "move")
            if kind == .down { port.applyStyle(["overflow_x": .string("hidden"), "overflow_y": .string("hidden")]) }
        }
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseDragged, .leftMouseUp] {
            let point = port.convert(NSPoint(x: 100, y: 150), to: nil)
            window.sendEvent(try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1)))
        }
        XCTAssertNil(port.scroll)
        XCTAssertEqual(heard, ["down", "move", "up"])
        XCTAssertNil(p.pointerHeld)
        XCTAssertNil(p.pointerSource)
        XCTAssertNil(removed?.superview, "the obsolete mouse target leaves after release")
    }

    /// The T3 Code clone's abort on a thread change under a press on its
    /// timeline's empty area (X87): a batch replaces the panel the scroll
    /// node is in, and the node is freed. AppKit asks the scroll view to
    /// leave as it frees the node (`-[NSView _finalize]`); one that stayed
    /// would reach its own dealloc with a superview, which AppKit raises.
    /// With no node hearing the pointer the node is freed in the batch;
    /// with an ancestor that does, at the up that ends the hold. AppKit's
    /// calls are made directly, as `GesturePrecedenceMacTests` makes them:
    /// a test's window ordered in keeps the removed views past the batch.
    func testAnEmptyScrollPortFreedUnderAPressGoesWithItsNode() throws {
        _ = NSApplication.shared
        clicksArriveAsInTheKeyWindow()
        for hears in [false, true] {
            let session = ExactApp.shared.makeSession(label: "freed-scroll")
            defer { session.destroy() }
            let p = session.presenter
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = p.viewport
            defer { window.close() }
            weak var node: NodeView?
            weak var scroll: ChainingScrollView?
            var point = NSPoint.zero
            autoreleasepool {
                p.apply(wireBatch([
                    ["op": "create", "id": 1, "kind": "view", "handlers": hears ? ["pointerdown", "pointerup"] : []],
                    ["op": "create", "id": 2, "kind": "view"],
                    ["op": "create", "id": 3, "kind": "view", "style": ["overflow_y": "scroll"]],
                    ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
                    ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 240],
                    ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 240],
                    ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 300, "h": 240],
                    ["op": "content", "id": 3, "w": 300, "h": 60]]))
                node = p.views[3]
                scroll = node?.scroll
                // The empty port, below its 60 points of content.
                point = node?.convert(NSPoint(x: 100, y: 150), to: nil) ?? .zero
            }
            var heard: [String] = []
            p.onPointer = { id, kind, _ in heard.append("\(kind == .down ? "down" : kind == .up ? "up" : "move") \(id)") }
            p.onPress = { heard.append("press \($0)") }
            func event(_ type: NSEvent.EventType) throws -> NSEvent {
                try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                    windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1))
            }
            func hit() throws -> NSView { try XCTUnwrap(p.viewport.hitTest(try XCTUnwrap(p.viewport.superview).convert(point, from: nil))) }
            try autoreleasepool {
                let pressed = try hit()
                XCTAssertTrue(pressed === scroll, "the empty port's hit is its scroll view")
                pressed.mouseDown(with: try event(.leftMouseDown))
            }
            // The batch a key starts while the button is held (thread.jump):
            // another panel, and the scroll node's destroyed.
            autoreleasepool {
                p.apply(wireBatch([
                    ["op": "create", "id": 4, "kind": "view"],
                    ["op": "create", "id": 5, "kind": "button", "handlers": ["press"]],
                    ["op": "children", "id": 4, "ids": [5]], ["op": "children", "id": 1, "ids": [4]],
                    ["op": "destroy", "id": 3], ["op": "destroy", "id": 2],
                    ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 300, "h": 240],
                    ["op": "frame", "id": 5, "x": 0, "y": 0, "w": 300, "h": 240]]))
            }
            XCTAssertEqual(node == nil, !hears, "only a hold keeps the node the button went down on")
            XCTAssertEqual(scroll == nil, !hears)
            // No up for that press: AppKit sends it to no view, the one
            // pressed having left the window. The next click is on the new
            // panel, and its up ends a hold (#519).
            try autoreleasepool {
                let button = try hit()
                XCTAssertTrue(button === p.views[5])
                button.mouseDown(with: try event(.leftMouseDown))
                button.mouseUp(with: try event(.leftMouseUp))
            }
            XCTAssertNil(node, "the scroll node is freed")
            XCTAssertNil(scroll, "its scroll view went with it")
            XCTAssertEqual(heard, hears ? ["down 1", "up 1", "press 5"] : ["press 5"])
            XCTAssertNil(p.pointerHeld)
            XCTAssertNil(p.pointerSource)
        }
    }

    /// A person's wheel, which AppKit scrolls; `atOnce` is the agent's.
    override func setUp() { ChainingScrollView.appliesAtOnce = false }
    override func tearDown() { ChainingScrollView.appliesAtOnce = ExactEnv.agentMode }
    private func atOnce<T>(_ body: () -> T) -> T {
        ChainingScrollView.appliesAtOnce = true
        defer { ChainingScrollView.appliesAtOnce = false }
        return body()
    }

    /// A scroller with both axes able to move, and 400 points of room on each.
    private func scroller(containX: Bool = false, containY: Bool = false) -> ChainingScrollView {
        let sv = ChainingScrollView(frame: NSRect(x: 0, y: 0, width: 300, height: 300))
        sv.scrollsX = true
        sv.scrollsY = true
        sv.containX = containX
        sv.containY = containY
        return sv
    }

    /// 400 points of travel on each axis, sitting at `origin`.
    private func extent(at origin: NSPoint = .zero) -> ChainingScrollView.Extent {
        ChainingScrollView.Extent(maxX: 400, maxY: 400, origin: origin)
    }

    /// The regression this file exists for: the lift that ends a trackpad
    /// gesture carries no delta, and a contained scroller must still see it
    /// or AppKit never learns the gesture is over and holds the stretch.
    func testAZeroDeltaLiftReachesAppKitOnAContainedAxis() {
        let sv = scroller(containY: true)
        XCTAssertEqual(sv.routing(dx: 0, dy: 0, phased: true, in: extent(at: NSPoint(x: 0, y: 400))), .appKit)
        // Even sitting past the end, which is where a stretched band is.
        XCTAssertEqual(sv.routing(dx: 0, dy: 0, phased: true, in: extent(at: NSPoint(x: 0, y: -60))), .appKit)
    }

    /// Before a gesture is any scroller's, a delta-less event is not begun at
    /// an `auto` scroller, which may yet pass the gesture to the one around
    /// it (it gets the lift once the gesture is its own: the latch's test);
    /// and it must not become a chained event either.
    func testAZeroDeltaEventIsDroppedWhenNoAxisIsContained() {
        XCTAssertEqual(scroller().routing(dx: 0, dy: 0, phased: true, in: extent()), .drop)
        // And a delta-less event with no phase at all is nobody's.
        XCTAssertEqual(scroller(containY: true).routing(dx: 0, dy: 0, phased: false, in: extent()), .drop)
    }

    /// A contained axis hands a *gesture* to AppKit — its scrolling, its
    /// momentum, and its rubber band — rather than clamping by hand.
    func testAContainedAxisWithRoomGivesAGestureToAppKit() {
        let sv = scroller(containY: true)
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: true, in: extent()), .appKit)
        // At the end of its travel too: that is when the band forms.
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: true, in: extent(at: NSPoint(x: 0, y: 400))), .appKit)
    }

    /// A wheel with no phases is a mouse wheel: AppKit scrolls it, by its own
    /// step, as it scrolls the page (LLP 1115.000 D8.2; it was clamped here
    /// by hand, 40 points a notch). Only the agent's is applied here, at
    /// once, which is what a drive can read back without racing AppKit.
    func testAPhaselessWheelOnAContainedAxisIsAppKitsAndStopsAtItsEnd() {
        let sv = scroller(containY: true)
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent()), .appKit)
        XCTAssertEqual(atOnce { sv.routing(dx: 0, dy: -50, phased: false, in: extent()) }, .here)
        // Still contained: at its end it stops rather than moving the page.
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent(at: NSPoint(x: 0, y: 400))), .drop)
    }

    /// `contain` forbids passing the rest of a gesture to an ancestor, so a
    /// contained axis with nothing to scroll stops the gesture rather than
    /// chaining it.
    func testAContainedAxisWithNoRoomDropsRatherThanChains() {
        let sv = scroller(containY: true)
        let none = ChainingScrollView.Extent(maxX: 0, maxY: 0, origin: .zero)
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: true, in: none), .drop)
    }

    /// LLP 1115.000 D8.2: an `auto` axis with room is scrolled by AppKit, a
    /// trackpad's gesture and a mouse's wheel alike (it was scrolled here by
    /// hand, so it had no rubber band and a wheel step of its own). What
    /// begins at its end still goes to the scroller around it, which
    /// Caltrain's fixture depends on. Under the agent it is applied at once.
    func testAnAutoAxisIsScrolledByAppKitAndChainsOnlyFromItsEnd() {
        let sv = scroller()
        for phased in [false, true] {
            XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: phased, in: extent()), .appKit, "phased \(phased)")
            XCTAssertEqual(atOnce { sv.routing(dx: 0, dy: -50, phased: phased, in: extent()) }, .here, "the agent's, phased \(phased)")
            // Scrolled to the bottom, a further downward delta belongs to the page.
            XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: phased, in: extent(at: NSPoint(x: 0, y: 400))), .chain)
            // And at the top, an upward one does.
            XCTAssertEqual(sv.routing(dx: 0, dy: 50, phased: phased, in: extent()), .chain)
        }
    }

    /// An axis that does not scroll at all chains, whatever the room.
    func testAnAxisThatDoesNotScrollChains() {
        let sv = scroller()
        sv.scrollsY = false
        XCTAssertEqual(sv.routing(dx: 0, dy: -50, phased: false, in: extent()), .chain)
    }

    /// A gesture is one thing: the larger delta decides which axis's rules
    /// apply, so a sideways gesture over a vertically-contained pane still
    /// chains.
    func testTheDominantAxisDecides() {
        let vertical = scroller(containY: true)
        let end = extent(at: NSPoint(x: 400, y: 400))
        XCTAssertEqual(vertical.routing(dx: -80, dy: -10, phased: true, in: end), .chain, "sideways, at that axis's end: the page's")
        XCTAssertEqual(vertical.routing(dx: -10, dy: -80, phased: true, in: end), .appKit, "downward, contained: kept, and AppKit's band")

        let horizontal = scroller(containX: true)
        XCTAssertEqual(horizontal.routing(dx: -80, dy: -10, phased: true, in: end), .appKit)
        XCTAssertEqual(horizontal.routing(dx: -10, dy: -80, phased: true, in: end), .chain)
    }

    /// An equal-magnitude gesture is vertical, so a pane contained only on y
    /// keeps it. Stated because "dominant" needs a tie-break and this is it.
    func testATieIsVertical() {
        let end = extent(at: NSPoint(x: 400, y: 400))
        XCTAssertEqual(scroller(containY: true).routing(dx: -50, dy: -50, phased: true, in: end), .appKit)
        XCTAssertEqual(scroller(containX: true).routing(dx: -50, dy: -50, phased: true, in: end), .chain)
    }

    /// A phase-less tick is not split (LLP 1070 G2, Chrome's rule, its
    /// §2): over a strip that scrolls only sideways, a mostly-vertical
    /// diagonal still scrolls the strip by its x and drops its y, and a
    /// purely vertical tick chains to the feed.
    func testAPhaselessTickGoesToTheDeepestScrollerThatCanTakeAnyOfIt() {
        let strip = scroller()
        strip.scrollsY = false
        let sideways = ChainingScrollView.Extent(maxX: 400, maxY: 0, origin: .zero)
        XCTAssertEqual(strip.routing(dx: -60, dy: -100, phased: false, in: sideways), .appKit)
        XCTAssertEqual(atOnce { strip.routing(dx: -60, dy: -100, phased: false, in: sideways) }, .here)
        XCTAssertEqual(strip.routing(dx: 0, dy: -120, phased: false, in: sideways), .chain)
        // At its end the strip takes no more x: the tick chains.
        let end = ChainingScrollView.Extent(maxX: 400, maxY: 0, origin: NSPoint(x: 400, y: 0))
        XCTAssertEqual(strip.routing(dx: -60, dy: -100, phased: false, in: end), .chain)
        // A gesture still goes by its dominant axis (latching is LLP 1070 G1).
        XCTAssertEqual(strip.routing(dx: -60, dy: -100, phased: true, in: sideways), .chain)
    }

    /// LLP 1115.000 D8.2: a gesture AppKit runs is AppKit's whole. Once an
    /// `auto` scroller owns it, every event of it goes to AppKit: past the
    /// scroller's end, where AppKit rubber-bands and does not hand the rest
    /// to the page, and the zero-delta lift and momentum's end, which its
    /// elastic machine needs or the band stays stretched.
    func testAGestureAppKitRunsReachesItWholeToItsLift() {
        let outer = scroller(), inner = scroller()
        outer.addSubview(inner)
        var latch = ChainingScrollView.Latch()
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: true, over: false, moving: false, routing: .drop), .drop, "nobody's yet")
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .appKit), .appKit)
        // At its end its routing would chain; the gesture is AppKit's, which holds it there.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .chain), .appKit)
        // The lift, and momentum's end, which frees the latch.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: false, routing: .drop), .appKit)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: true, moving: false, routing: .drop), .appKit)
        XCTAssertFalse(latch.active)
        // A gesture that begins at the inner list's end is the page's, and AppKit's there.
        _ = ChainingScrollView.latched(&latch, view: inner, began: true, over: false, moving: false, routing: .drop)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .chain), .chain)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: outer, began: false, over: false, moving: true, routing: .appKit), .appKit)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .appKit), .chain, "the inner list passes the rest up")
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: outer, began: false, over: false, moving: false, routing: .drop), .appKit, "and the page gets the lift")
    }

    /// A trackpad gesture latches (LLP 1070 G1, Chrome's rule): the first
    /// scroller that takes a moving event owns the rest of the gesture, its
    /// zero-delta lift and momentum included; an inner list that reaches
    /// its end mid-gesture does not hand the rest to the page. Here scrolled
    /// by hand, as the agent's gesture is.
    func testAGestureLatchesToItsFirstOwnerUntilItEnds() {
        let outer = scroller(), inner = scroller()
        outer.addSubview(inner)
        var latch = ChainingScrollView.Latch()
        // Began, zero-delta: nobody owns it yet.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: true, over: false, moving: false, routing: .drop), .drop)
        // The inner list takes the first moving event: it owns the gesture.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .here), .here)
        // At its end it would chain; latched, the rest is dropped.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .chain), .drop)
        // The lift and the momentum still reach it; the momentum's end frees it.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: false, routing: .appKit), .appKit)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: true, moving: true, routing: .here), .here)
        XCTAssertFalse(latch.active)
        // A gesture that starts at the inner list's end goes to the page: the
        // inner list chains, and the outer one owns it.
        _ = ChainingScrollView.latched(&latch, view: inner, began: true, over: false, moving: false, routing: .drop)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .chain), .chain)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: outer, began: false, over: false, moving: true, routing: .here), .here)
        // Later events arrive at the inner list first: it passes them up
        // even where it could now scroll.
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: inner, began: false, over: false, moving: true, routing: .here), .chain)
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: outer, began: false, over: false, moving: true, routing: .here), .here)
    }

    /// An owner destroyed mid-gesture takes the rest of the gesture with it.
    func testAGestureWhoseOwnerIsGoneIsDropped() {
        var latch = ChainingScrollView.Latch()
        let page = scroller()
        do {
            let gone = scroller()
            _ = ChainingScrollView.latched(&latch, view: gone, began: true, over: false, moving: true, routing: .here)
        }
        XCTAssertEqual(ChainingScrollView.latched(&latch, view: page, began: false, over: false, moving: true, routing: .here), .drop)
    }

    /// The readers' panes, as they are declared: `overflow-x: hidden` with
    /// `overscroll-behavior: contain` (LLP 1033 D4). A sideways gesture must
    /// not escape to the page, and must not scroll the pane either.
    func testAReaderPaneNeitherScrollsSidewaysNorLeaks() {
        let sv = scroller(containX: true, containY: true)
        sv.scrollsX = false
        let onlyVertical = ChainingScrollView.Extent(maxX: 0, maxY: 400, origin: .zero)
        XCTAssertEqual(sv.routing(dx: -80, dy: 0, phased: true, in: onlyVertical), .drop)
        XCTAssertEqual(sv.routing(dx: 0, dy: -80, phased: true, in: onlyVertical), .appKit)
        XCTAssertEqual(sv.routing(dx: 0, dy: -80, phased: false, in: onlyVertical), .appKit)
    }

    /// LLP 1115.000 D8.2: a mouse wheel's notch is AppKit's to apply, by its
    /// own line (`verticalLineScroll`, 10 points unless set; the hand-rolled
    /// tick was 40). AppKit applies it on its own frames, some tens of
    /// milliseconds on, which a test off screen cannot wait on steadily; the
    /// agent's is applied here at once, by the same line.
    func testAMouseWheelsNotchIsAppKitsLine() throws {
        func pane() -> (NSWindow, ChainingScrollView) {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            let sv = scroller()
            sv.scrollsX = false
            sv.documentView = FlippedView(frame: NSRect(x: 0, y: 0, width: 300, height: 3000))
            sv.contentView.postsBoundsChangedNotifications = true
            window.contentView = sv
            return (window, sv)
        }
        func notch() throws -> NSEvent {
            try XCTUnwrap(NSEvent(cgEvent: try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: -1, wheel2: 0, wheel3: 0))))
        }
        let (window, sv) = pane()
        defer { window.close() }
        XCTAssertEqual(sv.verticalLineScroll, 10, "AppKit's line, which nothing here sets")
        let notched = try notch()
        XCTAssertFalse(notched.hasPreciseScrollingDeltas)
        XCTAssertEqual(sv.routing(dx: 0, dy: notched.deltaY * sv.verticalLineScroll, phased: false, in: sv.extent), .appKit, "a person's notch is AppKit's to apply")
        atOnce { sv.scrollWheel(with: notched) }
        XCTAssertEqual(sv.contentView.bounds.minY, 10, "the agent's is applied at once, by AppKit's line")
    }
}
#endif
