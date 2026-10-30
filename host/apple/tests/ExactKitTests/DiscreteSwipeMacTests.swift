#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1057.001 §8 on AppKit: a node's swipe is AppKit's swipe event, in the
/// directions it names, the fingers' way, while `SwipeInput.allows` admits it
/// and it names no `touch-count`. The event goes up the responder chain to the
/// innermost such node, before history.
final class DiscreteSwipeMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close() }

    private func host(_ ops: [[String: Any]]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch(ops))
        return p
    }

    func testANodeTakesTheSwipesItNamesUnlessItIsDisabledInertOrGone() throws {
        XCTAssertEqual(NodeView.swipeWord(deltaX: 1, deltaY: 0), "left")
        XCTAssertEqual(NodeView.swipeWord(deltaX: -1, deltaY: 0), "right")
        XCTAssertEqual(NodeView.swipeWord(deltaX: 0, deltaY: 1), "up")
        XCTAssertEqual(NodeView.swipeWord(deltaX: 0, deltaY: -1), "down")
        XCTAssertNil(NodeView.swipeWord(deltaX: 0, deltaY: 0))
        let p = host([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "left down"]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "left", "touchCount": "2"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "right"]],
            ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
        ])
        let outer = try XCTUnwrap(p.views[1]), counted = try XCTUnwrap(p.views[2]), inner = try XCTUnwrap(p.views[3])
        XCTAssertTrue(outer.takesSwipe("left"))
        XCTAssertTrue(outer.takesSwipe("down"))
        XCTAssertFalse(outer.takesSwipe("right"))
        XCTAssertFalse(counted.takesSwipe("left"), "AppKit reports no count: a node that names one hears no swipe")
        XCTAssertTrue(inner.takesSwipe("right"))
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["disabled": "true"]]]))
        XCTAssertFalse(inner.takesSwipe("right"), "inside a disabled node (`SwipeInput.allows`, which a pan and the drags ask too)")
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["disabled"]], ["op": "props", "id": 3, "set": ["inert": "true"]]]))
        XCTAssertFalse(inner.takesSwipe("right"), "inert")
        p.apply(wireBatch([["op": "props", "id": 3, "clear": ["inert"]]]))
        XCTAssertTrue(inner.takesSwipe("right"))
        p.apply(wireBatch([["op": "children", "id": 2, "ids": []], ["op": "destroy", "id": 3]]))
        p.viewport.addSubview(inner)
        XCTAssertFalse(inner.takesSwipe("right"), "a view still in the window that is no longer its node's, as through an exit transition")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"]]]))
        XCTAssertFalse(outer.takesSwipe("left"))
    }

    /// The swipe event goes up the responder chain from the view under the
    /// pointer, through a scroll view that can scroll either way and a
    /// textarea's text view, to the innermost node that names its direction
    /// and is not disabled; one no node names goes on up.
    func testASwipeEventGoesUpTheChainToTheInnermostNodeThatNamesItsDirection() throws {
        let session = ExactApp.shared.makeSession(label: "discrete-swipe-event")
        defer { session.destroy() }
        let pad = Trackpad(session)
        defer { pad.close() }
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "left right up down"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "textarea", "props": ["value": "Hello there"], "handlers": ["input"]],
            ["op": "create", "id": 5, "kind": "view", "handlers": ["swipe"], "props": ["swipeDirection": "left"]],
            ["op": "children", "id": 3, "ids": [4, 5]], ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 800.0, "h": 200.0],
            ["op": "content", "id": 2, "w": 800.0, "h": 200.0],
            ["op": "frame", "id": 4, "x": 420.0, "y": 20.0, "w": 150.0, "h": 80.0],
            ["op": "frame", "id": 5, "x": 220.0, "y": 20.0, "w": 100.0, "h": 60.0],
        ]))
        var sent: [String] = []
        p.onSwipe = { sent.append("\($0) \($1)") }
        let scroll = try XCTUnwrap(p.views[2]?.scroll)
        // Scrolled into its middle, showing x 200 to 600 of its content, it can scroll either way
        scroll.contentView.scroll(to: NSPoint(x: 200, y: 0))
        scroll.reflectScrolledClipView(scroll.contentView)
        let content = try XCTUnwrap(p.views[3]), inner = try XCTUnwrap(p.views[5]), text = try XCTUnwrap(p.views[4]?.textArea)
        let plain = content.convert(NSPoint(x: 300, y: 150), to: nil), onInner = inner.convert(NSPoint(x: 50, y: 30), to: nil)
        let onText = text.convert(NSPoint(x: 20, y: 10), to: nil)
        XCTAssertTrue(pad.hit(at: onText) === text, "the event starts at the text view")
        XCTAssertTrue(pad.hit(at: onInner).map { $0.isDescendant(of: inner) } ?? false)
        // IOHID's direction masks: 1 up, 2 down, 4 left, 8 right
        for mask: Int64 in [8, 4, 1, 2] { try pad.swipe(mask, at: plain) }
        XCTAssertEqual(sent, ["1 right", "1 left", "1 up", "1 down"], "the scroll view, able to scroll either way, does not stand in the way")
        sent = []
        try pad.swipe(4, at: onInner)
        try pad.swipe(8, at: onInner)
        try pad.swipe(8, at: onText)
        XCTAssertEqual(sent, ["5 left", "1 right", "1 right"], "the innermost that names the direction")
        sent = []
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["disabled": "true"]]]))
        try pad.swipe(4, at: onInner)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["swipeDirection": "up"]]]))
        try pad.swipe(8, at: plain)
        XCTAssertEqual(sent, ["1 left"], "a disabled node passes it on; a direction no node names goes on up")
    }

    /// A node's swipe comes before history (LLP 1057.001 §8.2): on the node,
    /// history does not step; beside it the same swipe is history's, `deltaX`
    /// 1 (a swipe left) stepping Back and -1 Forward, as WebKit's web view
    /// does. `navigationHistory` withholds history's steps only.
    func testANodesSwipeComesBeforeHistory() throws {
        let source = """
        routes nav
          home "/"
            visit "/v/:n"

        component App
          state ways = "back forward"
          state seen = ""
          action next
            nav = push(nav, path("visit", 1))
          action follow(location: string)
            nav = go(nav, location)
          action withhold
            ways = "none"
          action swiped(e: SwipeEvent)
            seen = `${seen}swipe ${e.direction};`
          view
            main navigationKey="app" navigationBack="" navigate=follow navigationHistory=ways width="100%" height="100%"
              text `${top(nav).url}` testId="url"
              text seen testId="seen"
              column testId="card" swipe=swiped swipe-direction="left right" height=100
                text "Card"
              button press=next testId="next"
                text "Next"
              button press=withhold testId="withhold"
                text "Withhold"

        """
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try TableFixture.plan(source), size: CGSize(width: 400, height: 300)).error)
        let pad = Trackpad(session)
        defer { pad.close() }
        let p = session.presenter
        let node = { (id: String) in try XCTUnwrap(p.views.values.first { $0.props["testId"] == id }) }
        let text = { (id: String) in try node(id).props["text"] ?? "" }
        let center = { (v: NSView) in v.convert(NSPoint(x: v.bounds.midX, y: v.bounds.midY), to: nil) }
        let card = try center(node("card")), beside = try center(node("url"))
        try p.press(node("next").id)
        try pad.swipe(4, at: card)
        XCTAssertEqual(try text("seen"), "swipe left;", "on the card, its swipe and not history's Back")
        XCTAssertEqual(try text("url"), "/v/1")
        try pad.swipe(4, at: beside)
        XCTAssertEqual(try text("url"), "/", "beside it, history's Back")
        try pad.swipe(8, at: card)
        XCTAssertEqual(try text("seen"), "swipe left;swipe right;", "on the card, its swipe and not history's Forward")
        XCTAssertEqual(try text("url"), "/")
        try pad.swipe(8, at: beside)
        XCTAssertEqual(try text("url"), "/v/1", "beside it, history's Forward")
        try p.press(node("withhold").id)
        try pad.swipe(4, at: beside)
        XCTAssertEqual(try text("url"), "/v/1", "a way withheld does not step")
        try pad.swipe(4, at: card)
        XCTAssertEqual(try text("seen"), "swipe left;swipe right;swipe left;", "the node's swipe is still heard")
    }

    /// A vertical list on the Mac is a table, whose row that presses AppKit
    /// finds as the table (`TableRowCell.hitTest`): the table hands a swipe
    /// event down to what is under the pointer, so the row hears it.
    func testARowThatPressesInATableHearsASwipeEvent() throws {
        let source = """
        component Rows
          state rows = [0, 1, 2, 3]
          state seen = ""
          action open(r: number)
            seen = `${seen}open ${r};`
          action swiped(r: number)
            seen = `${seen}swipe ${r};`
          view
            column width="100%" height="100%"
              text seen testId="seen"
              list virtualized=true estimated-item-height=44 flex=1 min-height=0 width="100%" testId="rows"
                for r in rows key=`k${r}`
                  row press=open(r) swipe=swiped(r) swipe-direction="left right" testId=`row-${r}` height=44 width="100%" align-items="center"
                    text `Row ${r}`

        """
        let fixture = try TableFixture(count: 4, source: source)
        defer { fixture.close() }
        let pad = Trackpad(window: fixture.window, root: fixture.view)
        let row = try fixture.node("row-2")
        let point = row.convert(NSPoint(x: row.bounds.midX, y: row.bounds.midY), to: nil)
        XCTAssertTrue(pad.hit(at: point) is NSTableView, "AppKit finds the table under a row that presses")
        try pad.swipe(4, at: point)
        fixture.settle()
        XCTAssertEqual(try fixture.node("seen").props["text"], "swipe 2;")
    }
}

