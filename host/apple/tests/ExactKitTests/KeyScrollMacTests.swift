#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D8.5: an arrow, Space and the Page keys scroll the scroller
/// the focus is in: a `scroll` node around the focused text, or the page when
/// that node has nowhere to go. A page is AppKit's own action; here it is
/// applied at once, as under the agent, since AppKit animates it on its own
/// frames (`ChainingScrollView.appliesAtOnce`). A key the node itself takes
/// (Space on a pressable, Shift with an arrow in selected text) is not a
/// scroll, and nothing scrolls where nothing can.
final class KeyScrollMacTests: XCTestCase {
    private var window: NSWindow!
    override func setUp() { ChainingScrollView.appliesAtOnce = true }
    override func tearDown() { ChainingScrollView.appliesAtOnce = ExactEnv.agentMode; window?.close(); window = nil }

    /// A page 300 tall whose document is 900, holding a `scroll` node 200
    /// tall whose content is 1,000: text in the node, a pressable in it, and
    /// text on the page under it.
    private func fixture() -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "auto", "overflow_x": "hidden"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Words in the list"]],
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Words on the page"]],
            ["op": "children", "id": 1, "ids": [2, 5]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 900],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 400, "h": 200],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 380, "h": 20],
            ["op": "frame", "id": 4, "x": 0, "y": 30, "w": 100, "h": 30],
            ["op": "frame", "id": 5, "x": 0, "y": 220, "w": 380, "h": 20],
            ["op": "content", "id": 2, "w": 400.0, "h": 1000.0],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        return p
    }
    /// A key down as AppKit sends it. Arrows and the Page keys are function keys.
    private func key(_ code: UInt16, _ character: Int, _ flags: NSEvent.ModifierFlags = []) -> NSEvent {
        let function = character >= 0xF700
        let s = String(UnicodeScalar(character)!)
        return NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: function ? flags.union([.function]) : flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                windowNumber: window.windowNumber, context: nil, characters: s, charactersIgnoringModifiers: s, isARepeat: false, keyCode: code)!
    }
    private var down: NSEvent { key(125, 0xF701) }
    private var up: NSEvent { key(126, 0xF700) }
    private var pageDown: NSEvent { key(121, 0xF72D) }
    private var pageUp: NSEvent { key(116, 0xF72C) }
    private var home: NSEvent { key(115, 0xF729) }
    private var end: NSEvent { key(119, 0xF72B) }
    private func space(_ flags: NSEvent.ModifierFlags = []) -> NSEvent { key(49, 0x20, flags) }

    func testKeysScrollTheScrollerTheFocusIsIn() throws {
        let p = fixture()
        let text = try XCTUnwrap(p.views[3]), list = try XCTUnwrap(p.views[2]?.scroll)
        func at(_ view: NSScrollView) -> CGFloat { view.contentView.bounds.minY }
        XCTAssertTrue(window.makeFirstResponder(text))
        text.keyDown(with: down)
        XCTAssertEqual(at(list), list.verticalLineScroll, "an arrow, by AppKit's line")
        XCTAssertEqual(list.verticalLineScroll, 10)
        text.keyDown(with: down)
        text.keyDown(with: up)
        XCTAssertEqual(at(list), 10)
        // A page: the clip's height less AppKit's overlap (`verticalPageScroll`, 10).
        text.keyDown(with: pageDown)
        XCTAssertEqual(at(list), 200)
        text.keyDown(with: space())
        XCTAssertEqual(at(list), 390, "Space is a page down")
        text.keyDown(with: space(.shift))
        XCTAssertEqual(at(list), 200, "and Shift-Space a page up")
        text.keyDown(with: pageUp)
        XCTAssertEqual(at(list), 10)
        text.keyDown(with: end)
        XCTAssertEqual(at(list), 800, "End, the document's end")
        // At its end the key is still the list's: the page does not move.
        text.keyDown(with: pageDown)
        text.keyDown(with: down)
        XCTAssertEqual(at(list), 800)
        XCTAssertEqual(at(p.viewport), 0)
        text.keyDown(with: home)
        XCTAssertEqual(at(list), 0)
        text.keyDown(with: key(125, 0xF701, .command))
        XCTAssertEqual(at(list), 800, "Command with the down arrow, the end too")
        text.keyDown(with: key(126, 0xF700, .command))
        XCTAssertEqual(at(list), 0)
    }

    func testTheNearestScrollerThatCanMoveTakesTheKeyAndANodesOwnKeyIsNotAScroll() throws {
        let p = fixture()
        let list = try XCTUnwrap(p.views[2]?.scroll)
        func at(_ view: NSScrollView) -> CGFloat { view.contentView.bounds.minY }
        // Text on the page: the page scrolls, by its own page.
        let onPage = try XCTUnwrap(p.views[5])
        XCTAssertTrue(window.makeFirstResponder(onPage))
        onPage.keyDown(with: pageDown)
        XCTAssertEqual(at(p.viewport), 290)
        XCTAssertEqual(at(list), 0)
        onPage.keyDown(with: home)
        XCTAssertEqual(at(p.viewport), 0)
        // A list with nowhere to go passes the key to the page around it.
        p.apply(wireBatch([["op": "content", "id": 2, "w": 400.0, "h": 200.0]]))
        let text = try XCTUnwrap(p.views[3])
        XCTAssertTrue(window.makeFirstResponder(text))
        text.keyDown(with: down)
        XCTAssertEqual(at(p.viewport), 10)
        XCTAssertEqual(at(list), 0)
        // Space on a pressable presses it; it scrolls nothing.
        p.apply(wireBatch([["op": "content", "id": 2, "w": 400.0, "h": 1000.0]]))
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let button = try XCTUnwrap(p.views[4])
        XCTAssertTrue(window.makeFirstResponder(button))
        button.keyDown(with: space())
        XCTAssertEqual(pressed, [4])
        XCTAssertEqual(at(list), 0)
        // An arrow there is the scroller's all the same.
        button.keyDown(with: down)
        XCTAssertEqual(at(list), 10)
        // Space at a focused box inside a pressable goes up to the pressable, which it presses.
        let inner = NodeView(id: 6, kind: "view", presenter: p)
        inner.props = ["tabIndex": "0"]
        inner.frame = NSRect(x: 0, y: 0, width: 40, height: 20)
        button.container.addSubview(inner); p.views[6] = inner
        XCTAssertTrue(window.makeFirstResponder(inner))
        inner.keyDown(with: space())
        XCTAssertEqual(pressed, [4, 4])
        XCTAssertEqual(at(list), 10, "and scrolls nothing")
    }
}
#endif