/// A window around a view of the page, with a trackpad's three-finger swipe
/// built as AppKit builds it and sent through the window's own dispatch.
private final class Trackpad {
    let window: NSWindow
    let root: NSView

    init(window: NSWindow, root: NSView) {
        self.window = window
        self.root = root
    }
    /// A session's root view in a window of its own.
    convenience init(_ session: ExactSession) {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let root = ExactView(session: session)
        window.contentView = root
        self.init(window: window, root: root)
    }
    func close() { window.close() }

    func hit(at point: NSPoint) -> NSView? { root.superview.flatMap { root.hitTest($0.convert(point, from: nil)) } }

    /// A three-finger swipe at `point`: CoreGraphics' gesture event of
    /// IOHID's swipe type (16) and direction mask (`mask` 1 up, 2 down, 4
    /// left, 8 right), fields not in its headers.
    func swipe(_ mask: Int64, at point: NSPoint) throws {
        let cg = try XCTUnwrap(CGEvent(source: nil))
        cg.type = try XCTUnwrap(CGEventType(rawValue: 29))
        cg.setIntegerValueField(try XCTUnwrap(CGEventField(rawValue: 110)), value: 16)
        cg.setIntegerValueField(try XCTUnwrap(CGEventField(rawValue: 115)), value: mask)
        let event = try XCTUnwrap(Agent.windowEvent(cg, at: point, in: window, flags: []))
        XCTAssertEqual(event.type, .swipe)
        window.sendEvent(event)
    }
}
#endif
